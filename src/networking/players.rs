//! Player lifecycle and authoritative movement for Lightyear sessions.
//!
//! A successful remote connection creates a server-owned player entity. The
//! entity is replicated to all clients and associated with the connecting
//! peer through ControlledBy. Lightyear then routes that peer's input timeline
//! to the controlled entity. Clients render Remote copies; the local
//! first-person controller remains local and never owns the server Transform.

use bevy::prelude::*;
use lightyear::connection::host::HostClient;
use lightyear::prelude::client::input::*;
use lightyear::prelude::input::native::*;
use lightyear::prelude::server::*;
use lightyear::prelude::*;

use super::protocol::NetworkPlayerInput;
use serde::{Deserialize, Serialize};
use crate::player::{Player, PlayerInput};

const NETWORK_TICK_RATE: f32 = 64.0;
const NETWORK_TICK_SECONDS: f32 = 1.0 / NETWORK_TICK_RATE;
const PLAYER_WALK_SPEED: f32 = 3.5;
const PLAYER_SPRINT_SPEED: f32 = 6.0;
const PLAYER_CROUCH_SPEED: f32 = 1.8;
const PLAYER_GRAVITY: f32 = 20.0;
const PLAYER_JUMP_SPEED: f32 = 5.5;
const PLAYER_TERMINAL_VELOCITY: f32 = 45.0;
const PLAYER_GROUND_HEIGHT: f32 = 1.0;
const PLAYER_SPAWN_SPACING: f32 = 1.4;

/// Replicated identity marker shared by every network-visible player root.
/// Unlike Lightyear's Remote marker, this cannot match transport/link entities.
#[derive(Component, Serialize, Deserialize)]
pub(crate) struct NetworkAvatar;

/// Stable identity used by replicated inventory and held-item state.
#[derive(Component, Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct NetworkAvatarId(pub(crate) u64);

/// Server-only marker for a player controlled by a remote Lightyear peer.
#[derive(Component)]
pub(crate) struct NetworkPlayer;

/// Minimal vertical state for the first authoritative movement prototype.
/// Horizontal motion and orientation are derived entirely from input packets.
#[derive(Component, Default)]
struct NetworkPlayerMotion {
    vertical_velocity: f32,
}

/// Visual marker added to client-side remote copies after their first
/// frame. The mesh is a child of the replicated root and therefore inherits
/// the server-authored position and rotation.
#[derive(Component)]
struct NetworkPlayerVisual;

/// Installs the player networking systems on both host and client instances.
pub(crate) struct NetworkPlayersPlugin;

impl Plugin for NetworkPlayersPlugin {
    fn build(&self, app: &mut App) {
        app.component::<NetworkAvatar>().replicate();
        app.component::<NetworkAvatarId>().replicate();
        app.add_systems(
            FixedPreUpdate,
            buffer_local_player_input.in_set(InputSystems::WriteClientInputs),
        )
        .add_systems(FixedUpdate, simulate_authoritative_players)
        .add_systems(Update, spawn_remote_player_visuals)
        .add_observer(mark_local_player_as_network_avatar)
        .add_observer(add_input_marker_to_controlled_player)
        .add_observer(spawn_player_for_connected_peer);
    }
}

/// The host's local first-person player is also replicated to remote peers.
fn mark_local_player_as_network_avatar(
    trigger: On<Add<Player>>,
    mut commands: Commands,
) {
    commands
        .entity(trigger.entity)
        .insert((NetworkAvatar, NetworkAvatarId(0)));
}

/// When Lightyear marks a replicated entity as Controlled, attach the native
/// input marker. The InputPlugin creates and maintains ActionState for it.
fn add_input_marker_to_controlled_player(
    trigger: On<Add<Controlled>>,
    mut commands: Commands,
    authoritative_players: Query<&Transform, (With<Controlled>, Without<Player>)>,
    mut local_players: Query<&mut Transform, With<Player>>,
) {
    // The local first-person rig is spawned before the network handshake. Once
    // the server-controlled entity arrives, align that rig with its assigned
    // authoritative spawn transform so the owner does not appear at a second,
    // unrelated location in the level.
    if let Ok(authoritative) = authoritative_players.get(trigger.entity) {
        if let Ok(mut local) = local_players.single_mut() {
            local.translation = authoritative.translation;
            local.rotation = authoritative.rotation;
        }
    }

    commands
        .entity(trigger.entity)
        .insert(InputMarker::<NetworkPlayerInput>::default());
    info!(
        "Controlled player entity {:?} received by this client",
        trigger.entity
    );
}

