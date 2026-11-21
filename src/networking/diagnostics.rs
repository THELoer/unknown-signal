//! Compact in-game network diagnostics overlay.
//!
//! The overlay is intentionally local-only. It displays session state and
//! counts derived from the current Bevy world; it does not send debug traffic
//! over the network or expose private connection addresses.

use bevy::prelude::*;

use super::{NetworkDiagnostics, NetworkMode, NetworkStatus};

#[derive(Component)]
struct NetworkDiagnosticsText;

pub(crate) struct NetworkDiagnosticsHudPlugin;

impl Plugin for NetworkDiagnosticsHudPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_network_diagnostics_hud)
            .add_systems(Update, update_network_diagnostics_hud);
    }
}

fn spawn_network_diagnostics_hud(mut commands: Commands) {
    commands.spawn((
        NetworkDiagnosticsText,
        Text::new("NETWORK  ·  INITIALIZING"),
        TextFont {
            font_size: FontSize::Px(11.0),
            ..default()
        },
        TextColor(Color::srgba(0.66, 0.78, 0.79, 0.88)),
        Node {
            position_type: PositionType::Absolute,
            right: px(18),
            top: px(18),
            padding: UiRect::axes(px(10), px(7)),
            ..default()
        },
        BackgroundColor(Color::srgba(0.015, 0.025, 0.032, 0.68)),
        ZIndex(20),
    ));
}

fn update_network_diagnostics_hud(
    status: Res<NetworkStatus>,
    diagnostics: Res<NetworkDiagnostics>,
    mut labels: Query<&mut Text, With<NetworkDiagnosticsText>>,
) {
    if !status.is_changed() && !diagnostics.is_changed() {
        return;
    }

    let mode = match status.mode {
        Some(NetworkMode::Host) => "HOST",
        Some(NetworkMode::Client) => "CLIENT",
        None => "OFFLINE",
    };
    let state = if status.connected {
        "CONNECTED"
    } else if status.requested {
        "CONNECTING"
    } else {
        "IDLE"
    };

    let error_line = status
        .last_error
        .as_deref()
        .map(|error| format!("\nLAST EVENT  {error}"))
        .unwrap_or_default();

    let value = format!(
        "NETWORK  /  {mode}  /  {state}\nCLIENT LINKS  {:02}\nSERVER PEERS  {:02}\nREMOTE AVATARS  {:02}{}",
        diagnostics.connected_clients,
        diagnostics.connected_server_peers,
        diagnostics.remote_player_avatars,
        error_line,
    );

    for mut label in &mut labels {
        **label = value.clone();
    }
}
