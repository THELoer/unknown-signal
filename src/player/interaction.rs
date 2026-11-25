use std::collections::HashMap;
use std::f32::consts::FRAC_PI_2;

use avian3d::prelude::*;
use bevy::prelude::*;
use lightyear::prelude::client::Remote;
use lightyear::prelude::input::native::ActionState;
use lightyear::prelude::{AppComponentExt, NetworkTarget, Replicate};
use serde::{Deserialize, Serialize};

use crate::networking::players::{NetworkAvatarId, NetworkPlayer};
use crate::networking::protocol::NetworkPlayerInput;
use crate::networking::{NetworkMode, NetworkStatus};
use crate::player::controller::{Player, PlayerCamera};
use crate::player::{GameplayActive, PlayerInput, PlayerSettings};

const INTERACTION_DISTANCE: f32 = 3.0;
const DOOR_ANIMATION_SECONDS: f32 = 0.38;
const DOOR_WIDTH: f32 = 1.4;
const DOOR_HEIGHT: f32 = 2.2;

const PICKUP_ITEM_HALF_EXTENT: f32 = 0.22;
const HELD_ITEM_OFFSET: Vec3 = Vec3::new(0.28, -0.22, -0.85);
const DROP_DISTANCE: f32 = 1.35;
const INTERACTION_COOLDOWN_SECONDS: f32 = 0.20;

/// Replicated state for a door. The animation timer is deliberately local:
/// the server owns the actual Transform and clients receive its updates.
#[derive(Component, Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
struct DoorState {
    is_open: bool,
}

/// Replicated identity marker for a pickup. Keeping this separate from the
/// state lets the client attach local-only physics and rendering components.
#[derive(Component, Serialize, Deserialize, Clone, Copy, Debug, Default)]
struct PickupItem;

/// The server writes holder identity here. None means the item is in the
/// world; Some(id) means it follows that player's authoritative transform.
#[derive(Component, Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
struct PickupState {
    holder_id: Option<u64>,
}

/// Local-only interpolation state for the server-authoritative door transform.
#[derive(Component, Default)]
struct DoorAnimation {
    observed_open: Option<bool>,
    animation_from: Quat,
    animation_to: Quat,
    animation_elapsed: f32,
}

/// Local-only reference from a carried item to its authoritative player entity.
/// Entity IDs are never sent over the wire; PickupState::holder_id is.
#[derive(Component)]
struct HeldByServer(Entity);

#[derive(Component)]
struct DoorVisual;

#[derive(Component)]
struct PickupVisual;

#[derive(Component)]
struct InteractionPromptText;

#[derive(Component)]
struct HeldItemText;

pub struct InteractionPlugin;

impl Plugin for InteractionPlugin {
    fn build(&self, app: &mut App) {
        // Register only serializable gameplay state. Mesh handles, physics
        // colliders, animation timers and server Entity references stay local.
        app.component::<DoorState>().replicate();
        app.component::<PickupItem>().replicate();
        app.component::<PickupState>().replicate();

        app.add_systems(
            Startup,
            (spawn_test_door, spawn_test_pickup, spawn_interaction_hud),
        )
        .add_systems(
            Update,
            (
                configure_network_world_objects,
                attach_network_object_visuals,
                update_remote_pickup_collision,
                update_interaction_hud,
                process_local_host_interaction,
            )
                .chain(),
        )
        .add_systems(
            FixedUpdate,
            (
                process_remote_player_interactions,
                release_orphaned_held_items,
                animate_doors,
                follow_held_items,
            )
                .chain(),
        );
    }
}

