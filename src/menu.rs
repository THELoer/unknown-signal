use std::net::SocketAddr;

use bevy::{
    picking::hover::Hovered,
    prelude::*,
    ui::Pressed,
    ui_widgets::Button,
    window::{CursorGrabMode, CursorOptions, PrimaryWindow},
};

use crate::{
    networking::{NetworkRequest, NetworkStatus, PendingNetworkRequest},
    player::controller::GameplayActive,
};

#[derive(Component)]
struct MenuRoot;

#[derive(Component)]
struct MenuStatusText;

#[derive(Component)]
struct AddressText;

#[derive(Component, Clone, Copy)]
enum MenuAction {
    Host,
    Connect,
}

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
            .add_systems(Startup, setup_menu)
            .add_systems(
                Update,
                (
                    capture_address_input,
                    handle_menu_buttons,
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

    commands.spawn(Camera2d);
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
                    width: px(500),
                    padding: UiRect::all(px(34)),
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    ..default()
                },
                BackgroundColor(Color::srgba(0.035, 0.052, 0.064, 0.98)),
                BorderColor::all(Color::srgb(0.28, 0.39, 0.43)),
            ))
            .with_children(|panel| {
                panel.spawn((
                    Text::new("UNKNOWN SIGNAL"),
                    TextFont {
                        font_size: FontSize::Px(42.0),
                        ..default()
                    },
                    TextColor(Color::srgb(0.78, 0.86, 0.88)),
                ));
                panel.spawn((
                    Text::new("CO-OP SURVIVAL // NETWORK TEST"),
                    TextFont {
                        font_size: FontSize::Px(13.0),
                        ..default()
                    },
                    TextColor(Color::srgb(0.82, 0.55, 0.24)),
                    Node {
                        margin: UiRect::bottom(px(26)),
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
                    Text::new("Click Connect, then type digits, '.' or ':' to edit the address. Backspace deletes."),
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
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::Center,
                            margin: UiRect::vertical(px(6)),
                            ..default()
                        },
                        BackgroundColor(Color::srgb(0.16, 0.25, 0.27)),
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
                        BackgroundColor(Color::srgb(0.20, 0.22, 0.24)),
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
) {
    if menu.is_empty() {
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
    buttons: Query<&MenuAction, (With<Button>, Added<Pressed>)>,
    address: Res<AddressInput>,
    mut request: ResMut<PendingNetworkRequest>,
    mut status: ResMut<NetworkStatus>,
) {
    for action in &buttons {
        match action {
            MenuAction::Host => {
                request.0 = Some(NetworkRequest::Host);
                status.message = "Starting host server...".to_string();
            }
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
    mut buttons: Query<(&Hovered, Has<Pressed>, &mut BackgroundColor), With<MenuAction>>,
) {
    for (hovered, pressed, mut color) in &mut buttons {
        *color = if pressed {
            Color::srgb(0.34, 0.42, 0.34).into()
        } else if hovered.get() {
            Color::srgb(0.25, 0.34, 0.35).into()
        } else {
            Color::srgb(0.16, 0.23, 0.25).into()
        };
    }
}

fn close_menu_after_connection(
    mut commands: Commands,
    status: Res<NetworkStatus>,
    mut gameplay: ResMut<GameplayActive>,
    mut menu_roots: Query<Entity, With<MenuRoot>>,
    mut windows: Query<&mut CursorOptions, With<PrimaryWindow>>,
) {
    if !status.connected || gameplay.0 {
        return;
    }

    for entity in &mut menu_roots {
        commands.entity(entity).despawn();
    }
    gameplay.0 = true;

    for mut cursor in &mut windows {
        cursor.grab_mode = CursorGrabMode::Locked;
        cursor.visible = false;
    }
    info!("Entering gameplay after network connection");
}
