use std::net::SocketAddr;

use bevy::{
    picking::hover::Hovered,
    prelude::*,
    ui::Pressed,
    ui_widgets::{Button, Slider, SliderRange, SliderThumb, SliderValue, TrackClick},
    window::{CursorGrabMode, CursorOptions, PrimaryWindow},
};

use crate::{
    networking::{NetworkRequest, NetworkStatus, PendingNetworkRequest},
    player::{ControlAction, GameplayActive, PlayerSettings},
};

#[derive(Component)]
struct MenuRoot;

#[derive(Component)]
struct MenuCamera;

#[derive(Component)]
struct MenuStatusText;

#[derive(Component)]
struct AddressText;

#[derive(Component, Clone, Copy)]
enum MenuAction {
    Host,
    Connect,
    OpenSettings,
    CloseSettings,
    EditSensitivity,
    ToggleInvertY,
    Rebind(ControlAction),
}

#[derive(Component)]
struct SettingsOverlay;

#[derive(Component, Clone, Copy)]
struct BindingLabel(ControlAction);

#[derive(Component)]
struct SensitivityLabel;

#[derive(Component)]
struct SensitivitySlider;

#[derive(Resource, Default)]
struct EditingSensitivity(bool);

#[derive(Resource, Default)]
struct SensitivityBuffer(String);

#[derive(Component)]
struct InvertYLabel;

#[derive(Component)]
struct RebindHint;

#[derive(Resource, Default)]
struct PendingRebind(Option<ControlAction>);

#[derive(Resource)]
struct AddressInput(String);

impl Default for AddressInput {
    fn default() -> Self {
        Self("127.0.0.1:5000".to_string())
    }
}

pub struct MainMenuPlugin;

impl Plugin for MainMenuPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<AddressInput>()
            .init_resource::<PendingRebind>()
            .init_resource::<EditingSensitivity>()
            .init_resource::<SensitivityBuffer>()
            .add_systems(Startup, setup_menu)
            .add_systems(
                Update,
                (
                    capture_address_input,
                    handle_menu_buttons,
                    capture_rebind,
                    capture_sensitivity_input,
                    sync_sensitivity_controls,
                    update_settings_text,
                    update_menu_text,
                    update_button_appearance,
                    close_menu_after_connection,
                )
                    .chain(),
            );
    }
}

