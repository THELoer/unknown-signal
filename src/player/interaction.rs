use std::f32::consts::FRAC_PI_2;

use avian3d::prelude::*;
use bevy::prelude::*;

use super::controller::{CameraPitch, Player, PlayerCamera, EYE_HEIGHT};

const INTERACTION_DISTANCE: f32 = 3.0;
const DOOR_ANIMATION_SECONDS: f32 = 0.22;

pub struct InteractionPlugin;

impl Plugin for InteractionPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_test_door)
            .add_systems(Update, (interact_with_door, animate_doors).chain());
    }
}

#[derive(Component)]
struct Door {
    is_open: bool,
    closed_rotation: Quat,
    open_rotation: Quat,
    animation_from: Quat,
    animation_to: Quat,
    animation_elapsed: f32,
}

impl Default for Door {
    fn default() -> Self {
        Self {
            is_open: false,
            closed_rotation: Quat::IDENTITY,
            open_rotation: Quat::from_rotation_y(FRAC_PI_2),
            animation_from: Quat::IDENTITY,
            animation_to: Quat::IDENTITY,
            animation_elapsed: DOOR_ANIMATION_SECONDS,
        }
    }
}

fn spawn_test_door(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(1.4, 2.2, 0.16))),
        MeshMaterial3d(materials.add(Color::srgb(0.22, 0.30, 0.34))),
        RigidBody::Kinematic,
        Collider::cuboid(0.7, 1.1, 0.08),
        Door::default(),
        Transform::from_xyz(0.0, 1.1, 2.5),
    ));
}

fn interact_with_door(
    keyboard: Res<ButtonInput<KeyCode>>,
    players: Query<(Entity, &Transform), With<Player>>,
    cameras: Query<&CameraPitch, With<PlayerCamera>>,
    mut doors: Query<(&mut Transform, &mut Door), Without<Player>>,
    spatial_query: SpatialQuery,
) {
    if !keyboard.just_pressed(KeyCode::KeyE) {
        return;
    }

    let Ok((player_entity, player_transform)) = players.single() else {
        return;
    };
    let Ok(camera_pitch) = cameras.single() else {
        return;
    };

    let camera_rotation =
        player_transform.rotation * Quat::from_rotation_x(camera_pitch.0);
    let direction = camera_rotation * Dir3::NEG_Z;
    let origin = player_transform.translation + Vec3::Y * EYE_HEIGHT;
    let filter = SpatialQueryFilter::from_excluded_entities([player_entity]);

    let Some(hit) = spatial_query.cast_ray(
        origin,
        direction,
        INTERACTION_DISTANCE,
        true,
        &filter,
    ) else {
        return;
    };

    let Ok((mut transform, mut door)) = doors.get_mut(hit.entity) else {
        return;
    };

    door.is_open = !door.is_open;
    door.animation_from = transform.rotation;
    door.animation_to = if door.is_open {
        door.open_rotation
    } else {
        door.closed_rotation
    };
    door.animation_elapsed = 0.0;

    info!(
        "Door {}",
        if door.is_open { "opened" } else { "closed" }
    );
}

fn animate_doors(time: Res<Time>, mut doors: Query<(&mut Transform, &mut Door)>) {
    for (mut transform, mut door) in &mut doors {
        if door.animation_elapsed >= DOOR_ANIMATION_SECONDS {
            continue;
        }

        door.animation_elapsed =
            (door.animation_elapsed + time.delta_secs()).min(DOOR_ANIMATION_SECONDS);
        let linear_progress = door.animation_elapsed / DOOR_ANIMATION_SECONDS;
        let eased_progress = linear_progress * linear_progress * (3.0 - 2.0 * linear_progress);

        transform.rotation =
            door.animation_from.slerp(door.animation_to, eased_progress);
    }
}
