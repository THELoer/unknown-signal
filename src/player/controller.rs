use std::f32::consts::FRAC_PI_2;

use avian3d::{math::*, prelude::*};
use bevy::{
    camera::Hdr,
    input::{InputSystems, mouse::AccumulatedMouseMotion},
    picking::hover::Hovered,
    post_process::effect_stack::{ChromaticAberration, Vignette},
    prelude::*,
    ui::Pressed,
    ui_widgets::Button,
    window::{CursorGrabMode, CursorOptions, PrimaryWindow},
};

const PLAYER_RADIUS: f32 = 0.35;
const STANDING_CAPSULE_LENGTH: f32 = 1.1;
const CROUCHED_CAPSULE_LENGTH: f32 = 0.45;
const STANCE_HEIGHT_DELTA: f32 = (STANDING_CAPSULE_LENGTH - CROUCHED_CAPSULE_LENGTH) * 0.5;
pub(super) const EYE_HEIGHT: f32 = 0.55;
const MAX_PITCH: f32 = FRAC_PI_2 - 0.05;
const STAMINA_MAX: f32 = 100.0;
const SPRINT_DRAIN_PER_SECOND: f32 = 26.0;
const STAMINA_REGEN_PER_SECOND: f32 = 18.0;
const STAMINA_REGEN_DELAY: f32 = 0.85;
const INTERACTION_INPUT_BUFFER_SECONDS: f32 = 0.20;

#[derive(Resource, Default)]
pub struct GameplayActive(pub bool);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ControlAction {
    MoveForward,
    MoveBackward,
    MoveLeft,
    MoveRight,
    Sprint,
    Crouch,
    Jump,
    Interact,
    Flashlight,
}

#[derive(Resource)]
pub struct PlayerSettings {
    pub mouse_sensitivity: f32,
    pub invert_mouse_y: bool,
    pub move_forward: KeyCode,
    pub move_backward: KeyCode,
    pub move_left: KeyCode,
    pub move_right: KeyCode,
    pub sprint: KeyCode,
    pub crouch: KeyCode,
    pub jump: KeyCode,
    pub interact: KeyCode,
    pub flashlight: KeyCode,
}

impl Default for PlayerSettings {
    fn default() -> Self {
        Self {
            mouse_sensitivity: 0.003,
            invert_mouse_y: false,
            move_forward: KeyCode::KeyW,
            move_backward: KeyCode::KeyS,
            move_left: KeyCode::KeyA,
            move_right: KeyCode::KeyD,
            sprint: KeyCode::ShiftLeft,
            crouch: KeyCode::ControlLeft,
            jump: KeyCode::Space,
            interact: KeyCode::KeyE,
            flashlight: KeyCode::KeyF,
        }
    }
}

impl PlayerSettings {
    pub fn binding(&self, action: ControlAction) -> KeyCode {
        match action {
            ControlAction::MoveForward => self.move_forward,
            ControlAction::MoveBackward => self.move_backward,
            ControlAction::MoveLeft => self.move_left,
            ControlAction::MoveRight => self.move_right,
            ControlAction::Sprint => self.sprint,
            ControlAction::Crouch => self.crouch,
            ControlAction::Jump => self.jump,
            ControlAction::Interact => self.interact,
            ControlAction::Flashlight => self.flashlight,
        }
    }

    pub fn set_binding(&mut self, action: ControlAction, key: KeyCode) {
        match action {
            ControlAction::MoveForward => self.move_forward = key,
            ControlAction::MoveBackward => self.move_backward = key,
            ControlAction::MoveLeft => self.move_left = key,
            ControlAction::MoveRight => self.move_right = key,
            ControlAction::Sprint => self.sprint = key,
            ControlAction::Crouch => self.crouch = key,
            ControlAction::Jump => self.jump = key,
            ControlAction::Interact => self.interact = key,
            ControlAction::Flashlight => self.flashlight = key,
        }
    }
}

pub struct PlayerPlugin;

impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PlayerInput>()
            .init_resource::<GameplayActive>()
            .init_resource::<PlayerSettings>()
            .init_resource::<PlayerStamina>()
            .add_systems(Startup, (spawn_player, spawn_stamina_hud))
            .add_systems(
                PreUpdate,
                (
                    toggle_pause,
                    capture_player_input,
                    update_camera_look,
                    toggle_flashlight,
                )
                    .chain()
                    .after(InputSystems),
            )
            .add_systems(FixedUpdate, move_player)
            .add_systems(
                Update,
                (resume_from_pause_button, update_camera_stance, update_stamina_hud),
            );
    }
}

#[derive(Component)]
#[require(
    RigidBody::Kinematic,
    CustomPositionIntegration,
    SpeculativeCcd::new(0.0)
)]
pub struct Player;