fn setup_menu(
    mut commands: Commands,
    mut windows: Query<&mut CursorOptions, With<PrimaryWindow>>,
) {
    for mut cursor in &mut windows {
        cursor.grab_mode = CursorGrabMode::None;
        cursor.visible = true;
    }

    commands.spawn((Camera2d, Camera { order: 10, ..default() }, MenuCamera));
    commands
        .spawn((
            MenuRoot,
            Node {
                width: percent(100),
                height: percent(100),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(0.012, 0.020, 0.028, 0.97)),
        ))
        .with_children(|root| {
            root.spawn((
                Node {
                    width: px(560),
                    max_width: percent(92),
                    padding: UiRect::all(px(42)),
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Stretch,
                    row_gap: px(5),
                    border: UiRect::all(px(1)),
                    ..default()
                },
                BorderRadius::all(px(8)),
                BackgroundColor(Color::srgba(0.025, 0.040, 0.050, 0.985)),
                BorderColor::all(Color::srgb(0.22, 0.34, 0.37)),
            ))
            .with_children(|panel| {
                panel.spawn((
                    Text::new("UNKNOWN SIGNAL"),
                    TextFont {
                        font_size: FontSize::Px(46.0),
                        ..default()
                    },
                    TextColor(Color::srgb(0.88, 0.93, 0.91)),
                ));
                panel.spawn((
                    Text::new("CO-OP SURVIVAL  /  DEEP SPACE DIVISION"),
                    TextFont {
                        font_size: FontSize::Px(11.0),
                        ..default()
                    },
                    TextColor(Color::srgb(0.87, 0.55, 0.29)),
                    Node {
                        margin: UiRect::bottom(px(30)),
                        ..default()
                    },
                ));

                panel.spawn((
                    Text::new("SERVER ADDRESS"),
                    TextFont {
                        font_size: FontSize::Px(14.0),
                        ..default()
                    },
                    TextColor(Color::srgb(0.56, 0.68, 0.71)),
                ));
                panel.spawn((
                    AddressText,
                    Text::new("127.0.0.1:5000"),
                    TextFont {
                        font_size: FontSize::Px(19.0),
                        ..default()
                    },
                    TextColor(Color::srgb(0.91, 0.93, 0.91)),
                    Node {
                        width: percent(100),
                        justify_content: JustifyContent::Center,
                        padding: UiRect::all(px(12)),
                        margin: UiRect::vertical(px(8)),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.07, 0.09, 0.10)),
                ));
                panel.spawn((
                    Text::new("Edit address with Backspace, digits, '.' and ':', then connect."),
                    TextFont {
                        font_size: FontSize::Px(11.0),
                        ..default()
                    },
                    TextColor(Color::srgb(0.48, 0.57, 0.60)),
                    Node {
                        margin: UiRect::bottom(px(18)),
                        ..default()
                    },
                ));

                panel
                    .spawn((
                        Button,
                        Hovered::default(),
                        MenuAction::Host,
                        Node {
                            width: percent(100),
                            height: px(58),
                            border: UiRect::all(px(1)),
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::Center,
                            margin: UiRect::vertical(px(6)),
                            ..default()
                        },
                        BorderRadius::all(px(4)),
                        BorderColor::all(Color::srgb(0.32, 0.48, 0.47)),
                        BackgroundColor(Color::srgb(0.10, 0.25, 0.24)),
                    ))
                    .with_children(|button| {
                        button.spawn((
                            Text::new("HOST SERVER"),
                            TextFont {
                                font_size: FontSize::Px(20.0),
                                ..default()
                            },
                            TextColor(Color::srgb(0.90, 0.94, 0.91)),
                        ));
                    });

                panel
                    .spawn((
                        Button,
                        Hovered::default(),
                        MenuAction::Connect,
                        Node {
                            width: percent(100),
                            height: px(58),
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::Center,
                            margin: UiRect::vertical(px(6)),
                            ..default()
                        },
                        BorderRadius::all(px(4)),
                        BorderColor::all(Color::srgb(0.28, 0.37, 0.39)),
                        BackgroundColor(Color::srgb(0.10, 0.15, 0.17)),
                    ))
                    .with_children(|button| {
                        button.spawn((
                            Text::new("CONNECT SERVER"),
                            TextFont {
                                font_size: FontSize::Px(20.0),
                                ..default()
                            },
                            TextColor(Color::srgb(0.90, 0.94, 0.91)),
                        ));
                    });


                panel
                    .spawn((
                        Button,
                        Hovered::default(),
                        MenuAction::OpenSettings,
                        Node {
                            width: percent(100),
                            height: px(46),
                            border: UiRect::all(px(1)),
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::Center,
                            margin: UiRect::vertical(px(6)),
                            ..default()
                        },
                        BorderRadius::all(px(4)),
                        BorderColor::all(Color::srgb(0.24, 0.34, 0.36)),
                        BackgroundColor(Color::srgb(0.07, 0.11, 0.13)),
                    ))
                    .with_children(|button| {
                        button.spawn((
                            Text::new("SETTINGS"),
                            TextFont { font_size: FontSize::Px(17.0), ..default() },
                            TextColor(Color::srgb(0.82, 0.88, 0.88)),
                        ));
                    });

                panel.spawn((
                    MenuStatusText,
                    Text::new("Select Host Server or Connect Server"),
                    TextFont {
                        font_size: FontSize::Px(13.0),
                        ..default()
                    },
                    TextColor(Color::srgb(0.75, 0.80, 0.80)),
                    Node {
                        margin: UiRect::top(px(22)),
                        ..default()
                    },
                ));
            });
        });
}

