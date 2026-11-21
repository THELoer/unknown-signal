use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::time::Duration;

use bevy::prelude::*;
use lightyear::connection::host::HostClient;
use lightyear::connection::server::Start;
use lightyear::prelude::client::{ClientPlugins, Connect, RawClient};
use lightyear::prelude::server::{LinkOf, RawServer, ServerPlugins, ServerUdpIo};
use lightyear::prelude::{
    AppComponentExt, Client, Connected, Link, LocalAddr,
    NetworkTarget, PeerAddr, Replicate, ReplicationReceiver, ReplicationSender, UdpIo,
};

mod diagnostics;
mod players;
mod protocol;

const SERVER_PORT: u16 = 5000;
const TICK_RATE: f64 = 64.0;

/// The server currently owns the session simulation. Keep this value aligned
/// with the fixed simulation rate used by the gameplay systems.
#[derive(Resource, Clone, Copy, Debug)]
pub struct NetworkConfig {
    pub bind_addr: SocketAddr,
    pub tick_rate: f64,
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            bind_addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), SERVER_PORT),
            tick_rate: TICK_RATE,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum NetworkRequest {
    Host,
    Connect(SocketAddr),
}

#[derive(Resource, Default)]
pub struct PendingNetworkRequest(pub Option<NetworkRequest>);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NetworkMode {
    Host,
    Client,
}

#[derive(Resource)]
pub struct NetworkStatus {
    pub message: String,
    pub connected: bool,
    pub mode: Option<NetworkMode>,
    /// Number of remote server-side links currently marked Connected.
    pub peer_count: usize,
    /// Most recent local disconnect/error summary.
    pub last_error: Option<String>,
    requested: bool,
    session_entities: Vec<Entity>,
}

impl Default for NetworkStatus {
    fn default() -> Self {
        Self {
            message: "Select Host Server or Connect Server".to_string(),
            connected: false,
            mode: None,
            peer_count: 0,
            last_error: None,
            requested: false,
            session_entities: Vec::new(),
        }
    }
}

/// Small, local-only diagnostics snapshot. This deliberately contains no
/// gameplay state and is safe to expose to the menu/debug UI.
#[derive(Resource, Default, Debug)]
pub struct NetworkDiagnostics {
    pub connected_clients: usize,
    pub connected_server_peers: usize,
    pub remote_player_avatars: usize,
}

pub struct NetworkingPlugin;

impl Plugin for NetworkingPlugin {
    fn build(&self, app: &mut App) {
        let config = NetworkConfig::default();
        let tick_duration = Duration::from_secs_f64(1.0 / config.tick_rate);

        app.insert_resource(config)
            .add_plugins(ClientPlugins { tick_duration })
            .add_plugins(ServerPlugins { tick_duration })
            .add_plugins(protocol::NetworkProtocolPlugin)
            .add_plugins(players::NetworkPlayersPlugin)
            .add_plugins(diagnostics::NetworkDiagnosticsHudPlugin)
            .init_resource::<PendingNetworkRequest>()
            .init_resource::<NetworkStatus>()
            .init_resource::<NetworkDiagnostics>()
            // Transform is the first replicated gameplay component. The
            // server owns it; clients render Remote copies.
            .component::<Transform>()
            .replicate();

        app.add_systems(
            Update,
            (
                start_requested_session,
                monitor_connection,
                update_network_diagnostics,
            )
                .chain(),
        )
        .add_observer(add_replication_sender);
    }
}