/// Switch the test objects from local sandbox entities to server-owned network
/// entities once the user chooses Host or Connect in the menu.
///
/// A client removes its temporary Startup copies. The authoritative copies are
/// then created only by the host and arrive through Lightyear replication.
/// The surrounding frame is static greybox geometry and remains local.
fn configure_network_world_objects(
    mut commands: Commands,
    status: Res<NetworkStatus>,
    mut configured_mode: Local<Option<NetworkMode>>,
    objects: Query<Entity, (Or<(With<DoorState>, With<PickupItem>)>, Without<Remote>)>,
) {
    let Some(mode) = status.mode else {
        return;
    };

    if *configured_mode == Some(mode) {
        return;
    }

    match mode {
        NetworkMode::Host => {
            for entity in &objects {
                commands
                    .entity(entity)
                    .insert(Replicate::to_clients(NetworkTarget::All));
            }
            info!(
                "World interaction authority enabled; {} door/item entity(ies) will replicate",
                objects.iter().count()
            );
        }
        NetworkMode::Client => {
            let mut removed = 0usize;
            for entity in &objects {
                commands.entity(entity).despawn();
                removed += 1;
            }
            info!(
                "Removed {removed} local door/item placeholder(s); waiting for host world state"
            );
        }
    }

    *configured_mode = Some(mode);
}

/// Build render meshes and local colliders after an entity exists in this
/// process. Lightyear replicates state and Transform, never GPU asset handles.
fn attach_network_object_visuals(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    doors: Query<Entity, (With<DoorState>, Without<DoorVisual>)>,
    pickups: Query<(Entity, &PickupState), (With<PickupItem>, Without<PickupVisual>)>,
    remote_entities: Query<(), With<Remote>>,
) {
    if doors.is_empty() && pickups.is_empty() {
        return;
    }

    let cube = meshes.add(Cuboid::new(1.0, 1.0, 1.0));
    let door_material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.25, 0.31, 0.34),
        metallic: 0.58,
        perceptual_roughness: 0.62,
        ..default()
    });
    let handle_material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.72, 0.46, 0.12),
        metallic: 0.68,
        perceptual_roughness: 0.42,
        ..default()
    });
    let pickup_material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.34, 0.25, 0.15),
        metallic: 0.28,
        perceptual_roughness: 0.88,
        ..default()
    });

    for entity in &doors {
        let is_remote = remote_entities.get(entity).is_ok();
        let mut door = commands.entity(entity);
        door.insert((DoorVisual, Visibility::default(), InheritedVisibility::default()));
        if is_remote {
            // Replicated roots do not receive Avian components. Add a local
            // kinematic body so the child collider participates in raycasts.
            door.insert(RigidBody::Kinematic);
        }

        door.with_children(|children| {
            children.spawn((
                Mesh3d(cube.clone()),
                MeshMaterial3d(door_material.clone()),
                Transform::from_xyz(DOOR_WIDTH * 0.5, 0.0, 0.0)
                    .with_scale(Vec3::new(DOOR_WIDTH, DOOR_HEIGHT, 0.14)),
                Name::new("Replicated Door Leaf"),
            ));
            children.spawn((
                Collider::cuboid(DOOR_WIDTH * 0.5, DOOR_HEIGHT * 0.5, 0.07),
                Transform::from_xyz(DOOR_WIDTH * 0.5, 0.0, 0.0),
                Name::new("Door Interaction Collider"),
            ));
            children.spawn((
                Mesh3d(cube.clone()),
                MeshMaterial3d(handle_material.clone()),
                Transform::from_xyz(DOOR_WIDTH - 0.13, 0.0, -0.085)
                    .with_scale(Vec3::new(0.045, 0.28, 0.035)),
                Name::new("Door Handle"),
            ));
        });
    }

    for (entity, state) in &pickups {
        let is_remote = remote_entities.get(entity).is_ok();
        let mut pickup = commands.entity(entity);
        pickup.insert((PickupVisual, Visibility::default(), InheritedVisibility::default()));

        if is_remote {
            pickup.insert(RigidBody::Kinematic);
            if state.holder_id.is_none() {
                pickup.insert(Collider::cuboid(
                    PICKUP_ITEM_HALF_EXTENT,
                    PICKUP_ITEM_HALF_EXTENT,
                    PICKUP_ITEM_HALF_EXTENT,
                ));
            }
        }

        pickup.with_children(|children| {
            children.spawn((
                Mesh3d(cube.clone()),
                MeshMaterial3d(pickup_material.clone()),
                Transform::from_scale(Vec3::splat(PICKUP_ITEM_HALF_EXTENT * 2.0)),
                Name::new("Replicated Pickup Visual"),
            ));
        });
    }
}