fn capture_address_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut address: ResMut<AddressInput>,
    menu: Query<(), With<MenuRoot>>,
    overlays: Query<(), With<SettingsOverlay>>,
) {
    if menu.is_empty() || !overlays.is_empty() {
        return;
    }

    if keyboard.just_pressed(KeyCode::Backspace) {
        address.0.pop();
    }

    for (key, character) in [
        (KeyCode::Digit0, '0'),
        (KeyCode::Digit1, '1'),
        (KeyCode::Digit2, '2'),
        (KeyCode::Digit3, '3'),
        (KeyCode::Digit4, '4'),
        (KeyCode::Digit5, '5'),
        (KeyCode::Digit6, '6'),
        (KeyCode::Digit7, '7'),
        (KeyCode::Digit8, '8'),
        (KeyCode::Digit9, '9'),
        (KeyCode::Numpad0, '0'),
        (KeyCode::Numpad1, '1'),
        (KeyCode::Numpad2, '2'),
        (KeyCode::Numpad3, '3'),
        (KeyCode::Numpad4, '4'),
        (KeyCode::Numpad5, '5'),
        (KeyCode::Numpad6, '6'),
        (KeyCode::Numpad7, '7'),
        (KeyCode::Numpad8, '8'),
        (KeyCode::Numpad9, '9'),
    ] {
        if keyboard.just_pressed(key) && address.0.len() < 48 {
            address.0.push(character);
        }
    }

    if keyboard.just_pressed(KeyCode::Period) && address.0.len() < 48 {
        address.0.push('.');
    }
    if keyboard.just_pressed(KeyCode::Semicolon) && address.0.len() < 48 {
        address.0.push(':');
    }
}

fn handle_menu_buttons(
    mut commands: Commands,
    buttons: Query<&MenuAction, (With<Button>, Added<Pressed>)>,
    address: Res<AddressInput>,
    mut request: ResMut<PendingNetworkRequest>,
    mut status: ResMut<NetworkStatus>,
    mut settings: ResMut<PlayerSettings>,
    mut pending_rebind: ResMut<PendingRebind>,
    mut editing_sensitivity: ResMut<EditingSensitivity>,
    mut sensitivity_buffer: ResMut<SensitivityBuffer>,
    overlays: Query<Entity, With<SettingsOverlay>>,
) {
    for action in &buttons {
        match action {
            MenuAction::Host => {
                request.0 = Some(NetworkRequest::Host);
                status.message = "Starting host server...".to_string();
            }
            MenuAction::OpenSettings => {
                if overlays.is_empty() {
                    spawn_settings_overlay(&mut commands, &settings);
                }
            }
            MenuAction::CloseSettings => {
                for entity in &overlays {
                    commands.entity(entity).despawn();
                }
                pending_rebind.0 = None;
                editing_sensitivity.0 = false;
            }
            MenuAction::EditSensitivity => {
                editing_sensitivity.0 = !editing_sensitivity.0;
                if editing_sensitivity.0 {
                    sensitivity_buffer.0 = format!("{:.4}", settings.mouse_sensitivity);
                }
            },
            MenuAction::ToggleInvertY => settings.invert_mouse_y = !settings.invert_mouse_y,
            MenuAction::Rebind(action) => pending_rebind.0 = Some(*action),
            MenuAction::Connect => match address.0.parse::<SocketAddr>() {
                Ok(server_addr) => {
                    request.0 = Some(NetworkRequest::Connect(server_addr));
                    status.message = format!("Connecting to {server_addr}...");
                }
                Err(error) => {
                    status.message = format!("Invalid server address: {error}");
                }
            },
        }
    }
}

fn update_menu_text(
    address: Res<AddressInput>,
    status: Res<NetworkStatus>,
    mut address_text: Query<&mut Text, (With<AddressText>, Without<MenuStatusText>)>,
    mut status_text: Query<&mut Text, (With<MenuStatusText>, Without<AddressText>)>,
) {
    if address.is_changed() {
        for mut text in &mut address_text {
            **text = address.0.clone();
        }
    }
    if status.is_changed() {
        for mut text in &mut status_text {
            **text = status.message.clone();
        }
    }
}

