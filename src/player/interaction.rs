use std::f32::consts::FRAC_PI_2;

use avian3d::prelude::*;
use bevy::prelude::*;

use super::controller::{Player, PlayerCamera, PlayerSettings};
use crate::player::GameplayActive;

const INTERACTION_DISTANCE: f32 = 3.0;
const DOOR_ANIMATION_SECONDS: f32 = 0.38;
const DOOR_WIDTH: f32 = 1.4;
const DOOR_HEIGHT: f32 = 2.2;

const PICKUP_ITEM_HALF_EXTENT: f32 = 0.22;
const HELD_ITEM_OFFSET: Vec3 = Vec3::new(0.28, -0.22, -0.85);
const DROP_DISTANCE: f32 = 1.35;

#[derive(Component)]
struct PickupItem;

#[derive(Component)]
struct InteractionPromptText;

#[derive(Component)]
struct HeldItemText;

#[derive(Resource, Default)]
struct HeldItem(Option<Entity>);

pub struct InteractionPlugin;

impl Plugin for InteractionPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<HeldItem>()
            .add_systems(Startup, (spawn_test_door, spawn_test_pickup, spawn_interaction_hud))
            .add_systems(
                Update,
                (update_interaction_hud, interact_with_door, animate_doors).chain(),
            );
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

fn update_interaction_hud(
    active: Res<GameplayActive>,
    players: Query<Entity, With<Player>>,
    cameras: Query<&GlobalTransform, With<PlayerCamera>>,
    items: Query<Entity, With<PickupItem>>,
    collider_bodies: Query<&ColliderOf>,
    doors: Query<(), With<Door>>,
    spatial_query: SpatialQuery,
    held_item: Res<HeldItem>,
    mut prompt_text: Query<&mut Text, (With<InteractionPromptText>, Without<HeldItemText>)>,
    mut held_text: Query<&mut Text, (With<HeldItemText>, Without<InteractionPromptText>)>,
) {
    let mut prompt = String::new();
    if active.0 {
        if let (Ok(player), Ok(camera)) = (players.single(), cameras.single()) {
            let transform = camera.compute_transform();
            let filter = SpatialQueryFilter::from_excluded_entities([player]);
            if let Some(hit) = spatial_query.cast_ray(
                transform.translation,
                transform.forward(),
                INTERACTION_DISTANCE,
                true,
                &filter,
            ) {
                if items.get(hit.entity).is_ok() {
                    prompt = "E  —  PICK UP".to_string();
                } else if let Ok(collider_of) = collider_bodies.get(hit.entity) {
                    if doors.get(collider_of.body).is_ok() {
                        prompt = "E  —  OPEN / CLOSE".to_string();
                    }
                }
            }
        }
    }

    if held_item.0.is_some() {
        for mut text in &mut held_text {
            **text = "CARRYING ITEM  ·  E TO DROP".to_string();
        }
    } else {
        for mut text in &mut held_text {
            **text = String::new();
        }
    }

    for mut text in &mut prompt_text {
        **text = prompt.clone();
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
            Visibility::default(),
            Transform::from_xyz(-DOOR_WIDTH * 0.5, DOOR_HEIGHT * 0.5, 2.5),
        ))
        .id();

    commands.entity(hinge).with_children(|children| {
        // Door leaf and its collider share the same local offset from the hinge.
        children.spawn((
            Mesh3d(cube.clone()),
            MeshMaterial3d(door_material.clone()),
            Transform::from_xyz(DOOR_WIDTH * 0.5, 0.0, 0.0).with_scale(Vec3::new(
                DOOR_WIDTH,
                DOOR_HEIGHT,
                0.14,
            )),
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

fn spawn_test_pickup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let mesh = meshes.add(Cuboid::new(0.44, 0.44, 0.44));
    let material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.34, 0.25, 0.15),
        metallic: 0.28,
        perceptual_roughness: 0.88,
        ..default()
    });

    commands.spawn((
        PickupItem,
        Mesh3d(mesh),
        MeshMaterial3d(material),
        RigidBody::Dynamic,
        Collider::cuboid(
            PICKUP_ITEM_HALF_EXTENT,
            PICKUP_ITEM_HALF_EXTENT,
            PICKUP_ITEM_HALF_EXTENT,
        ),
        Transform::from_xyz(0.0, 1.1, 3.8),
        LinearVelocity::default(),
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

fn interact_with_door(
    mut commands: Commands,
    active: Res<GameplayActive>,
    settings: Res<PlayerSettings>,
    keyboard: Res<ButtonInput<KeyCode>>,
    players: Query<Entity, With<Player>>,
    cameras: Query<(Entity, &GlobalTransform), With<PlayerCamera>>,
    items: Query<Entity, With<PickupItem>>,
    collider_bodies: Query<&ColliderOf>,
    mut doors: Query<(&Transform, &mut Door), Without<Player>>,
    spatial_query: SpatialQuery,
    mut held_item: ResMut<HeldItem>,
) {
    if !active.0 || !keyboard.just_pressed(settings.interact) {
        return;
    }

    let Ok(player_entity) = players.single() else {
        return;
    };
    let Ok((camera_entity, camera_global)) = cameras.single() else {
        return;
    };
    let camera_transform = camera_global.compute_transform();

    // Pressing E while carrying an object drops it in front of the player.
    if let Some(item_entity) = held_item.0.take() {
        let drop_position =
            camera_transform.translation + *camera_transform.forward() * DROP_DISTANCE;
        commands.entity(item_entity).remove::<ChildOf>().insert((
            RigidBody::Dynamic,
            Collider::cuboid(
                PICKUP_ITEM_HALF_EXTENT,
                PICKUP_ITEM_HALF_EXTENT,
                PICKUP_ITEM_HALF_EXTENT,
            ),
            LinearVelocity::default(),
            Transform::from_translation(drop_position).with_rotation(camera_transform.rotation),
        ));
        info!("Dropped item");
        return;
    }

    let filter = SpatialQueryFilter::from_excluded_entities([player_entity]);
    let Some(hit) = spatial_query.cast_ray(
        camera_transform.translation,
        camera_transform.forward(),
        INTERACTION_DISTANCE,
        true,
        &filter,
    ) else {
        return;
    };

    // Pickup targets take priority over doors when the ray hits an item.
    if items.get(hit.entity).is_ok() {
        commands
            .entity(hit.entity)
            .remove::<RigidBody>()
            .remove::<Collider>()
            .remove::<LinearVelocity>()
            .insert((
                ChildOf(camera_entity),
                Transform::from_translation(HELD_ITEM_OFFSET),
            ));
        held_item.0 = Some(hit.entity);
        info!("Picked up item");
        return;
    }

    // Door leaf colliders are children; resolve their owning hinge body.
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

    info!("Door {}", if door.is_open { "opened" } else { "closed" });
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