/// Remote clients use a non-simulated collider for aiming at free pickups. A
/// carried item has no collider, so it cannot be picked up by a second player.
fn update_remote_pickup_collision(
    mut commands: Commands,
    pickups: Query<
        (Entity, &PickupState),
        (With<PickupItem>, With<Remote>, Changed<PickupState>),
    >,
) {
    for (entity, state) in &pickups {
        if state.holder_id.is_some() {
            commands.entity(entity).remove::<Collider>();
        } else {
            commands.entity(entity).insert(Collider::cuboid(
                PICKUP_ITEM_HALF_EXTENT,
                PICKUP_ITEM_HALF_EXTENT,
                PICKUP_ITEM_HALF_EXTENT,
            ));
        }
    }
}

fn spawn_interaction_hud(mut commands: Commands) {
    commands.spawn((
        InteractionPromptText,
        Text::new(""),
        TextFont {
            font_size: FontSize::Px(18.0),
            ..default()
        },
        TextColor(Color::srgb(0.88, 0.93, 0.91)),
        Node {
            position_type: PositionType::Absolute,
            left: percent(0),
            right: percent(0),
            bottom: px(42),
            justify_content: JustifyContent::Center,
            ..default()
        },
    ));

    commands.spawn((
        HeldItemText,
        Text::new(""),
        TextFont {
            font_size: FontSize::Px(13.0),
            ..default()
        },
        TextColor(Color::srgb(0.70, 0.78, 0.79)),
        Node {
            position_type: PositionType::Absolute,
            left: px(24),
            bottom: px(24),
            ..default()
        },
    ));

    commands.spawn((
        Text::new("+"),
        TextFont {
            font_size: FontSize::Px(22.0),
            ..default()
        },
        TextColor(Color::srgba(0.82, 0.89, 0.88, 0.72)),
        Node {
            position_type: PositionType::Absolute,
            left: percent(50),
            top: percent(50),
            ..default()
        },
    ));
}

/// Resolve the identity of the local network pawn. On a host this is the
/// in-process Player (id 0); on a client it is the Lightyear Controlled copy.
fn local_network_avatar_id(
    status: &NetworkStatus,
    local_players: &Query<&NetworkAvatarId, With<Player>>,
    controlled_players: &Query<&NetworkAvatarId, With<lightyear::prelude::Controlled>>,
) -> Option<u64> {
    match status.mode {
        Some(NetworkMode::Client) => controlled_players.single().ok().map(|id| id.0),
        Some(NetworkMode::Host) | None => local_players.single().ok().map(|id| id.0),
    }
}

