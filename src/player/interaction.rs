use std::f32::consts::FRAC_PI_2;

use avian3d::prelude::*;
use bevy::prelude::*;

use super::controller::{CameraPitch, Player, PlayerCamera, EYE_HEIGHT};

const INTERACTION_DISTANCE: f32 = 3.0;
const DOOR_ANIMATION_SECONDS: f32 = 0.38;
const DOOR_WIDTH: f32 = 1.4;
const DOOR_HEIGHT: f32 = 2.2;

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
    animation_from: Quat,
    animation_to: Quat,
    animation_elapsed: f32,
}

impl Default for Door {
    fn default() -> Self {
        Self {
            is_open: false,
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
    let cube = meshes.add(Cuboid::new(1.0, 1.0, 1.0));
    let frame_material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.10, 0.14, 0.17),
        metallic: 0.72,
        perceptual_roughness: 0.48,
        ..default()
    });
    let door_material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.25, 0.31, 0.34),
        metallic: 0.58,
        perceptual_roughness: 0.62,
        ..default()
    });
    let warning_material = materials.add(Color::srgb(0.72, 0.46, 0.12));

    // The rigid body origin is the hinge, not the centre of the door leaf.
    let hinge = commands
        .spawn((
            RigidBody::Kinematic,
            Door::default(),
            Transform::from_xyz(-DOOR_WIDTH * 0.5, DOOR_HEIGHT * 0.5, 2.5),
        ))
        .id();

    commands.entity(hinge).with_children(|children| {
        // Door leaf and its collider share the same local offset from the hinge.
        children.spawn((
            Mesh3d(cube.clone()),
            MeshMaterial3d(door_material.clone()),
            Transform::from_xyz(DOOR_WIDTH * 0.5, 0.0, 0.0)
                .with_scale(Vec3::new(DOOR_WIDTH, DOOR_HEIGHT, 0.14)),
        ));
        children.spawn((
            Collider::cuboid(DOOR_WIDTH * 0.5, DOOR_HEIGHT * 0.5, 0.07),
            Transform::from_xyz(DOOR_WIDTH * 0.5, 0.0, 0.0),
        ));
        // A narrow illuminated-looking handle is a visual accent only.
        children.spawn((
            Mesh3d(cube.clone()),
            MeshMaterial3d(warning_material.clone()),
            Transform::from_xyz(DOOR_WIDTH - 0.13, 0.0, -0.085)
                .with_scale(Vec3::new(0.045, 0.28, 0.035)),
        ));
    });

    // The frame is independent of the animated rigid body and never moves.
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
        Collider::cuboid(size.x * 0.5, size.y * 0.5, size.z * 0.5),
        Transform::from_translation(position).with_scale(size),
    ));
}

fn interact_with_door(
    keyboard: Res<ButtonInput<KeyCode>>,
    players: Query<(Entity, &Transform), With<Player>>,
    cameras: Query<&CameraPitch, With<PlayerCamera>>,
    collider_bodies: Query<&ColliderOf>,
    mut doors: Query<(&Transform, &mut Door), Without<Player>>,
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

    // The ray hits the offset child collider; resolve its owning hinge body.
    let Ok(collider_of) = collider_bodies.get(hit.entity) else {
        return;
    };
    let Ok((transform, mut door)) = doors.get_mut(collider_of.body) else {
        return;
    };

    door.is_open = !door.is_open;
    door.animation_from = transform.rotation;
    door.animation_to = if door.is_open {
        Quat::from_rotation_y(FRAC_PI_2)
    } else {
        Quat::IDENTITY
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
        let progress = door.animation_elapsed / DOOR_ANIMATION_SECONDS;
        let eased = progress * progress * (3.0 - 2.0 * progress);
        transform.rotation = door.animation_from.slerp(door.animation_to, eased);
    }
}
