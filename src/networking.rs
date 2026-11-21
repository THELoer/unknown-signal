use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::time::Duration;

use bevy::prelude::*;
use lightyear::connection::server::Start;
use lightyear::prelude::client::{ClientPlugins, Connect, RawClient, Remote};
use lightyear::prelude::server::{LinkOf, RawServer, ServerPlugins, ServerUdpIo};
use lightyear::prelude::{
    AppComponentExt, Client, Connected, Link, LocalAddr, NetworkTarget, PeerAddr, Replicate,
    ReplicationReceiver, ReplicationSender, UdpIo,
};

const SERVER_PORT: u16 = 5000;
const HOST_CLIENT_PORT: u16 = 5001;
const TICK_RATE: f64 = 64.0;

#[derive(Clone, Copy, Debug)]
pub enum NetworkRequest {
    Host,
    Connect(SocketAddr),
}

#[derive(Resource, Default)]
pub struct PendingNetworkRequest(pub Option<NetworkRequest>);

#[derive(Clone, Copy, Debug)]
pub enum NetworkMode {
    Host,
    Client,
}

#[derive(Resource)]
pub struct NetworkStatus {
    pub message: String,
    pub connected: bool,
    pub mode: Option<NetworkMode>,
    requested: bool,
}

impl Default for NetworkStatus {
    fn default() -> Self {
        Self {
            message: "Select Host Server or Connect Server".to_string(),
            connected: false,
            mode: None,
            requested: false,
        }
    }
}

pub struct NetworkingPlugin;

impl Plugin for NetworkingPlugin {
    fn build(&self, app: &mut App) {
        let tick_duration = Duration::from_secs_f64(1.0 / TICK_RATE);

        app.add_plugins(ClientPlugins { tick_duration })
            .add_plugins(ServerPlugins { tick_duration })
            .init_resource::<PendingNetworkRequest>()
            .init_resource::<NetworkStatus>()
            .component::<Transform>()
            .replicate();

        app.add_systems(Update, (start_requested_session, monitor_connection, spawn_remote_player_visuals))
            .add_observer(add_replication_sender);
    }
}

fn start_requested_session(
    mut commands: Commands,
    mut request: ResMut<PendingNetworkRequest>,
    mut status: ResMut<NetworkStatus>,
) {
    let Some(request) = request.0.take() else {
        return;
    };

    status.connected = false;
    status.requested = true;

    match request {
        NetworkRequest::Host => {
            let server_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), SERVER_PORT);
            let server = commands
                .spawn((RawServer, LocalAddr(server_addr), ServerUdpIo))
                .id();
            commands.trigger(Start { entity: server });

            let local_client_addr =
                SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), HOST_CLIENT_PORT);
            spawn_client(
                &mut commands,
                local_client_addr,
                SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), SERVER_PORT),
            );

            status.mode = Some(NetworkMode::Host);
            status.message = format!("Starting host on 0.0.0.0:{SERVER_PORT}...");
            info!("Started host server on 0.0.0.0:{SERVER_PORT}");
        }
        NetworkRequest::Connect(server_addr) => {
            let local_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 0);
            spawn_client(&mut commands, local_addr, server_addr);
            status.mode = Some(NetworkMode::Client);
            status.message = format!("Connecting to {server_addr}...");
            info!("Connecting to server {server_addr}");
        }
    }
}

fn spawn_client(commands: &mut Commands, local_addr: SocketAddr, server_addr: SocketAddr) {
    let mut client = commands.spawn((
        Client,
        LocalAddr(local_addr),
        PeerAddr(server_addr),
        Link::default(),
        ReplicationReceiver,
        RawClient,
        UdpIo::default(),
    ));
    client.trigger(Connect::from);
}

fn add_replication_sender(
    trigger: On<Add<Connected>>,
    mut commands: Commands,
    server_links: Query<(), With<LinkOf>>,
    players: Query<Entity, With<crate::player::Player>>,
) {
    // Host mode also has a local client endpoint. Only server-side peer links
    // should receive a replication sender.
    if server_links.get(trigger.entity).is_ok() {
        commands.entity(trigger.entity).insert(ReplicationSender);
        for player in &players {
            commands.entity(player).insert(Replicate::to_clients(NetworkTarget::All));
        }
        info!("A remote client connected; player transform replication enabled");
    }
}

fn monitor_connection(
    clients: Query<(), (With<Client>, With<Connected>)>,
    mut status: ResMut<NetworkStatus>,
) {
    if !status.requested || status.connected || clients.is_empty() {
        return;
    }

    status.connected = true;
    status.message = match status.mode {
        Some(NetworkMode::Host) => "Host session ready. Waiting for other players.".to_string(),
        Some(NetworkMode::Client) => "Connected to host. Joining session...".to_string(),
        None => "Connected.".to_string(),
    };
    info!("{}", status.message);
}


#[derive(Component)]
struct RemotePlayerVisual;

fn spawn_remote_player_visuals(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    remote_players: Query<(Entity, &Transform), (With<Remote>, Without<RemotePlayerVisual>)>,
) {
    if remote_players.is_empty() { return; }
    let mesh = meshes.add(Capsule3d::new(0.28, 1.0));
    let material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.22, 0.48, 0.52),
        metallic: 0.18,
        perceptual_roughness: 0.82,
        ..default()
    });
    for (entity, _) in &remote_players {
        commands.entity(entity).insert(RemotePlayerVisual).with_child((
            Mesh3d(mesh.clone()),
            MeshMaterial3d(material.clone()),
            Transform::default(),
            Name::new("Remote Player Avatar"),
        ));
        info!("Spawned visual for replicated remote player {entity:?}");
    }
}