fn update_interaction_hud(
    active: Res<GameplayActive>,
    status: Res<NetworkStatus>,
    players: Query<Entity, With<Player>>,
    local_players: Query<&NetworkAvatarId, With<Player>>,
    controlled_players: Query<&NetworkAvatarId, With<lightyear::prelude::Controlled>>,
    cameras: Query<&GlobalTransform, With<PlayerCamera>>,
    items: Query<(Entity, &PickupState), With<PickupItem>>,
    collider_bodies: Query<&ColliderOf>,
    doors: Query<(), With<DoorState>>,
    spatial_query: SpatialQuery,
    mut prompt_text: Query<&mut Text, (With<InteractionPromptText>, Without<HeldItemText>)>,
    mut held_text: Query<&mut Text, (With<HeldItemText>, Without<InteractionPromptText>)>,
) {
    let local_id = local_network_avatar_id(&status, &local_players, &controlled_players);
    let carrying_item = local_id.is_some_and(|id| {
        items.iter().any(|(_, state)| state.holder_id == Some(id))
    });

    let mut prompt = String::new();
    if active.0 {
        if carrying_item {
            prompt = "E  —  DROP ITEM".to_string();
        } else if let (Ok(player), Ok(camera)) = (players.single(), cameras.single()) {
            let transform = camera.compute_transform();
            let filter = SpatialQueryFilter::from_excluded_entities([player]);
            if let Some(hit) = spatial_query.cast_ray(
                transform.translation,
                transform.forward(),
                INTERACTION_DISTANCE,
                true,
                &filter,
            ) {
                if items
                    .get(hit.entity)
                    .is_ok_and(|(_, state)| state.holder_id.is_none())
                {
                    prompt = "E  —  PICK UP".to_string();
                } else {
                    let door_entity = collider_bodies
                        .get(hit.entity)
                        .map(|collider| collider.body)
                        .unwrap_or(hit.entity);
                    if doors.get(door_entity).is_ok() {
                        prompt = "E  —  OPEN / CLOSE".to_string();
                    }
                }
            }
        }
    }

    for mut text in &mut held_text {
        **text = if carrying_item {
            "CARRYING ITEM  ·  E TO DROP".to_string()
        } else {
            String::new()
        };
    }

    for mut text in &mut prompt_text {
        **text = prompt.clone();
    }
}

/// Host-local interaction is authoritative too: it mutates the same server
/// entities that are replicated to clients. A remote client never runs this
/// path; its E press travels inside NetworkPlayerInput instead.
fn process_local_host_interaction(
    mut commands: Commands,
    time: Res<Time>,
    mut cooldown: Local<f32>,
    active: Res<GameplayActive>,
    status: Res<NetworkStatus>,
    settings: Res<PlayerSettings>,
    keyboard: Res<ButtonInput<KeyCode>>,
    players: Query<(Entity, &NetworkAvatarId), With<Player>>,
    cameras: Query<&GlobalTransform, With<PlayerCamera>>,
    spatial_query: SpatialQuery,
    collider_bodies: Query<&ColliderOf>,
    mut doors: Query<&mut DoorState>,
    mut pickups: Query<(Entity, &mut PickupState), With<PickupItem>>,
    held_items: Query<(Entity, &HeldByServer), With<PickupItem>>,
) {
    *cooldown = (*cooldown - time.delta_secs()).max(0.0);
    if !active.0
        || status.mode == Some(NetworkMode::Client)
        || !keyboard.just_pressed(settings.interact)
        || *cooldown > 0.0
    {
        return;
    }
    *cooldown = INTERACTION_COOLDOWN_SECONDS;

    let Ok((player, player_id)) = players.single() else {
        return;
    };
    let Ok(camera) = cameras.single() else {
        return;
    };
    let camera = camera.compute_transform();

    apply_interaction_request(
        player,
        player_id.0,
        camera.translation,
        camera.forward(),
        &mut commands,
        &spatial_query,
        &collider_bodies,
        &mut doors,
        &mut pickups,
        &held_items,
    );
}