#[derive(Component)]
pub(super) struct PlayerCamera;

#[derive(Component, Default)]
pub(super) struct CameraPitch(pub(super) f32);

#[derive(Component)]
struct Flashlight {
    enabled: bool,
}

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

#[derive(Resource)]
struct PlayerStamina {
    current: f32,
    regen_cooldown: f32,
}

impl Default for PlayerStamina {
    fn default() -> Self {
        Self {
            current: STAMINA_MAX,
            regen_cooldown: 0.0,
        }
    }
}

#[derive(Component)]
struct StaminaHud;

fn spawn_stamina_hud(mut commands: Commands) {
    commands.spawn((
        StaminaHud,
        Text::new("STAMINA  100%"),
        TextFont {
            font_size: FontSize::Px(14.0),
            ..default()
        },
        TextColor(Color::srgb(0.68, 0.84, 0.78)),
        Node {
            position_type: PositionType::Absolute,
            left: px(24),
            top: px(24),
            ..default()
        },
    ));
}

fn update_stamina_hud(stamina: Res<PlayerStamina>, mut labels: Query<&mut Text, With<StaminaHud>>) {
    let percent = (stamina.current / STAMINA_MAX * 100.0).round() as u32;
    for mut label in &mut labels {
        **label = format!("STAMINA  {percent:>3}%");
    }
}

#[derive(Resource, Default)]
pub(crate) struct PlayerInput {
    pub(crate) movement: Vec2,
    pub(crate) sprint: bool,
    pub(crate) crouch: bool,
    pub(crate) jump_pressed: bool,
    pub(crate) interact_pressed: bool,
    pub(crate) interaction_buffer_seconds: f32,
    pub(crate) pitch: f32,
}

fn spawn_player(mut commands: Commands) {
    commands
        .spawn((
            Player,
            PlayerStance::default(),
            PlayerMovementSettings::default(),
            Collider::capsule(PLAYER_RADIUS, STANDING_CAPSULE_LENGTH),
            LinearVelocity::default(),
            TranslationInterpolation,
            Visibility::default(),
            InheritedVisibility::default(),
            Transform::from_xyz(0.0, 1.0, 5.0),
        ))
        .with_child((
            Camera3d::default(),
            Hdr,
            ChromaticAberration {
                intensity: 0.004,
                max_samples: 8,
                ..default()
            },
            Vignette {
                intensity: 0.14,
                radius: 0.86,
                smoothness: 0.72,
                roundness: 1.0,
                center: Vec2::splat(0.5),
                edge_compensation: 0.0,
                color: Color::BLACK,
            },
            Transform::from_xyz(0.0, EYE_HEIGHT, 0.0),
            PlayerCamera,
            CameraPitch::default(),
            AmbientLight {
                color: Color::srgb(0.42, 0.50, 0.66),
                brightness: 260.0,
                ..default()
            },
            DistanceFog {
                color: Color::srgb(0.035, 0.045, 0.065),
                falloff: FogFalloff::Linear {
                    start: 14.0,
                    end: 48.0,
                },
                ..default()
            },
        ))
        // Keep the spotlight on its own entity: Bevy warns that sharing an entity
        // with Camera replaces the camera frustum with the spotlight frustum.
        .with_child((
            SpotLight {
                color: Color::srgb(0.78, 0.86, 1.0),
                intensity: 0.0,
                range: 32.0,
                radius: 0.04,
                inner_angle: 0.22,
                outer_angle: 0.58,
                shadow_maps_enabled: true,
                ..default()
            },
            Flashlight { enabled: false },
            Transform::default(),
        ));

    info!(
        "Player controls: WASD move, mouse look, Shift sprint, Ctrl crouch, Space jump, E interact, Esc release cursor"
    );
}

fn update_camera_stance(
    time: Res<Time>,
    players: Query<&PlayerStance, With<Player>>,
    mut cameras: Query<&mut Transform, With<PlayerCamera>>,
) {
    let Ok(stance) = players.single() else {
        return;
    };

    let target_height = if stance.crouched {
        EYE_HEIGHT - STANCE_HEIGHT_DELTA
    } else {
        EYE_HEIGHT
    };
    let blend = 1.0 - (-12.0 * time.delta_secs()).exp();

    for mut transform in &mut cameras {
        transform.translation.y += (target_height - transform.translation.y) * blend;
    }
}

