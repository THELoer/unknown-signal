use bevy::prelude::*;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(WorldScenePlugin)
        .run();
}

/// Owns the first playable development scene.
struct WorldScenePlugin;

impl Plugin for WorldScenePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_test_scene);
    }
}

fn spawn_test_scene(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // Industrial test chamber floor.
    commands.spawn((
        Mesh3d(meshes.add(Plane3d::default().mesh().size(20.0, 20.0))),
        MeshMaterial3d(materials.add(Color::srgb(0.12, 0.15, 0.18))),
        Transform::default(),
    ));

    // A few simple forms make scale, lighting, and depth immediately visible.
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(2.0, 2.0, 2.0))),
        MeshMaterial3d(materials.add(Color::srgb(0.35, 0.42, 0.48))),
        Transform::from_xyz(0.0, 1.0, 0.0),
    ));

    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(1.0, 3.0, 1.0))),
        MeshMaterial3d(materials.add(Color::srgb(0.28, 0.34, 0.39))),
        Transform::from_xyz(-4.0, 1.5, -2.0),
    ));

    commands.spawn((
        Mesh3d(meshes.add(Sphere::new(0.8).mesh().uv(32, 18))),
        MeshMaterial3d(materials.add(Color::srgb(0.55, 0.22, 0.18))),
        Transform::from_xyz(3.0, 0.8, -2.0),
    ));

    commands.spawn((
        PointLight {
            intensity: 4_000.0,
            range: 30.0,
            shadow_maps_enabled: true,
            ..default()
        },
        Transform::from_xyz(2.0, 7.0, 4.0),
    ));

    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(-9.0, 7.0, 12.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
}