/// Remote input is evaluated only on the server-owned NetworkPlayer entities.
/// The request contains no target Entity, so clients cannot ask the server to
/// mutate arbitrary entities by guessing their local Bevy IDs.
fn process_remote_player_interactions(
    mut commands: Commands,
    time: Res<Time>,
    mut cooldowns: Local<HashMap<Entity, f32>>,
    players: Query<
        (
            Entity,
            &Transform,
            &NetworkAvatarId,
            &ActionState<NetworkPlayerInput>,
        ),
        (With<NetworkPlayer>, Without<Remote>),
    >,
    spatial_query: SpatialQuery,
    collider_bodies: Query<&ColliderOf>,
    mut doors: Query<&mut DoorState>,
    mut pickups: Query<(Entity, &mut PickupState), With<PickupItem>>,
    held_items: Query<(Entity, &HeldByServer), With<PickupItem>>,
) {
    cooldowns.retain(|entity, _| players.get(*entity).is_ok());
    for (player, transform, player_id, action_state) in &players {
        let cooldown = cooldowns.entry(player).or_default();
        *cooldown = (*cooldown - time.delta_secs()).max(0.0);

        let mut input = action_state.0.clone();
        input.sanitize();
        if !input.interact || *cooldown > 0.0 {
            continue;
        }
        *cooldown = INTERACTION_COOLDOWN_SECONDS;

        let aim = Quat::from_rotation_y(input.yaw) * Quat::from_rotation_x(input.pitch);
        let direction = Dir3::new(aim * Vec3::NEG_Z).unwrap_or(Dir3::NEG_Z);
        let origin = transform.translation + Vec3::Y * 0.55;

        apply_interaction_request(
            player,
            player_id.0,
            origin,
            direction,
            &mut commands,
            &spatial_query,
            &collider_bodies,
            &mut doors,
            &mut pickups,
            &held_items,
        );
    }
}

/// Shared authoritative interaction handler for the host's local player and
/// all remote server pawns. It enforces range, ownership and current item state.
fn apply_interaction_request(
    player: Entity,
    player_id: u64,
    origin: Vec3,
    direction: Dir3,
    commands: &mut Commands,
    spatial_query: &SpatialQuery,
    collider_bodies: &Query<&ColliderOf>,
    doors: &mut Query<&mut DoorState>,
    pickups: &mut Query<(Entity, &mut PickupState), With<PickupItem>>,
    held_items: &Query<(Entity, &HeldByServer), With<PickupItem>>,
) {
    // E while carrying drops the item. The server chooses the drop position;
    // the client never submits a world-space location.
    if let Some(item) = held_items
        .iter()
        .find_map(|(entity, holder)| (holder.0 == player).then_some(entity))
    {
        if let Ok((_, mut state)) = pickups.get_mut(item) {
            state.holder_id = None;
        }

        let drop_position = origin + *direction * DROP_DISTANCE + Vec3::Y * 0.15;
        commands.entity(item).remove::<HeldByServer>().insert((
            RigidBody::Dynamic,
            Collider::cuboid(
                PICKUP_ITEM_HALF_EXTENT,
                PICKUP_ITEM_HALF_EXTENT,
                PICKUP_ITEM_HALF_EXTENT,
            ),
            LinearVelocity::default(),
            Transform::from_translation(drop_position),
        ));
        info!("Player {player_id} dropped a network item");
        return;
    }

    let filter = SpatialQueryFilter::from_excluded_entities([player]);
    let Some(hit) = spatial_query.cast_ray(
        origin,
        direction,
        INTERACTION_DISTANCE,
        true,
        &filter,
    ) else {
        return;
    };

    // A free pickup can be claimed by exactly one player. The server checks
    // the replicated state before inserting its private holder Entity.
    if let Ok((item, mut state)) = pickups.get_mut(hit.entity) {
        if state.holder_id.is_some() {
            return;
        }

        state.holder_id = Some(player_id);
        commands
            .entity(item)
            .insert(HeldByServer(player))
            .remove::<RigidBody>()
            .remove::<Collider>()
            .remove::<LinearVelocity>();
        info!("Player {player_id} picked up a network item");
        return;
    }

    // Door colliders live on visual children. Resolve the collider back to the
    // replicated root before changing its authoritative state.
    let door_entity = collider_bodies
        .get(hit.entity)
        .map(|collider| collider.body)
        .unwrap_or(hit.entity);

    if let Ok(mut state) = doors.get_mut(door_entity) {
        state.is_open = !state.is_open;
        info!(
            "Player {player_id} {} a network door",
            if state.is_open { "opened" } else { "closed" }
        );
    }
}

