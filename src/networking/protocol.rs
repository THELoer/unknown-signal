//! Shared Lightyear input protocol for authoritative player movement.
//!
//! The client samples its local controls and sends one compact input snapshot
//! per fixed tick. The server owns the replicated player Transform and applies
//! those snapshots. Keeping the wire type independent from Bevy's local
//! PlayerInput resource prevents UI, key bindings, and device state from ever
//! becoming network protocol details.

use bevy::ecs::entity::{EntityMapper, MapEntities};
use bevy::prelude::*;
use lightyear::prelude::input::native::InputPlugin;
use serde::{Deserialize, Serialize};

/// Input state sent by a controlling client for one simulation tick.
///
/// Movement axes are normalized to [-1, 1]. Yaw is expressed in radians and is
/// sampled from the local player root, not from the camera pitch. Jump is a
/// one-tick edge, while sprint and crouch are held states.
#[derive(
    Serialize,
    Deserialize,
    Debug,
    Default,
    PartialEq,
    Clone,
    Reflect,
)]
pub(crate) struct NetworkPlayerInput {
    pub(crate) move_x: f32,
    pub(crate) move_y: f32,
    pub(crate) yaw: f32,
    pub(crate) sprint: bool,
    pub(crate) crouch: bool,
    pub(crate) jump: bool,
}

impl NetworkPlayerInput {
    /// Returns a neutral packet while preserving a valid, finite orientation.
    pub(crate) fn neutral(yaw: f32) -> Self {
        Self {
            yaw: if yaw.is_finite() { yaw } else { 0.0 },
            ..default()
        }
    }

    /// Reject invalid floating-point input before it reaches authoritative
    /// movement. A malformed packet must never inject NaN into a Transform.
    pub(crate) fn sanitize(&mut self) {
        self.move_x = finite_axis(self.move_x);
        self.move_y = finite_axis(self.move_y);
        self.yaw = if self.yaw.is_finite() {
            self.yaw
        } else {
            0.0
        };
        let length = self.move_x.hypot(self.move_y);
        if length > 1.0 {
            self.move_x /= length;
            self.move_y /= length;
        }
    }

    pub(crate) fn has_movement(&self) -> bool {
        self.move_x.abs() > f32::EPSILON || self.move_y.abs() > f32::EPSILON
    }
}

fn finite_axis(value: f32) -> f32 {
    if value.is_finite() {
        value.clamp(-1.0, 1.0)
    } else {
        0.0
    }
}

// The input packet contains no Entity references, so there is nothing to map
// between the client and server ECS worlds. Lightyear still requires this
// implementation for its native input protocol.
impl MapEntities for NetworkPlayerInput {
    fn map_entities<M: EntityMapper>(&mut self, _entity_mapper: &mut M) {}
}

/// Registers the native input timeline and its serialization protocol.
///
/// This plugin is installed once in the shared app, before any connection
/// endpoints are created, so host and remote clients use the same wire type.
pub(crate) struct NetworkProtocolPlugin;

impl Plugin for NetworkProtocolPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(InputPlugin::<NetworkPlayerInput>::default());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn neutral_input_has_no_movement_or_actions() {
        let input = NetworkPlayerInput::neutral(1.25);
        assert_eq!(input.move_x, 0.0);
        assert_eq!(input.move_y, 0.0);
        assert_eq!(input.yaw, 1.25);
        assert!(!input.sprint);
        assert!(!input.crouch);
        assert!(!input.jump);
    }

    #[test]
    fn sanitize_clamps_axes_and_diagonal_length() {
        let mut input = NetworkPlayerInput {
            move_x: 4.0,
            move_y: 4.0,
            yaw: 0.0,
            ..default()
        };

        input.sanitize();

        assert!(input.move_x <= 1.0);
        assert!(input.move_y <= 1.0);
        assert!((input.move_x.hypot(input.move_y) - 1.0).abs() < 0.0001);
    }

    #[test]
    fn sanitize_replaces_non_finite_values() {
        let mut input = NetworkPlayerInput {
            move_x: f32::NAN,
            move_y: f32::INFINITY,
            yaw: f32::NEG_INFINITY,
            ..default()
        };

        input.sanitize();

        assert_eq!(input.move_x, 0.0);
        assert_eq!(input.move_y, 0.0);
        assert_eq!(input.yaw, 0.0);
    }
}