fn capture_player_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    settings: Res<PlayerSettings>,
    active: Res<GameplayActive>,
    mut player_input: ResMut<PlayerInput>,
    cameras: Query<&CameraPitch, With<PlayerCamera>>,
) {
    if !active.0 {
        *player_input = PlayerInput::default();
        return;
    }

    let horizontal = (keyboard.pressed(settings.move_right) as i8)
        - (keyboard.pressed(settings.move_left) as i8);
    let vertical = (keyboard.pressed(settings.move_forward) as i8)
        - (keyboard.pressed(settings.move_backward) as i8);

    player_input.movement = Vec2::new(horizontal as f32, vertical as f32).clamp_length_max(1.0);
    player_input.sprint = keyboard.pressed(settings.sprint);
    player_input.crouch = keyboard.pressed(settings.crouch);
    player_input.jump_pressed = keyboard.just_pressed(settings.jump);
    if keyboard.just_pressed(settings.interact) {
        player_input.interaction_buffer_seconds = INTERACTION_INPUT_BUFFER_SECONDS;
    }
    player_input.interaction_buffer_seconds =
        (player_input.interaction_buffer_seconds - time.delta_secs()).max(0.0);
    player_input.interact_pressed = player_input.interaction_buffer_seconds > 0.0;
    player_input.pitch = cameras.single().map(|camera| camera.0).unwrap_or(0.0);
}

#[derive(Component)]
struct PauseOverlay;

#[derive(Component)]
struct ResumePauseButton;

/// Escape pauses only this local player's controls and UI. The networking
/// plugin and authoritative server simulation keep running for other peers.
fn toggle_pause(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut gameplay: ResMut<GameplayActive>,
    mut commands: Commands,
    overlays: Query<Entity, With<PauseOverlay>>,
    mut windows: Query<&mut CursorOptions, With<PrimaryWindow>>,
) {
    if !keyboard.just_pressed(KeyCode::Escape) {
        return;
    }

    if gameplay.0 {
        gameplay.0 = false;
        commands
            .spawn((
                PauseOverlay,
                GlobalZIndex(100),
                Node {
                    position_type: PositionType::Absolute,
                    width: percent(100),
                    height: percent(100),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    ..default()
                },
                BackgroundColor(Color::srgba(0.008, 0.014, 0.020, 0.88)),
            ))
            .with_children(|root| {
                root.spawn((
                    Node {
                        width: px(360),
                        padding: UiRect::all(px(30)),
                        row_gap: px(22),
                        flex_direction: FlexDirection::Column,
                        align_items: AlignItems::Center,
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.025, 0.040, 0.050)),
                ))
                .with_children(|panel| {
                    panel.spawn((
                        Text::new("PAUSED"),
                        TextFont { font_size: FontSize::Px(32.0), ..default() },
                        TextColor(Color::srgb(0.86, 0.91, 0.91)),
                    ));
                    panel.spawn((
                        Button,
                        Hovered::default(),
                        ResumePauseButton,
                        Node {
                            width: percent(100),
                            padding: UiRect::axes(px(22), px(13)),
                            justify_content: JustifyContent::Center,
                            ..default()
                        },
                        BackgroundColor(Color::srgb(0.10, 0.25, 0.24)),
                    ))
                    .with_child((
                        Text::new("RESUME  ·  ESC"),
                        TextFont { font_size: FontSize::Px(16.0), ..default() },
                        TextColor(Color::srgb(0.88, 0.93, 0.91)),
                    ));
                });
            });

        for mut cursor in &mut windows {
            cursor.grab_mode = CursorGrabMode::None;
            cursor.visible = true;
        }
        info!("Gameplay paused locally");
    } else if !overlays.is_empty() {
        gameplay.0 = true;
        for entity in &overlays {
            commands.entity(entity).despawn();
        }
        for mut cursor in &mut windows {
            cursor.grab_mode = CursorGrabMode::Locked;
            cursor.visible = false;
        }
        info!("Gameplay resumed locally");
    }
}

fn resume_from_pause_button(
    buttons: Query<(), (With<ResumePauseButton>, Added<Pressed>)>,
    mut gameplay: ResMut<GameplayActive>,
    mut commands: Commands,
    overlays: Query<Entity, With<PauseOverlay>>,
    mut windows: Query<&mut CursorOptions, With<PrimaryWindow>>,
) {
    if buttons.is_empty() {
        return;
    }

    gameplay.0 = true;
    for entity in &overlays {
        commands.entity(entity).despawn();
    }
    for mut cursor in &mut windows {
        cursor.grab_mode = CursorGrabMode::Locked;
        cursor.visible = false;
    }
    info!("Gameplay resumed from pause menu");
}