fn update_button_appearance(
    mut buttons: Query<(&MenuAction, &Hovered, Has<Pressed>, &mut BackgroundColor), With<Button>>,
) {
    for (action, hovered, pressed, mut color) in &mut buttons {
        let (normal, hot, down) = match action {
            MenuAction::Host => (
                Color::srgb(0.10, 0.25, 0.24),
                Color::srgb(0.14, 0.36, 0.32),
                Color::srgb(0.07, 0.18, 0.17),
            ),
            MenuAction::Connect => (
                Color::srgb(0.10, 0.15, 0.17),
                Color::srgb(0.17, 0.25, 0.27),
                Color::srgb(0.07, 0.11, 0.13),
            ),
            _ => (
                Color::srgb(0.07, 0.11, 0.13),
                Color::srgb(0.15, 0.23, 0.24),
                Color::srgb(0.05, 0.08, 0.09),
            ),
        };
        *color = if pressed { down } else if hovered.get() { hot } else { normal }.into();
    }
}

fn close_menu_after_connection(
    mut commands: Commands,
    status: Res<NetworkStatus>,
    mut gameplay: ResMut<GameplayActive>,
    mut menu_roots: Query<Entity, With<MenuRoot>>,
    mut menu_cameras: Query<Entity, With<MenuCamera>>,
    mut windows: Query<&mut CursorOptions, With<PrimaryWindow>>,
) {
    if !status.connected || gameplay.0 {
        return;
    }

    for entity in &mut menu_roots {
        commands.entity(entity).despawn();
    }
    for entity in &mut menu_cameras {
        commands.entity(entity).despawn();
    }
    gameplay.0 = true;

    for mut cursor in &mut windows {
        cursor.grab_mode = CursorGrabMode::Locked;
        cursor.visible = false;
    }
    info!("Entering gameplay after network connection");
}


