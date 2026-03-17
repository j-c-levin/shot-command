use bevy::prelude::*;
use bevy_replicon::prelude::*;
use serde::{Deserialize, Serialize};

// ── Constants ────────────────────────────────────────────────────────────
pub const BASE_CAPTURE_TIME: f32 = 20.0;
pub const DECAY_RATE: f32 = 0.025;
pub const SCORE_VICTORY_THRESHOLD: f32 = 300.0;
pub const SCORE_TICK_RATE: f32 = 1.0;
pub const DEFAULT_ZONE_RADIUS: f32 = 100.0;

// ── Components ───────────────────────────────────────────────────────────
#[derive(Component, Clone, Debug, Serialize, Deserialize)]
pub struct ControlPoint;

#[derive(Component, Clone, Debug, Serialize, Deserialize)]
pub struct ControlPointRadius(pub f32);

#[derive(Component, Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum ControlPointState {
    Neutral,
    Capturing { team: u8, progress: f32 },
    Captured { team: u8 },
    Decapturing { team: u8, progress: f32 },
}

impl Default for ControlPointState {
    fn default() -> Self {
        Self::Neutral
    }
}

/// Scores per team. Lives as a component on the ControlPoint entity so it
/// replicates automatically via bevy_replicon.
#[derive(Component, Clone, Debug, Default, Serialize, Deserialize)]
pub struct TeamScores {
    pub scores: [f32; 2],
}

// ── Pure functions ───────────────────────────────────────────────────────

/// Compute capture speed per second given a net ship advantage.
/// Returns 0.0 if net is 0.
pub fn capture_speed(net_advantage: u32) -> f32 {
    if net_advantage == 0 {
        return 0.0;
    }
    (net_advantage as f32).sqrt() / BASE_CAPTURE_TIME
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capture_speed_zero_advantage() {
        assert_eq!(capture_speed(0), 0.0);
    }

    #[test]
    fn capture_speed_one_ship() {
        let speed = capture_speed(1);
        assert!((speed - 0.05).abs() < 1e-6);
    }

    #[test]
    fn capture_speed_four_ships() {
        let speed = capture_speed(4);
        assert!((speed - 0.1).abs() < 1e-6);
    }

    #[test]
    fn capture_speed_nine_ships() {
        let speed = capture_speed(9);
        assert!((speed - 0.15).abs() < 1e-6);
    }

    #[test]
    fn capture_speed_diminishing_returns() {
        let s1 = capture_speed(1);
        let s4 = capture_speed(4);
        let s9 = capture_speed(9);
        assert!(s4 / s1 > s9 / s4);
    }

    #[test]
    fn default_state_is_neutral() {
        assert_eq!(ControlPointState::default(), ControlPointState::Neutral);
    }

    #[test]
    fn team_scores_default_zero() {
        let scores = TeamScores::default();
        assert_eq!(scores.scores[0], 0.0);
        assert_eq!(scores.scores[1], 0.0);
    }
}
