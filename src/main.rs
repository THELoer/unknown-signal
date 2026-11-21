mod player;

use avian3d::prelude::{Collider, PhysicsPlugins, RigidBody};
use bevy::prelude::*;

fn main() {
    App::new()
        .add_plugins((DefaultPlugins, PhysicsPlugins::default()))
        .add_plugins((WorldScenePlugin, player::PlayerPlugin, player::InteractionPlugin))
        .run();
}

/// Owns the industrial greybox used to validate movement, lighting and interaction.
struct WorldScenePlugin;

impl Plugin for WorldScenePlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(AmbientLight {
            color: Color::srgb(0.42, 0.50, 0.66),
            brightness: 22.0,
            ..default()
        })
        .add_systems(Startup, spawn_test_scene);
    }
}

fn spawn_test_scene(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let cube = meshes.add(Cuboid::new(1.0, 1.0, 1.0));

    let floor = materials.add(StandardMaterial {
        base_color: Color::srgb(0.075, 0.085, 0.10),
        metallic: 0.22,
        perceptual_roughness: 0.86,
        ..default()
    });
    let wall = materials.add(StandardMaterial {
        base_color: Color::srgb(0.17, 0.20, 0.23),
        metallic: 0.32,
        perceptual_roughness: 0.78,
        ..default()
    });
    let trim = materials.add(StandardMaterial {
        base_color: Color::srgb(0.065, 0.085, 0.105),
        metallic: 0.68,
        perceptual_roughness: 0.52,
        ..default()
    });
    let panel = materials.add(StandardMaterial {
        base_color: Color::srgb(0.24, 0.29, 0.32),
        metallic: 0.58,
        perceptual_roughness: 0.66,
        ..default()
    });
    let hazard = materials.add(StandardMaterial {
        base_color: Color::srgb(0.72, 0.43, 0.10),
        metallic: 0.18,
        perceptual_roughness: 0.74,
        ..default()
    });
    let crate_material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.22, 0.25, 0.25),
        metallic: 0.38,
        perceptual_roughness: 0.9,
        ..default()
    });
    let lamp_material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.40, 0.52, 0.62),
        metallic: 0.12,
        perceptual_roughness: 0.34,
        ..default()
    });

    // Main room: 16 x 16 metres, with a low industrial ceiling.
    spawn_box(&mut commands, cube.clone(), floor.clone(), Vec3::new(16.0, 0.2, 16.0), Vec3::new(0.0, -0.1, 0.0), true);
    spawn_box(&mut commands, cube.clone(), wall.clone(), Vec3::new(16.0, 5.0, 0.25), Vec3::new(0.0, 2.5, -8.0), true);
    spawn_box(&mut commands, cube.clone(), wall.clone(), Vec3::new(16.0, 5.0, 0.25), Vec3::new(0.0, 2.5, 8.0), true);
    spawn_box(&mut commands, cube.clone(), wall.clone(), Vec3::new(0.25, 5.0, 16.0), Vec3::new(-8.0, 2.5, 0.0), true);
    spawn_box(&mut commands, cube.clone(), wall.clone(), Vec3::new(0.25, 5.0, 16.0), Vec3::new(8.0, 2.5, 0.0), true);
    spawn_box(&mut commands, cube.clone(), trim.clone(), Vec3::new(16.0, 0.22, 16.0), Vec3::new(0.0, 5.0, 0.0), true);

    // A partition wall makes the door a real passage instead of a floating prop.
    spawn_box(&mut commands, cube.clone(), wall.clone(), Vec3::new(7.0, 5.0, 0.25), Vec3::new(-4.5, 2.5, 2.5), true);
    spawn_box(&mut commands, cube.clone(), wall.clone(), Vec3::new(7.0, 5.0, 0.25), Vec3::new(4.5, 2.5, 2.5), true);
    spawn_box(&mut commands, cube.clone(), wall.clone(), Vec3::new(2.0, 2.7, 0.25), Vec3::new(0.0, 3.65, 2.5), true);

    // Heavy structural ribs and wall-mounted service panels.
    for z in [-6.0, -2.0, 2.0, 6.0] {
        spawn_box(&mut commands, cube.clone(), trim.clone(), Vec3::new(16.0, 0.16, 0.20), Vec3::new(0.0, 4.78, z), false);
    }
    for x in [-7.65, -4.0, 4.0, 7.65] {
        spawn_box(&mut commands, cube.clone(), trim.clone(), Vec3::new(0.16, 4.8, 0.28), Vec3::new(x, 2.5, -7.82), false);
    }
    for z in [-5.5, -1.5, 4.5] {
        spawn_box(&mut commands, cube.clone(), panel.clone(), Vec3::new(0.12, 1.35, 1.7), Vec3::new(-7.82, 2.7, z), false);
        spawn_box(&mut commands, cube.clone(), trim.clone(), Vec3::new(0.14, 0.08, 1.8), Vec3::new(-7.72, 3.38, z), false);
        spawn_box(&mut commands, cube.clone(), panel.clone(), Vec3::new(0.12, 1.35, 1.7), Vec3::new(7.82, 2.7, z), false);
    }

    // Yellow safety markings along the room edges and around the doorway.
    for x in [-7.35, 7.35] {
        spawn_box(&mut commands, cube.clone(), hazard.clone(), Vec3::new(0.07, 0.012, 15.0), Vec3::new(x, 0.012, 0.0), false);
    }
    for x in [-0.95, 0.95] {
        spawn_box(&mut commands, cube.clone(), hazard.clone(), Vec3::new(0.045, 0.018, 0.55), Vec3::new(x, 0.02, 2.5), false);
    }

    // A few chunky equipment crates give the room readable scale.
    spawn_box(&mut commands, cube.clone(), crate_material.clone(), Vec3::new(1.25, 1.0, 1.0), Vec3::new(-3.2, 0.5, -1.0), true);
    spawn_box(&mut commands, cube.clone(), trim.clone(), Vec3::new(1.28, 0.10, 1.03), Vec3::new(-3.2, 1.02, -1.0), false);
    spawn_box(&mut commands, cube.clone(), crate_material.clone(), Vec3::new(0.85, 0.75, 0.85), Vec3::new(3.8, 0.375, -4.8), true);
    spawn_box(&mut commands, cube.clone(), panel.clone(), Vec3::new(1.0, 0.12, 0.12), Vec3::new(3.8, 0.45, -4.8), false);

    // Ceiling fixtures use non-shadow-casting point lights to keep the greybox inexpensive.
    for (x, z, color) in [
        (-4.5, -4.5, Color::srgb(0.68, 0.80, 1.0)),
        (4.5, -4.5, Color::srgb(0.68, 0.80, 1.0)),
        (-4.5, 4.5, Color::srgb(0.95, 0.70, 0.42)),
        (4.5, 4.5, Color::srgb(0.68, 0.80, 1.0)),
    ] {
        spawn_box(&mut commands, cube.clone(), lamp_material.clone(), Vec3::new(1.35, 0.08, 0.42), Vec3::new(x, 4.82, z), false);
        commands.spawn((
            PointLight {
                color,
                intensity: 850.0,
                range: 9.0,
                radius: 0.18,
                shadow_maps_enabled: false,
                ..default()
            },
            Transform::from_xyz(x, 4.55, z),
        ));
    }

    // Retain one physics test object, now styled as a low industrial console.
    spawn_box(&mut commands, cube.clone(), panel.clone(), Vec3::new(1.5, 1.0, 0.8), Vec3::new(0.0, 0.5, -0.2), true);
    spawn_box(&mut commands, cube.clone(), hazard.clone(), Vec3::new(1.2, 0.045, 0.04), Vec3::new(0.0, 1.02, -0.62), false);

    commands.spawn((
        Mesh3d(meshes.add(Sphere::new(0.55).mesh().uv(24, 12))),
        MeshMaterial3d(materials.add(Color::srgb(0.42, 0.19, 0.15))),
        RigidBody::Dynamic,
        Collider::sphere(0.55),
        Transform::from_xyz(3.0, 3.0, -2.0),
    ));
}

fn spawn_box(
    commands: &mut Commands,
    cube: Handle<Mesh>,
    material: Handle<StandardMaterial>,
    size: Vec3,
    position: Vec3,
    solid: bool,
) {
    let mut entity = commands.spawn((
        Mesh3d(cube),
        MeshMaterial3d(material),
        Transform::from_translation(position).with_scale(size),
    ));

    if solid {
        // The unit collider is scaled by the same Transform as the unit cube mesh.
        entity.insert((RigidBody::Static, Collider::cuboid(0.5, 0.5, 0.5)));
    }
}