fn spawn_settings_overlay(commands: &mut Commands, settings: &PlayerSettings) {
    commands.spawn((
        SettingsOverlay,
        GlobalZIndex(20),
        Node {
            position_type: PositionType::Absolute,
            width: percent(100),
            height: percent(100),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            ..default()
        },
        BackgroundColor(Color::srgba(0.008, 0.014, 0.020, 0.985)),
    )).with_children(|overlay| {
        overlay.spawn((
            Node {
                width: px(680),
                max_width: percent(94),
                max_height: percent(94),
                overflow: Overflow::scroll_y(),
                padding: UiRect::all(px(32)),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Stretch,
                row_gap: px(7),
                ..default()
            },
            BorderRadius::all(px(8)),
            BackgroundColor(Color::srgb(0.025, 0.040, 0.050)),
            BorderColor::all(Color::srgb(0.22, 0.34, 0.37)),
        )).with_children(|panel| {
            panel.spawn((
                Text::new("SYSTEM SETTINGS"),
                TextFont { font_size: FontSize::Px(30.0), ..default() },
                TextColor(Color::srgb(0.82, 0.89, 0.90)),
            ));
            panel.spawn((
                Text::new("MOUSE"),
                TextFont { font_size: FontSize::Px(14.0), ..default() },
                TextColor(Color::srgb(0.82, 0.55, 0.24)),
                Node { margin: UiRect::top(px(8)), ..default() },
            ));
            panel.spawn((
                Node {
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::SpaceBetween,
                    margin: UiRect::bottom(px(5)),
                    ..default()
                },
            )).with_children(|row| {
                row.spawn((
                    Text::new("LOOK SENSITIVITY"),
                    TextFont { font_size: FontSize::Px(13.0), ..default() },
                    TextColor(Color::srgb(0.66, 0.76, 0.77)),
                ));
                row.spawn((
                    Button,
                    Hovered::default(),
                    MenuAction::EditSensitivity,
                    Node {
                        min_width: px(104),
                        height: px(34),
                        padding: UiRect::axes(px(9), px(4)),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.09, 0.14, 0.16)),
                )).with_children(|button| {
                    button.spawn((
                        SensitivityLabel,
                        Text::new(format!("{:.4}", settings.mouse_sensitivity)),
                        TextFont { font_size: FontSize::Px(15.0), ..default() },
                        TextColor(Color::srgb(0.94, 0.68, 0.34)),
                    ));
                });
            });
            panel.spawn((
                SensitivitySlider,
                Slider { track_click: TrackClick::Snap, ..default() },
                SliderValue(settings.mouse_sensitivity),
                SliderRange::new(0.0005, 0.01),
                Node {
                    width: percent(100),
                    height: px(22),
                    justify_content: JustifyContent::Center,
                    margin: UiRect::bottom(px(4)),
                    ..default()
                },
                Hovered::default(),
            )).with_children(|slider| {
                slider.spawn((
                    Node {
                        width: percent(100),
                        height: px(5),
                        border_radius: BorderRadius::MAX,
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.13, 0.20, 0.21)),
                ));
                slider.spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        left: px(0),
                        right: px(12),
                        top: px(0),
                        bottom: px(0),
                        ..default()
                    },
                )).with_children(|track| {
                    track.spawn((
                        SliderThumb,
                        Node {
                            position_type: PositionType::Absolute,
                            left: percent(((settings.mouse_sensitivity - 0.0005) / 0.0095 * 100.0).clamp(0.0, 100.0)),
                            width: px(14),
                            height: px(14),
                            top: px(4),
                            border_radius: BorderRadius::MAX,
                            ..default()
                        },
                        BackgroundColor(Color::srgb(0.91, 0.61, 0.27)),
                    ));
                });
            });
            panel.spawn((
                Text::new("Drag the slider or click the value to type a number."),
                TextFont { font_size: FontSize::Px(10.0), ..default() },
                TextColor(Color::srgb(0.47, 0.58, 0.60)),
                Node { margin: UiRect::bottom(px(8)), ..default() },
            ));
            panel.spawn((
                Button,
                Hovered::default(),
                MenuAction::ToggleInvertY,
                Node {
                    width: percent(100),
                    min_height: px(34),
                    padding: UiRect::axes(px(10), px(5)),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    ..default()
                },
                BackgroundColor(Color::srgb(0.14, 0.21, 0.23)),
            )).with_children(|button| {
                button.spawn((
                    InvertYLabel,
                    Text::new(if settings.invert_mouse_y { "INVERT Y: ON" } else { "INVERT Y: OFF" }),
                    TextFont { font_size: FontSize::Px(13.0), ..default() },
                    TextColor(Color::srgb(0.88, 0.91, 0.89)),
                ));
            });
            panel.spawn((
                Text::new("KEYBOARD"),
                TextFont { font_size: FontSize::Px(14.0), ..default() },
                TextColor(Color::srgb(0.82, 0.55, 0.24)),
                Node { margin: UiRect::top(px(8)), ..default() },
            ));
            for (action, label) in [
                (ControlAction::MoveForward, "MOVE FORWARD"),
                (ControlAction::MoveBackward, "MOVE BACKWARD"),
                (ControlAction::MoveLeft, "MOVE LEFT"),
                (ControlAction::MoveRight, "MOVE RIGHT"),
                (ControlAction::Sprint, "SPRINT"),
                (ControlAction::Crouch, "CROUCH"),
                (ControlAction::Jump, "JUMP"),
                (ControlAction::Interact, "INTERACT / PICK UP"),
                (ControlAction::Flashlight, "FLASHLIGHT"),
            ] {
                spawn_binding_button(panel, action, label, settings.binding(action));
            }
            panel.spawn((
                RebindHint,
                Text::new("Select an action to rebind it. Press Escape to cancel."),
                TextFont { font_size: FontSize::Px(11.0), ..default() },
                TextColor(Color::srgb(0.58, 0.68, 0.70)),
                Node { margin: UiRect::vertical(px(8)), ..default() },
            ));
            spawn_action_button(panel, "BACK", MenuAction::CloseSettings);
        });
    });
}