/// If a player disconnects while carrying an item, release it back into the
/// world instead of leaving a permanently hidden, collider-less object.
fn release_orphaned_held_items(
    mut commands: Commands,
    held_items: Query<(Entity, &HeldByServer), With<PickupItem>>,
    players: Query<(), Or<(With<Player>, With<NetworkPlayer>)>>,
    mut pickup_states: Query<&mut PickupState>,
) {
    for (item, holder) in &held_items {
        if players.get(holder.0).is_ok() {
            continue;
        }

        if let Ok(mut state) = pickup_states.get_mut(item) {
            state.holder_id = None;
        }
        commands.entity(item).remove::<HeldByServer>().insert((
            RigidBody::Dynamic,
            Collider::cuboid(
                PICKUP_ITEM_HALF_EXTENT,
                PICKUP_ITEM_HALF_EXTENT,
                PICKUP_ITEM_HALF_EXTENT,
            ),
            LinearVelocity::default(),
        ));
        warn!("Released a carried item after its owner disconnected");
    }
}

/// Animate only authoritative door roots. Clients receive the resulting
/// Transform updates and do not run a competing local animation.
fn animate_doors(
    time: Res<Time>,
    mut doors: Query<(&mut Transform, &DoorState, &mut DoorAnimation), Without<Remote>>,
) {
    for (mut transform, state, mut animation) in &mut doors {
        if animation.observed_open != Some(state.is_open) {
            animation.animation_from = transform.rotation;
            animation.animation_to = if state.is_open {
                Quat::from_rotation_y(FRAC_PI_2)
            } else {
                Quat::IDENTITY
            };
            animation.animation_elapsed = 0.0;
            animation.observed_open = Some(state.is_open);
        }

        if animation.animation_elapsed >= DOOR_ANIMATION_SECONDS {
            continue;
        }

        animation.animation_elapsed =
            (animation.animation_elapsed + time.delta_secs()).min(DOOR_ANIMATION_SECONDS);
        let progress = animation.animation_elapsed / DOOR_ANIMATION_SECONDS;
        let eased = progress * progress * (3.0 - 2.0 * progress);
        transform.rotation = animation
            .animation_from
            .slerp(animation.animation_to, eased);
    }
}

/// Keep a carried item in front of its owner using only server-side transforms.
/// This also means a disconnect cannot leave a client-owned item transform.
fn follow_held_items(
    mut pickups: Query<(&mut Transform, &HeldByServer), With<PickupItem>>,
    players: Query<
        (
            Entity,
            &Transform,
            Option<&ActionState<NetworkPlayerInput>>,
        ),
        (Or<(With<Player>, With<NetworkPlayer>)>, Without<PickupItem>),
    >,
    local_input: Res<PlayerInput>,
) {
    for (mut item_transform, holder) in &mut pickups {
        let Ok((_, player_transform, network_input)) = players.get(holder.0) else {
            continue;
        };

        let (yaw, pitch) = if let Some(action_state) = network_input {
            let input = &action_state.0;
            (input.yaw, input.pitch)
        } else {
            (
                player_transform.rotation.to_euler(EulerRot::YXZ).0,
                local_input.pitch,
            )
        };

        let aim = Quat::from_rotation_y(yaw) * Quat::from_rotation_x(pitch);
        item_transform.translation =
            player_transform.translation + aim * HELD_ITEM_OFFSET + Vec3::Y * 0.55;
        item_transform.rotation = aim;
    }
}