/// Capture the local controller state into Lightyear's fixed-tick input buffer.
///
/// This is deliberately scheduled in WriteClientInputs. Writing ActionState in
/// Update or an arbitrary FixedUpdate system can miss the tick Lightyear is
/// about to serialize, producing a connected client that never moves remotely.
fn buffer_local_player_input(
    timeline: Res<LocalTimeline>,
    mut local_input: ResMut<PlayerInput>,
    status: Res<super::NetworkStatus>,
    local_players: Query<&Transform, With<Player>>,
    mut controlled_inputs: Query<
        &mut ActionState<NetworkPlayerInput>,
        With<InputMarker<NetworkPlayerInput>>,
    >,
) {
    let yaw = local_players
        .single()
        .map(|transform| transform.rotation.to_euler(EulerRot::YXZ).0)
        .unwrap_or(0.0);

    let mut packet = NetworkPlayerInput {
        move_x: local_input.movement.x,
        move_y: local_input.movement.y,
        yaw,
        pitch: local_input.pitch,
        interact: local_input.interact_pressed,
        sprint: local_input.sprint,
        crouch: local_input.crouch,
        jump: local_input.jump_pressed,
    };
    packet.sanitize();

    let mut wrote_client_input = false;
    for mut action_state in &mut controlled_inputs {
        action_state.0 = packet.clone();
        wrote_client_input = true;
    }
    // Keep the edge buffered until a client-controlled pawn exists and a
    // fixed-tick sample has actually been written. Host/offline input is local.
    if status.mode != Some(super::NetworkMode::Client) || wrote_client_input {
        local_input.interact_pressed = false;
    }

    trace!(
        target: "unknown_signal::network",
        tick = timeline.tick().0,
        move_x = packet.move_x,
        move_y = packet.move_y,
        yaw = packet.yaw,
        pitch = packet.pitch,
        interact = packet.interact,
        wrote_input = wrote_client_input,
        sprint = packet.sprint,
        crouch = packet.crouch,
        jump = packet.jump,
        "buffered local player input"
    );
}

/// A remote connection is not a player until Lightyear has accepted it and
/// emitted Connected. The in-process HostClient also has ClientOf and a
/// RemoteId (Local(0)), but it is the host's own endpoint, not a remote player.
/// Exclude it explicitly; the host already has its local `Player` entity.
fn spawn_player_for_connected_peer(
    trigger: On<Add<Connected>>,
    mut commands: Commands,
    remote_clients: Query<&RemoteId, (With<ClientOf>, Without<HostClient>)>,
) {
    let Ok(remote_id) = remote_clients.get(trigger.entity) else {
        return;
    };

    let peer_id = remote_id.0;
    let spawn_lane = (peer_id.to_bits() % 7) as f32 - 3.0;
    let spawn_position = Vec3::new(spawn_lane * PLAYER_SPAWN_SPACING, PLAYER_GROUND_HEIGHT, 0.0);

    let player = commands
        .spawn((
            NetworkPlayer,
            NetworkAvatar,
            NetworkAvatarId(peer_id.to_bits().wrapping_add(1)),
            NetworkPlayerMotion::default(),
            ActionState::<NetworkPlayerInput>::default(),
            Transform::from_translation(spawn_position),
            Visibility::default(),
            InheritedVisibility::default(),
            Name::new(format!("Network Player {peer_id:?}")),
            // Server owns the Transform. Every connected peer receives a
            // Remote copy; ControlledBy grants input ownership only to this peer.
            Replicate::to_clients(NetworkTarget::All),
            ControlledBy {
                owner: trigger.entity,
                lifetime: Default::default(),
            },
        ))
        .id();

    info!(
        "Spawned authoritative player {:?} for peer {:?} at {:?}",
        player, peer_id, spawn_position
    );
}

/// Apply input on the server at the same fixed rate used by Lightyear.
///
/// This first pass uses a compact kinematic controller. It keeps the server
/// authoritative and makes client movement visible to every peer while the
/// Avian collision/controller integration is developed as a separate step.
fn simulate_authoritative_players(
    mut players: Query<
        (
            &mut Transform,
            &ActionState<NetworkPlayerInput>,
            &mut NetworkPlayerMotion,
        ),
        (With<NetworkPlayer>, Without<Remote>),
    >,
) {
    for (mut transform, action_state, mut motion) in &mut players {
        let mut input = action_state.0.clone();
        input.sanitize();

        transform.rotation = Quat::from_rotation_y(input.yaw);

        let direction = calculate_move_direction(&input);
        let speed = movement_speed(&input);
        transform.translation += direction * speed * NETWORK_TICK_SECONDS;

        let grounded = transform.translation.y <= PLAYER_GROUND_HEIGHT + 0.001;
        if grounded {
            transform.translation.y = PLAYER_GROUND_HEIGHT;
            motion.vertical_velocity = 0.0;
            if input.jump {
                motion.vertical_velocity = PLAYER_JUMP_SPEED;
            }
        } else {
            motion.vertical_velocity = (motion.vertical_velocity
                - PLAYER_GRAVITY * NETWORK_TICK_SECONDS)
                .max(-PLAYER_TERMINAL_VELOCITY);
        }

        transform.translation.y += motion.vertical_velocity * NETWORK_TICK_SECONDS;
        if transform.translation.y < PLAYER_GROUND_HEIGHT {
            transform.translation.y = PLAYER_GROUND_HEIGHT;
            motion.vertical_velocity = 0.0;
        }
    }
}