fn spawn_action_button(parent: &mut ChildSpawnerCommands, label: &str, action: MenuAction) {
    parent.spawn((
        Button,
        Hovered::default(),
        action,
        Node {
            min_width: px(42),
            min_height: px(34),
            border: UiRect::all(px(1)),
            padding: UiRect::axes(px(10), px(5)),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            margin: UiRect::vertical(px(2)),
            ..default()
        },
        BorderRadius::all(px(4)),
        BorderColor::all(Color::srgb(0.24, 0.37, 0.39)),
        BackgroundColor(Color::srgb(0.10, 0.17, 0.19)),
    )).with_children(|button| {
        button.spawn((
            Text::new(label.to_string()),
            TextFont { font_size: FontSize::Px(13.0), ..default() },
            TextColor(Color::srgb(0.88, 0.91, 0.89)),
        ));
    });
}

fn spawn_binding_button(
    parent: &mut ChildSpawnerCommands,
    action: ControlAction,
    label: &str,
    key: KeyCode,
) {
    parent.spawn((
        Button,
        Hovered::default(),
        MenuAction::Rebind(action),
        Node {
            width: percent(100),
            min_height: px(28),
            padding: UiRect::axes(px(10), px(4)),
            justify_content: JustifyContent::SpaceBetween,
            align_items: AlignItems::Center,
            margin: UiRect::vertical(px(1)),
            ..default()
        },
        BorderRadius::all(px(3)),
        BorderColor::all(Color::srgb(0.16, 0.25, 0.27)),
        BackgroundColor(Color::srgb(0.055, 0.09, 0.105)),
    )).with_children(|button| {
        button.spawn((
            Text::new(label.to_string()),
            TextFont { font_size: FontSize::Px(12.0), ..default() },
            TextColor(Color::srgb(0.72, 0.80, 0.81)),
        ));
        button.spawn((
            BindingLabel(action),
            Text::new(format!("{key:?}")),
            TextFont { font_size: FontSize::Px(12.0), ..default() },
            TextColor(Color::srgb(0.92, 0.68, 0.34)),
        ));
    });
}

fn capture_rebind(
    keyboard: Res<ButtonInput<KeyCode>>,
    overlays: Query<(), With<SettingsOverlay>>,
    mut pending: ResMut<PendingRebind>,
    mut settings: ResMut<PlayerSettings>,
) {
    if overlays.is_empty() {
        pending.0 = None;
        return;
    }
    let Some(action) = pending.0 else { return; };
    if keyboard.just_pressed(KeyCode::Escape) {
        pending.0 = None;
        return;
    }
    const KEYS: &[KeyCode] = &[
        KeyCode::KeyA, KeyCode::KeyB, KeyCode::KeyC, KeyCode::KeyD,
        KeyCode::KeyE, KeyCode::KeyF, KeyCode::KeyG, KeyCode::KeyH,
        KeyCode::KeyI, KeyCode::KeyJ, KeyCode::KeyK, KeyCode::KeyL,
        KeyCode::KeyM, KeyCode::KeyN, KeyCode::KeyO, KeyCode::KeyP,
        KeyCode::KeyQ, KeyCode::KeyR, KeyCode::KeyS, KeyCode::KeyT,
        KeyCode::KeyU, KeyCode::KeyV, KeyCode::KeyW, KeyCode::KeyX,
        KeyCode::KeyY, KeyCode::KeyZ, KeyCode::Space, KeyCode::ShiftLeft,
        KeyCode::ControlLeft, KeyCode::AltLeft, KeyCode::Tab, KeyCode::CapsLock,
        KeyCode::ArrowUp, KeyCode::ArrowDown, KeyCode::ArrowLeft, KeyCode::ArrowRight,
        KeyCode::Digit0, KeyCode::Digit1, KeyCode::Digit2, KeyCode::Digit3,
        KeyCode::Digit4, KeyCode::Digit5, KeyCode::Digit6, KeyCode::Digit7,
        KeyCode::Digit8, KeyCode::Digit9,
    ];
    for &key in KEYS {
        if keyboard.just_pressed(key) {
            settings.set_binding(action, key);
            pending.0 = None;
            break;
        }
    }
}