/// Replacing a session first tears down the endpoints created by the previous
/// request. This prevents repeated menu actions from leaving stale UDP sockets
/// and multiple client entities in the same Bevy App.
fn start_requested_session(
    mut commands: Commands,
    mut request: ResMut<PendingNetworkRequest>,
    mut status: ResMut<NetworkStatus>,
    config: Res<NetworkConfig>,
) {
    let Some(request) = request.0.take() else {
        return;
    };

    for entity in status.session_entities.drain(..) {
        commands.entity(entity).despawn();
    }

    status.connected = false;
    status.peer_count = 0;
    status.last_error = None;
    status.requested = true;

    match request {
        NetworkRequest::Host => {
            let server = commands
                .spawn((RawServer, LocalAddr(config.bind_addr), ServerUdpIo))
                .id();
            commands.trigger(Start { entity: server });

            // Host mode is an in-process Lightyear host-client pair. A UDP
            // loopback client is a separate peer and is not the host topology.
            let mut host_client = commands.spawn((
                Client,
                LinkOf { endpoint: server },
                Link::default(),
                ReplicationReceiver,
            ));
            let host_client_entity = host_client.id();
            host_client.trigger(Connect::from);

            status.session_entities.extend([server, host_client_entity]);
            status.mode = Some(NetworkMode::Host);
            status.message = format!("Starting host on {}...", config.bind_addr);
            info!("Starting host server on {}", config.bind_addr);
        }
        NetworkRequest::Connect(server_addr) => {
            let local_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 0);
            let client = spawn_client(&mut commands, local_addr, server_addr);
            status.session_entities.push(client);
            status.mode = Some(NetworkMode::Client);
            status.message = format!("Connecting to {server_addr}...");
            info!("Connecting to server {server_addr}");
        }
    }
}

fn spawn_client(
    commands: &mut Commands,
    local_addr: SocketAddr,
    server_addr: SocketAddr,
) -> Entity {
    let mut client = commands.spawn((
        Client,
        LocalAddr(local_addr),
        PeerAddr(server_addr),
        Link::default(),
        ReplicationReceiver,
        RawClient,
        UdpIo::default(),
    ));
    let entity = client.id();
    client.trigger(Connect::from);
    entity
}

/// A server-side remote link needs a sender; the local HostClient does not.
/// Existing local host players are sent with interpolation to every peer.
fn add_replication_sender(
    trigger: On<Add<Connected>>,
    mut commands: Commands,
    server_links: Query<(), With<LinkOf>>,
    host_clients: Query<(), With<HostClient>>,
    players: Query<Entity, With<crate::player::Player>>,
) {
    if server_links.get(trigger.entity).is_ok() && host_clients.get(trigger.entity).is_err() {
        commands.entity(trigger.entity).insert(ReplicationSender);
        for player in &players {
            commands.entity(player).insert(Replicate::to_clients(NetworkTarget::All));
        }
        info!(
            "Remote peer {:?} connected; host player replication enabled",
            trigger.entity
        );
    }
}

/// Tracks the local endpoint's actual Connected state instead of treating the
/// existence of a Client entity as proof of a successful connection.
fn monitor_connection(
    clients: Query<(), (With<Client>, With<Connected>)>,
    server_peers: Query<(), (With<LinkOf>, With<Connected>, Without<HostClient>)>,
    mut status: ResMut<NetworkStatus>,
) {
    if !status.requested {
        return;
    }

    let local_connected = !clients.is_empty();
    let peer_count = server_peers.iter().count();
    status.peer_count = peer_count;

    if local_connected && !status.connected {
        status.connected = true;
        status.message = match status.mode {
            Some(NetworkMode::Host) => "Host session ready. Waiting for other players.".to_string(),
            Some(NetworkMode::Client) => "Connected to host. Joining session...".to_string(),
            None => "Connected.".to_string(),
        };
        info!("{}", status.message);
    } else if !local_connected && status.connected {
        status.connected = false;
        status.message = "Connection lost. Return to the menu and reconnect.".to_string();
        status.last_error = Some("The active client endpoint is no longer connected".to_string());
        warn!("{}", status.message);
    }

    if matches!(status.mode, Some(NetworkMode::Host)) && status.connected {
        status.message = format!("Host online · {} remote peer(s)", status.peer_count);
    }
}

fn update_network_diagnostics(
    clients: Query<(), (With<Client>, With<Connected>)>,
    server_peers: Query<(), (With<LinkOf>, With<Connected>, Without<HostClient>)>,
    remote_players: Query<(), With<lightyear::prelude::client::Remote>>,
    mut diagnostics: ResMut<NetworkDiagnostics>,
) {
    diagnostics.connected_clients = clients.iter().count();
    diagnostics.connected_server_peers = server_peers.iter().count();
    diagnostics.remote_player_avatars = remote_players.iter().count();
}