fn spawn_test_door(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let cube = meshes.add(Cuboid::new(1.0, 1.0, 1.0));
    let frame_material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.10, 0.14, 0.17),
        metallic: 0.72,
        perceptual_roughness: 0.48,
        ..default()
    });
    let warning_material = materials.add(Color::srgb(0.72, 0.46, 0.12));

    // The root transform is the hinge pivot. Its state and transform replicate;
    // the mesh and collider are attached locally after the root is created.
    commands.spawn((
        DoorState::default(),
        DoorAnimation::default(),
        RigidBody::Kinematic,
        Transform::from_xyz(-DOOR_WIDTH * 0.5, DOOR_HEIGHT * 0.5, 2.5),
        Visibility::default(),
        Name::new("Network Door"),
    ));

    // The frame is decorative static geometry. Every client builds the same
    // frame locally, while the moving leaf itself comes from the host.
    spawn_frame_piece(
        &mut commands,
        cube.clone(),
        frame_material.clone(),
        Vec3::new(0.14, DOOR_HEIGHT + 0.22, 0.24),
        Vec3::new(-0.82, DOOR_HEIGHT * 0.5, 2.5),
    );
    spawn_frame_piece(
        &mut commands,
        cube.clone(),
        frame_material.clone(),
        Vec3::new(0.14, DOOR_HEIGHT + 0.22, 0.24),
        Vec3::new(0.82, DOOR_HEIGHT * 0.5, 2.5),
    );
    spawn_frame_piece(
        &mut commands,
        cube.clone(),
        frame_material.clone(),
        Vec3::new(1.78, 0.16, 0.28),
        Vec3::new(0.0, DOOR_HEIGHT + 0.11, 2.5),
    );

    // Hinge barrels remain fixed to the frame.
    for y in [-0.82, 0.0, 0.82] {
        spawn_frame_piece(
            &mut commands,
            cube.clone(),
            warning_material.clone(),
            Vec3::new(0.09, 0.16, 0.12),
            Vec3::new(-0.7, DOOR_HEIGHT * 0.5 + y, 2.34),
        );
    }
}

fn spawn_test_pickup(mut commands: Commands) {
    // Only the host's root is authoritative. Clients discard their temporary
    // Startup copy and render the replicated root with a local mesh child.
    commands.spawn((
        PickupItem,
        PickupState::default(),
        RigidBody::Dynamic,
        Collider::cuboid(
            PICKUP_ITEM_HALF_EXTENT,
            PICKUP_ITEM_HALF_EXTENT,
            PICKUP_ITEM_HALF_EXTENT,
        ),
        Transform::from_xyz(0.0, 1.1, 3.8),
        LinearVelocity::default(),
        Name::new("Network Pickup"),
    ));
}

fn spawn_frame_piece(
    commands: &mut Commands,
    mesh: Handle<Mesh>,
    material: Handle<StandardMaterial>,
    size: Vec3,
    position: Vec3,
) {
    commands.spawn((
        Mesh3d(mesh),
        MeshMaterial3d(material),
        RigidBody::Static,
        Collider::cuboid(0.5, 0.5, 0.5),
        Transform::from_translation(position).with_scale(size),
    ));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_new_door_starts_closed() {
        assert!(!DoorState::default().is_open);
    }

    #[test]
    fn a_new_pickup_starts_unclaimed() {
        assert_eq!(PickupState::default().holder_id, None);
    }

    #[test]
    fn pickup_ownership_is_exclusive_by_state() {
        let mut pickup = PickupState::default();
        pickup.holder_id = Some(42);

        assert_eq!(pickup.holder_id, Some(42));
        assert_ne!(pickup.holder_id, Some(7));
    }

    #[test]
    fn held_item_offset_is_in_front_of_camera() {
        let offset = Quat::IDENTITY * HELD_ITEM_OFFSET;

        assert!(offset.z < 0.0);
        assert!(offset.x > 0.0);
        assert!(offset.y < 0.0);
    }

    #[test]
    fn door_animation_target_is_a_quarter_turn() {
        let open_rotation = Quat::from_rotation_y(FRAC_PI_2);
        let (_, yaw, _) = open_rotation.to_euler(EulerRot::XYZ);

        assert!((yaw - FRAC_PI_2).abs() < 0.0001);
    }
}
