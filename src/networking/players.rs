//! Player lifecycle and authoritative movement for Lightyear sessions.
//!
//! A successful remote connection creates a server-owned player entity. The
//! entity is replicated to all clients and associated with the connecting
//! peer through ControlledBy. Lightyear then routes that peer's input timeline
//! to the controlled entity. Clients render Interpolated copies; the local
//! first-person controller remains local and never owns the server Transform.

use avian3d::prelude::*;
use bevy::prelude::*;
use lightyear::connection::client::Disconnected;
use lightyear::prelude::client::input::*;
use lightyear::prelude::input::native::*;
use lightyear::prelude::server::*;
use lightyear::prelude::*;

use super::protocol::NetworkPlayerInput;
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

/// Server-only marker for a player controlled by a remote Lightyear peer.
#[derive(Component)]
pub(crate) struct NetworkPlayer;

/// Records which connection owns a server-side player entity. This is used
/// to clean up the player when that connection is disconnected or rejected.
#[derive(Component)]
struct NetworkPlayerOwner(Entity);

/// Minimal vertical state for the first authoritative movement prototype.
/// Horizontal motion and orientation are derived entirely from input packets.
#[derive(Component, Default)]
struct NetworkPlayerMotion {
    vertical_velocity: f32,
}

/// Visual marker added to client-side interpolated copies after their first
/// frame. The mesh is a child of the replicated root and therefore inherits
/// the server-authored position and rotation.
#[derive(Component)]
struct NetworkPlayerVisual;

/// Installs the player networking systems on both host and client instances.
pub(crate) struct NetworkPlayersPlugin;

impl Plugin for NetworkPlayersPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            FixedPreUpdate,
            buffer_local_player_input.in_set(InputSystems::WriteClientInputs),
        )
        .add_systems(FixedUpdate, simulate_authoritative_players)
        .add_systems(Update, spawn_interpolated_player_visuals)
        .add_observer(add_input_marker_to_controlled_player)
        .add_observer(spawn_player_for_connected_peer)
        .add_observer(despawn_players_for_disconnected_peer);
    }
}

/// When Lightyear marks a replicated entity as Controlled, attach the native
/// input marker. The InputPlugin creates and maintains ActionState for it.
fn add_input_marker_to_controlled_player(
    trigger: On<Add<Controlled>>,
    mut commands: Commands,
) {
    commands
        .entity(trigger.entity)
        .insert(InputMarker::<NetworkPlayerInput>::default());
    debug!(
        "Added network input marker to controlled player {:?}",
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
    local_input: Res<PlayerInput>,
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
        sprint: local_input.sprint,
        crouch: local_input.crouch,
        jump: local_input.jump_pressed,
    };
    packet.sanitize();

    for mut action_state in &mut controlled_inputs {
        action_state.0 = packet.clone();
    }

    trace!(
        target: "unknown_signal::network",
        tick = timeline.tick().0,
        move_x = packet.move_x,
        move_y = packet.move_y,
        yaw = packet.yaw,
        sprint = packet.sprint,
        crouch = packet.crouch,
        jump = packet.jump,
        "buffered local player input"
    );
}

/// A remote connection is not a player until Lightyear has accepted it and
/// emitted Connected. ClientOf prevents this observer from treating the local
/// HostClient endpoint as a remote player.
fn spawn_player_for_connected_peer(
    trigger: On<Add<Connected>>,
    mut commands: Commands,
    remote_clients: Query<&RemoteId, With<ClientOf>>,
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
            NetworkPlayerOwner(trigger.entity),
            NetworkPlayerMotion::default(),
            Transform::from_translation(spawn_position),
            Visibility::default(),
            InheritedVisibility::default(),
            Name::new(format!("Network Player {peer_id:?}")),
            // Server owns the Transform. Every connected peer receives an
            // interpolated copy; the controlling peer supplies input through
            // ControlledBy rather than writing replicated state directly.
            Replicate::to_clients(NetworkTarget::All),
            InterpolationTarget::to_clients(NetworkTarget::All),
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

/// Remove only the player owned by the disconnected link. Other clients and
/// their player entities remain alive; the server session itself stays up.
fn despawn_players_for_disconnected_peer(
    trigger: On<Add<Disconnected>>,
    mut commands: Commands,
    players: Query<(Entity, &NetworkPlayerOwner)>,
) {
    let mut removed = 0usize;

    for (entity, owner) in &players {
        if owner.0 == trigger.entity {
            commands.entity(entity).despawn();
            removed += 1;
        }
    }

    if removed > 0 {
        info!(
            "Removed {removed} network player(s) for disconnected link {:?}",
            trigger.entity
        );
    }
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
        With<NetworkPlayer>,
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

/// Replicated entities intended for other peers receive Interpolated. Remote
/// is a connection/topology marker, not the marker for a replicated player
/// entity; querying Remote here was the reason the previous visual system
/// skipped the client's replicated player entirely.
fn spawn_interpolated_player_visuals(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    interpolated_players: Query<Entity, (With<Interpolated>, Without<NetworkPlayerVisual>)>,
) {
    if interpolated_players.is_empty() {
        return;
    }

    let mesh = meshes.add(Capsule3d::new(0.28, 1.0));
    let material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.22, 0.48, 0.52),
        metallic: 0.18,
        perceptual_roughness: 0.82,
        ..default()
    });

    for entity in &interpolated_players {
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
                Name::new("Interpolated Network Player Avatar"),
            ));

        debug!(
            "Attached network player visual to interpolated entity {:?}",
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
