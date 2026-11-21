use std::f32::consts::FRAC_PI_2;

use avian3d::{math::*, prelude::*};
use bevy::{
    input::{InputSystems, mouse::AccumulatedMouseMotion},
    prelude::*,
    window::{CursorGrabMode, CursorOptions, PrimaryWindow},
};

const PLAYER_RADIUS: f32 = 0.35;
const STANDING_CAPSULE_LENGTH: f32 = 1.1;
const CROUCHED_CAPSULE_LENGTH: f32 = 0.45;
const STANCE_HEIGHT_DELTA: f32 = (STANDING_CAPSULE_LENGTH - CROUCHED_CAPSULE_LENGTH) * 0.5;
const EYE_HEIGHT: f32 = 0.55;
const MOUSE_SENSITIVITY: f32 = 0.0022;
const MAX_PITCH: f32 = FRAC_PI_2 - 0.05;

pub struct PlayerPlugin;

impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PlayerInput>()
            .add_systems(Startup, (spawn_player, lock_cursor))
            .add_systems(PreUpdate, capture_player_input.after(InputSystems))
            .add_systems(Update, (handle_cursor, update_camera_look).chain())
            .add_systems(FixedUpdate, move_player);
    }
}

#[derive(Component)]
#[require(
    RigidBody::Kinematic,
    CustomPositionIntegration,
    SpeculativeCcd(0.0)
)]
struct Player;

#[derive(Component)]
struct PlayerCamera;

#[derive(Component, Default)]
struct CameraPitch(f32);

#[derive(Component, Default)]
struct PlayerStance {
    crouched: bool,
}

#[derive(Component)]
struct PlayerMovementSettings {
    walk_speed: f32,
    sprint_speed: f32,
    crouch_speed: f32,
    acceleration: f32,
    gravity: f32,
    terminal_velocity: f32,
    jump_speed: f32,
}

impl Default for PlayerMovementSettings {
    fn default() -> Self {
        Self {
            walk_speed: 3.5,
            sprint_speed: 6.0,
            crouch_speed: 1.8,
            acceleration: 24.0,
            gravity: 20.0,
            terminal_velocity: 45.0,
            jump_speed: 5.5,
        }
    }
}

#[derive(Resource, Default)]
struct PlayerInput {
    movement: Vec2,
    sprint: bool,
    crouch: bool,
    jump_pressed: bool,
}

fn spawn_player(mut commands: Commands) {
    commands
        .spawn((
            Player,
            PlayerStance::default(),
            PlayerMovementSettings::default(),
            Collider::capsule(PLAYER_RADIUS, STANDING_CAPSULE_LENGTH),
            LinearVelocity::default(),
            TransformInterpolation,
            Visibility::default(),
            Transform::from_xyz(0.0, 1.0, 5.0),
        ))
        .with_child((
            Camera3d::default(),
            Transform::from_xyz(0.0, EYE_HEIGHT, 0.0),
            PlayerCamera,
            CameraPitch::default(),
        ));

    info!(
        "Player controls: WASD move, mouse look, Shift sprint, Ctrl crouch, Space jump, Esc release cursor"
    );
}

fn lock_cursor(mut windows: Query<&mut CursorOptions, With<PrimaryWindow>>) {
    for mut cursor in &mut windows {
        cursor.grab_mode = CursorGrabMode::Locked;
        cursor.visible = false;
    }
}

fn capture_player_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut player_input: ResMut<PlayerInput>,
) {
    let horizontal = (keyboard.pressed(KeyCode::KeyD) as i8)
        - (keyboard.pressed(KeyCode::KeyA) as i8);
    let vertical = (keyboard.pressed(KeyCode::KeyW) as i8)
        - (keyboard.pressed(KeyCode::KeyS) as i8);

    player_input.movement =
        Vec2::new(horizontal as f32, vertical as f32).clamp_length_max(1.0);
    player_input.sprint = keyboard.pressed(KeyCode::ShiftLeft)
        || keyboard.pressed(KeyCode::ShiftRight);
    player_input.crouch = keyboard.pressed(KeyCode::ControlLeft)
        || keyboard.pressed(KeyCode::ControlRight)
        || keyboard.pressed(KeyCode::KeyC);
    player_input.jump_pressed = keyboard.just_pressed(KeyCode::Space);
}

fn handle_cursor(
    keyboard: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut windows: Query<&mut CursorOptions, With<PrimaryWindow>>,
) {
    for mut cursor in &mut windows {
        if keyboard.just_pressed(KeyCode::Escape) {
            cursor.grab_mode = CursorGrabMode::None;
            cursor.visible = true;
        } else if mouse.just_pressed(MouseButton::Left) {
            cursor.grab_mode = CursorGrabMode::Locked;
            cursor.visible = false;
        }
    }
}