fn update_camera_look(
    settings: Res<PlayerSettings>,
    mouse_motion: Res<AccumulatedMouseMotion>,
    windows: Query<&CursorOptions, With<PrimaryWindow>>,
    active: Res<GameplayActive>,
    mut players: Query<&mut Transform, (With<Player>, Without<PlayerCamera>)>,
    mut cameras: Query<(&mut Transform, &mut CameraPitch), (With<PlayerCamera>, Without<Player>)>,
) {
    if !active.0 {
        return;
    }

    let cursor_locked = windows
        .iter()
        .any(|cursor| cursor.grab_mode == CursorGrabMode::Locked);

    if !cursor_locked || mouse_motion.delta == Vec2::ZERO {
        return;
    }

    let delta = mouse_motion.delta;
    for mut player_transform in &mut players {
        let (mut yaw, _, _) = player_transform.rotation.to_euler(EulerRot::YXZ);
        yaw -= delta.x * settings.mouse_sensitivity;
        player_transform.rotation = Quat::from_rotation_y(yaw);
    }

    for (mut camera_transform, mut pitch) in &mut cameras {
        let pitch_sign = if settings.invert_mouse_y { 1.0 } else { -1.0 };
        pitch.0 = (pitch.0 + delta.y * settings.mouse_sensitivity * pitch_sign)
            .clamp(-MAX_PITCH, MAX_PITCH);
        camera_transform.rotation = Quat::from_rotation_x(pitch.0);
    }
}

fn move_player(
    mut commands: Commands,
    time: Res<Time>,
    active: Res<GameplayActive>,
    mut player_input: ResMut<PlayerInput>,
    mut stamina: ResMut<PlayerStamina>,
    mut players: Query<
        (
            Entity,
            &mut Transform,
            &mut LinearVelocity,
            &mut PlayerStance,
            &PlayerMovementSettings,
        ),
        With<Player>,
    >,
    move_and_slide: MoveAndSlide,
) {
    if !active.0 {
        return;
    }

    let delta = time.delta_secs();

    for (entity, mut transform, mut velocity, mut stance, settings) in &mut players {
        // Resize the capsule while keeping the player's feet at the same height.
        if player_input.crouch && !stance.crouched {
            transform.translation.y -= STANCE_HEIGHT_DELTA;
            commands
                .entity(entity)
                .insert(Collider::capsule(PLAYER_RADIUS, CROUCHED_CAPSULE_LENGTH));
            stance.crouched = true;
        } else if !player_input.crouch && stance.crouched {
            let crouched_half_height = PLAYER_RADIUS + CROUCHED_CAPSULE_LENGTH * 0.5;
            let head_origin = transform.translation + Vec3::Y * (crouched_half_height - 0.02);
            let blocked = move_and_slide
                .spatial_query
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
                commands
                    .entity(entity)
                    .insert(Collider::capsule(PLAYER_RADIUS, STANDING_CAPSULE_LENGTH));
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
        let foot_origin = transform.translation - Vec3::Y * (half_height - 0.05);
        let filter = SpatialQueryFilter::from_excluded_entities([entity]);
        let grounded = move_and_slide
            .spatial_query
            .cast_ray(foot_origin, Dir3::NEG_Y, 0.25, true, &filter)
            .is_some();

        if grounded && velocity.y < 0.0 {
            velocity.y = 0.0;
        } else {
            velocity.y = (velocity.y - settings.gravity * delta).max(-settings.terminal_velocity);
        }

        if player_input.jump_pressed && grounded {
            velocity.y = settings.jump_speed;
        }

        let forward = *transform.forward();
        let right = *transform.right();
        let wish_direction = (right * player_input.movement.x + forward * player_input.movement.y)
            .normalize_or_zero();

        let sprinting = player_input.sprint
            && !stance.crouched
            && player_input.movement != Vec2::ZERO
            && stamina.current > 0.0;
        if sprinting {
            stamina.current = (stamina.current - SPRINT_DRAIN_PER_SECOND * delta).max(0.0);
            stamina.regen_cooldown = STAMINA_REGEN_DELAY;
        } else if stamina.regen_cooldown > 0.0 {
            stamina.regen_cooldown = (stamina.regen_cooldown - delta).max(0.0);
        } else {
            stamina.current = (stamina.current + STAMINA_REGEN_PER_SECOND * delta).min(STAMINA_MAX);
        }

        let max_speed = if stance.crouched {
            settings.crouch_speed
        } else if sprinting {
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

fn toggle_flashlight(
    settings: Res<PlayerSettings>,
    keyboard: Res<ButtonInput<KeyCode>>,
    active: Res<GameplayActive>,
    mut flashlights: Query<(&mut SpotLight, &mut Flashlight)>,
) {
    if !active.0 || !keyboard.just_pressed(settings.flashlight) {
        return;
    }

    for (mut light, mut flashlight) in &mut flashlights {
        flashlight.enabled = !flashlight.enabled;
        light.intensity = if flashlight.enabled { 30_000.0 } else { 0.0 };
        info!(
            "Flashlight {}",
            if flashlight.enabled { "on" } else { "off" }
        );
    }
}