/// Convert normalized local input into a world-space direction using the
/// authoritative yaw. Bevy's forward axis is -Z, matching the local controller.
fn calculate_move_direction(input: &NetworkPlayerInput) -> Vec3 {
    let local_direction = Vec3::new(input.move_x, 0.0, -input.move_y);
    (Quat::from_rotation_y(input.yaw) * local_direction).normalize_or_zero()
}

/// Keep the speed policy in one place so tests and server simulation use the
/// exact same sprint/crouch rules.
fn movement_speed(input: &NetworkPlayerInput) -> f32 {
    if input.crouch {
        PLAYER_CROUCH_SPEED
    } else if input.sprint && input.has_movement() {
        PLAYER_SPRINT_SPEED
    } else {
        PLAYER_WALK_SPEED
    }
}

/// Render actual player roots, not every Lightyear Remote entity. A remote
/// client receives host Player and other NetworkPlayer roots with NetworkAvatar.
/// The host itself renders the authoritative NetworkPlayer root directly;
/// its server-side entity does not carry the client-side Remote marker.
fn spawn_remote_player_visuals(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    remote_players: Query<
        Entity,
        (
            With<NetworkAvatar>,
            Or<(With<Remote>, With<NetworkPlayer>)>,
            Without<Controlled>,
            Without<NetworkPlayerVisual>,
        ),
    >,
) {
    if remote_players.is_empty() {
        return;
    }

    let mesh = meshes.add(Capsule3d::new(0.28, 1.0));
    let material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.22, 0.48, 0.52),
        metallic: 0.18,
        perceptual_roughness: 0.82,
        ..default()
    });

    for entity in &remote_players {
        commands
            .entity(entity)
            .insert((
                NetworkPlayerVisual,
                Visibility::default(),
                InheritedVisibility::default(),
            ))
            .with_child((
                Mesh3d(mesh.clone()),
                MeshMaterial3d(material.clone()),
                Transform::default(),
                Name::new("Remote Network Player Avatar"),
            ));

        info!(
            "Attached player avatar visual to network player entity {:?}",
            entity
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn forward_input_moves_toward_negative_z() {
        let input = NetworkPlayerInput {
            move_y: 1.0,
            ..default()
        };

        let direction = calculate_move_direction(&input);

        assert!(direction.z < -0.99);
        assert!(direction.x.abs() < 0.001);
    }

    #[test]
    fn yaw_rotates_forward_input_into_world_space() {
        let input = NetworkPlayerInput {
            move_y: 1.0,
            yaw: std::f32::consts::FRAC_PI_2,
            ..default()
        };

        let direction = calculate_move_direction(&input);

        assert!(direction.x < -0.99 || direction.x > 0.99);
        assert!(direction.z.abs() < 0.001);
    }

    #[test]
    fn diagonal_input_is_normalized_before_movement() {
        let mut input = NetworkPlayerInput {
            move_x: 1.0,
            move_y: 1.0,
            ..default()
        };
        input.sanitize();

        let direction = calculate_move_direction(&input);

        assert!((direction.length() - 1.0).abs() < 0.001);
    }

    #[test]
    fn crouch_is_slower_than_walk_and_sprint() {
        let crouch = NetworkPlayerInput {
            crouch: true,
            sprint: true,
            move_y: 1.0,
            ..default()
        };
        let walk = NetworkPlayerInput {
            move_y: 1.0,
            ..default()
        };
        let sprint = NetworkPlayerInput {
            sprint: true,
            move_y: 1.0,
            ..default()
        };

        assert!(movement_speed(&crouch) < movement_speed(&walk));
        assert!(movement_speed(&walk) < movement_speed(&sprint));
    }

    #[test]
    fn sprint_without_movement_uses_walk_speed() {
        let input = NetworkPlayerInput {
            sprint: true,
            ..default()
        };

        assert_eq!(movement_speed(&input), PLAYER_WALK_SPEED);
    }

    #[test]
    fn neutral_input_produces_zero_world_direction() {
        let input = NetworkPlayerInput::default();

        assert_eq!(calculate_move_direction(&input), Vec3::ZERO);
    }
}