fn update_camera_look(
    mouse_motion: Res<AccumulatedMouseMotion>,
    windows: Query<&CursorOptions, With<PrimaryWindow>>,
    mut players: Query<&mut Transform, (With<Player>, Without<PlayerCamera>)>,
    mut cameras: Query<
        (&mut Transform, &mut CameraPitch),
        (With<PlayerCamera>, Without<Player>),
    >,
) {
    let cursor_locked = windows
        .iter()
        .any(|cursor| cursor.grab_mode == CursorGrabMode::Locked);

    if !cursor_locked || mouse_motion.delta == Vec2::ZERO {
        return;
    }

    let delta = mouse_motion.delta;
    for mut player_transform in &mut players {
        let (mut yaw, _, _) = player_transform.rotation.to_euler(EulerRot::YXZ);
        yaw -= delta.x * MOUSE_SENSITIVITY;
        player_transform.rotation = Quat::from_rotation_y(yaw);
    }

    for (mut camera_transform, mut pitch) in &mut cameras {
        pitch.0 = (pitch.0 - delta.y * MOUSE_SENSITIVITY).clamp(-MAX_PITCH, MAX_PITCH);
        camera_transform.rotation = Quat::from_rotation_x(pitch.0);
    }
}

fn move_player(
    mut commands: Commands,
    time: Res<Time>,
    mut player_input: ResMut<PlayerInput>,
    mut players: Query<(
        Entity,
        &mut Transform,
        &mut LinearVelocity,
        &mut PlayerStance,
        &PlayerMovementSettings,
    ), With<Player>>,
    spatial_query: SpatialQuery,
    move_and_slide: MoveAndSlide,
) {
    let delta = time.delta_secs();

    for (entity, mut transform, mut velocity, mut stance, settings) in
        &mut players
    {
        // Resize the capsule while keeping the player's feet at the same height.
        if player_input.crouch && !stance.crouched {
            transform.translation.y -= STANCE_HEIGHT_DELTA;
            commands.entity(entity).insert(Collider::capsule(
                PLAYER_RADIUS,
                CROUCHED_CAPSULE_LENGTH,
            ));
            stance.crouched = true;
        } else if !player_input.crouch && stance.crouched {
            let crouched_half_height = PLAYER_RADIUS + CROUCHED_CAPSULE_LENGTH * 0.5;
            let head_origin = transform.translation
                + Vec3::Y * (crouched_half_height - 0.02);
            let blocked = spatial_query
                .cast_ray(
                    head_origin,
                    Dir3::Y,
                    STANDING_CAPSULE_LENGTH - CROUCHED_CAPSULE_LENGTH + 0.05,
                    true,
                    &SpatialQueryFilter::from_excluded_entities([entity]),
                )
                .is_some();

            if !blocked {
                transform.translation.y += STANCE_HEIGHT_DELTA;
                commands.entity(entity).insert(Collider::capsule(
                    PLAYER_RADIUS,
                    STANDING_CAPSULE_LENGTH,
                ));
                stance.crouched = false;
            }
        }

        let collider = Collider::capsule(
            PLAYER_RADIUS,
            if stance.crouched {
                CROUCHED_CAPSULE_LENGTH
            } else {
                STANDING_CAPSULE_LENGTH
            },
        );

        let half_height = PLAYER_RADIUS
            + if stance.crouched {
                CROUCHED_CAPSULE_LENGTH * 0.5
            } else {
                STANDING_CAPSULE_LENGTH * 0.5
            };

        // Probe just below the capsule's feet so a jump is only possible while grounded.
        let foot_origin = transform.translation - Vec3::Y * (half_height - 0.08);
        let filter = SpatialQueryFilter::from_excluded_entities([entity]);
        let grounded = spatial_query
            .cast_ray(foot_origin, Dir3::NEG_Y, 0.16, true, &filter)
            .is_some();

        if grounded && velocity.y < 0.0 {
            velocity.y = 0.0;
        } else {
            velocity.y = (velocity.y - settings.gravity * delta)
                .max(-settings.terminal_velocity);
        }

        if player_input.jump_pressed && grounded {
            velocity.y = settings.jump_speed;
        }

        let forward = *transform.forward();
        let right = *transform.right();
        let wish_direction =
            (right * player_input.movement.x + forward * player_input.movement.y)
                .normalize_or_zero();

        let max_speed = if stance.crouched {
            settings.crouch_speed
        } else if player_input.sprint {
            settings.sprint_speed
        } else {
            settings.walk_speed
        };

        let target_velocity = wish_direction * max_speed;
        let acceleration_blend = 1.0 - (-settings.acceleration * delta).exp();
        velocity.x += (target_velocity.x - velocity.x) * acceleration_blend;
        velocity.z += (target_velocity.z - velocity.z) * acceleration_blend;

        let output = move_and_slide.move_and_slide(
            &collider,
            transform.translation.real(),
            transform.rotation,
            velocity.0,
            time.delta(),
            &MoveAndSlideConfig::default(),
            &filter,
            |_| MoveAndSlideHitResponse::Accept,
        );

        transform.translation = output.position.f32();
        velocity.0 = output.projected_velocity;
    }

    // Consume the edge-triggered jump input once, even if FixedUpdate runs multiple times.
    player_input.jump_pressed = false;
}