fn update_settings_text(
    settings: Res<PlayerSettings>,
    editing_sensitivity: Res<EditingSensitivity>,
    sensitivity_buffer: Res<SensitivityBuffer>,
    pending: Res<PendingRebind>,
    mut bindings: Query<(&BindingLabel, &mut Text)>,
    mut sensitivity: Query<&mut Text, (With<SensitivityLabel>, Without<BindingLabel>)>,
    mut invert: Query<&mut Text, (With<InvertYLabel>, Without<BindingLabel>, Without<SensitivityLabel>)>,
    mut hint: Query<&mut Text, (With<RebindHint>, Without<BindingLabel>, Without<SensitivityLabel>, Without<InvertYLabel>)>,
) {
    for (label, mut text) in &mut bindings {
        **text = if pending.0 == Some(label.0) {
            "PRESS A KEY...".to_string()
        } else {
            format!("{:?}", settings.binding(label.0))
        };
    }
    for mut text in &mut sensitivity {
        **text = format!("SENSITIVITY  {:.4}", settings.mouse_sensitivity);
    }
    for mut text in &mut invert {
        **text = if settings.invert_mouse_y { "INVERT Y: ON" } else { "INVERT Y: OFF" }.to_string();
    }
    for mut text in &mut hint {
        **text = if pending.0.is_some() {
            "Press a key to assign it. Escape cancels.".to_string()
        } else {
            "Select an action to rebind it.".to_string()
        };
    }
}


fn capture_sensitivity_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut editing: ResMut<EditingSensitivity>,
    mut buffer: ResMut<SensitivityBuffer>,
    mut settings: ResMut<PlayerSettings>,
) {
    if !editing.0 { return; }
    if keyboard.just_pressed(KeyCode::Escape) {
        editing.0 = false;
        return;
    }
    if keyboard.just_pressed(KeyCode::Enter) || keyboard.just_pressed(KeyCode::NumpadEnter) {
        if let Ok(value) = buffer.0.parse::<f32>() {
            if value.is_finite() {
                settings.mouse_sensitivity = value.clamp(0.0005, 0.01);
            }
        }
        editing.0 = false;
        return;
    }
    if keyboard.just_pressed(KeyCode::Backspace) {
        buffer.0.pop();
    }
    for (key, character) in [
        (KeyCode::Digit0, '0'), (KeyCode::Digit1, '1'),
        (KeyCode::Digit2, '2'), (KeyCode::Digit3, '3'),
        (KeyCode::Digit4, '4'), (KeyCode::Digit5, '5'),
        (KeyCode::Digit6, '6'), (KeyCode::Digit7, '7'),
        (KeyCode::Digit8, '8'), (KeyCode::Digit9, '9'),
        (KeyCode::Numpad0, '0'), (KeyCode::Numpad1, '1'),
        (KeyCode::Numpad2, '2'), (KeyCode::Numpad3, '3'),
        (KeyCode::Numpad4, '4'), (KeyCode::Numpad5, '5'),
        (KeyCode::Numpad6, '6'), (KeyCode::Numpad7, '7'),
        (KeyCode::Numpad8, '8'), (KeyCode::Numpad9, '9'),
    ] {
        if keyboard.just_pressed(key) && buffer.0.len() < 8 {
            buffer.0.push(character);
        }
    }
    if (keyboard.just_pressed(KeyCode::Period) || keyboard.just_pressed(KeyCode::NumpadDecimal))
        && !buffer.0.contains('.') && buffer.0.len() < 8
    {
        buffer.0.push('.');
    }
}

fn sync_sensitivity_controls(
    mut settings: ResMut<PlayerSettings>,
    mut sliders: Query<&mut SliderValue, With<SensitivitySlider>>,
    mut thumbs: Query<&mut Node, With<SliderThumb>>,
) {
    for mut slider in &mut sliders {
        let clamped = slider.0.clamp(0.0005, 0.01);
        if (clamped - settings.mouse_sensitivity).abs() > f32::EPSILON {
            settings.mouse_sensitivity = clamped;
        }
        slider.0 = settings.mouse_sensitivity;
    }
    let position = ((settings.mouse_sensitivity - 0.0005) / 0.0095 * 100.0).clamp(0.0, 100.0);
    for mut node in &mut thumbs {
        node.left = percent(position);
    }
}
