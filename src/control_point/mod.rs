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

/// Compute the next control point state given team ship counts and delta time.
/// Returns (new_state, scoring_team) where scoring_team is Some(team_id) if a
/// team should score this frame.
pub fn compute_next_state(
    current: &ControlPointState,
    team0_count: u32,
    team1_count: u32,
    dt: f32,
) -> (ControlPointState, Option<u8>) {
    let (majority_team, net) = if team0_count > team1_count {
        (Some(0u8), team0_count - team1_count)
    } else if team1_count > team0_count {
        (Some(1u8), team1_count - team0_count)
    } else {
        (None, 0)
    };

    let speed = capture_speed(net);

    match current {
        ControlPointState::Neutral => {
            if let Some(team) = majority_team {
                let progress = (speed * dt).min(1.0);
                if progress >= 1.0 {
                    (ControlPointState::Captured { team }, None)
                } else {
                    (ControlPointState::Capturing { team, progress }, None)
                }
            } else {
                (ControlPointState::Neutral, None)
            }
        }

        ControlPointState::Capturing { team, progress } => {
            match majority_team {
                Some(t) if t == *team => {
                    let new_progress = progress + speed * dt;
                    if new_progress >= 1.0 {
                        (ControlPointState::Captured { team: *team }, None)
                    } else {
                        (ControlPointState::Capturing { team: *team, progress: new_progress }, None)
                    }
                }
                Some(_enemy) => {
                    let new_progress = progress - speed * dt;
                    if new_progress <= 0.0 {
                        (ControlPointState::Neutral, None)
                    } else {
                        (ControlPointState::Capturing { team: *team, progress: new_progress }, None)
                    }
                }
                None => {
                    if team0_count == 0 && team1_count == 0 {
                        let new_progress = progress - DECAY_RATE * dt;
                        if new_progress <= 0.0 {
                            (ControlPointState::Neutral, None)
                        } else {
                            (ControlPointState::Capturing { team: *team, progress: new_progress }, None)
                        }
                    } else {
                        // Tied with ships present — freeze
                        (ControlPointState::Capturing { team: *team, progress: *progress }, None)
                    }
                }
            }
        }

        ControlPointState::Captured { team } => {
            match majority_team {
                Some(t) if t != *team => {
                    let new_progress = 1.0 - speed * dt;
                    (ControlPointState::Decapturing { team: *team, progress: new_progress.max(0.0) }, Some(*team))
                }
                _ => {
                    (ControlPointState::Captured { team: *team }, Some(*team))
                }
            }
        }

        ControlPointState::Decapturing { team, progress } => {
            match majority_team {
                Some(t) if t != *team => {
                    let new_progress = progress - speed * dt;
                    if new_progress <= 0.0 {
                        (ControlPointState::Neutral, None)
                    } else {
                        (ControlPointState::Decapturing { team: *team, progress: new_progress }, None)
                    }
                }
                Some(t) if t == *team => {
                    let new_progress = progress + speed * dt;
                    if new_progress >= 1.0 {
                        (ControlPointState::Captured { team: *team }, None)
                    } else {
                        (ControlPointState::Decapturing { team: *team, progress: new_progress }, None)
                    }
                }
                _ => {
                    if team0_count == 0 && team1_count == 0 {
                        let new_progress = progress - DECAY_RATE * dt;
                        if new_progress <= 0.0 {
                            (ControlPointState::Neutral, None)
                        } else {
                            (ControlPointState::Decapturing { team: *team, progress: new_progress }, None)
                        }
                    } else {
                        // Tied with ships present — freeze
                        (ControlPointState::Decapturing { team: *team, progress: *progress }, None)
                    }
                }
            }
        }
    }
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

    // ── State machine tests ──────────────────────────────────────────────

    #[test]
    fn neutral_to_capturing_with_majority() {
        let (state, _) = compute_next_state(
            &ControlPointState::Neutral,
            2, 0, 1.0,
        );
        match state {
            ControlPointState::Capturing { team, progress } => {
                assert_eq!(team, 0);
                assert!(progress > 0.07 && progress < 0.08);
            }
            _ => panic!("Expected Capturing, got {:?}", state),
        }
    }

    #[test]
    fn neutral_stays_neutral_when_tied() {
        let (state, _) = compute_next_state(
            &ControlPointState::Neutral,
            1, 1, 1.0,
        );
        assert_eq!(state, ControlPointState::Neutral);
    }

    #[test]
    fn neutral_stays_neutral_when_empty() {
        let (state, _) = compute_next_state(
            &ControlPointState::Neutral,
            0, 0, 1.0,
        );
        assert_eq!(state, ControlPointState::Neutral);
    }

    #[test]
    fn neutral_to_captured_in_one_step_with_huge_dt() {
        let (state, _) = compute_next_state(
            &ControlPointState::Neutral,
            9, 0, 100.0,
        );
        assert_eq!(state, ControlPointState::Captured { team: 0 });
    }

    #[test]
    fn capturing_completes_at_full_progress() {
        let (state, _) = compute_next_state(
            &ControlPointState::Capturing { team: 0, progress: 0.99 },
            1, 0, 1.0,
        );
        assert_eq!(state, ControlPointState::Captured { team: 0 });
    }

    #[test]
    fn capturing_freezes_when_tied() {
        let (state, _) = compute_next_state(
            &ControlPointState::Capturing { team: 0, progress: 0.5 },
            1, 1, 1.0,
        );
        assert_eq!(state, ControlPointState::Capturing { team: 0, progress: 0.5 });
    }

    #[test]
    fn capturing_decays_when_empty() {
        let (state, _) = compute_next_state(
            &ControlPointState::Capturing { team: 0, progress: 0.5 },
            0, 0, 1.0,
        );
        match state {
            ControlPointState::Capturing { team, progress } => {
                assert_eq!(team, 0);
                assert!((progress - 0.475).abs() < 1e-6);
            }
            _ => panic!("Expected Capturing, got {:?}", state),
        }
    }

    #[test]
    fn capturing_reverts_to_neutral_when_enemy_drains() {
        let (state, _) = compute_next_state(
            &ControlPointState::Capturing { team: 0, progress: 0.01 },
            0, 1, 1.0,
        );
        assert_eq!(state, ControlPointState::Neutral);
    }

    #[test]
    fn captured_to_decapturing_on_enemy_majority() {
        let (state, scoring_team) = compute_next_state(
            &ControlPointState::Captured { team: 0 },
            0, 1, 1.0,
        );
        match state {
            ControlPointState::Decapturing { team, progress } => {
                assert_eq!(team, 0);
                assert!((progress - 0.95).abs() < 1e-6);
            }
            _ => panic!("Expected Decapturing, got {:?}", state),
        }
        assert_eq!(scoring_team, Some(0));
    }

    #[test]
    fn captured_stays_captured_no_enemies() {
        let (state, scoring_team) = compute_next_state(
            &ControlPointState::Captured { team: 0 },
            0, 0, 1.0,
        );
        assert_eq!(state, ControlPointState::Captured { team: 0 });
        assert_eq!(scoring_team, Some(0));
    }

    #[test]
    fn captured_stays_captured_with_owner_present() {
        let (state, scoring_team) = compute_next_state(
            &ControlPointState::Captured { team: 0 },
            2, 0, 1.0,
        );
        assert_eq!(state, ControlPointState::Captured { team: 0 });
        assert_eq!(scoring_team, Some(0));
    }

    #[test]
    fn decapturing_to_neutral() {
        let (state, scoring) = compute_next_state(
            &ControlPointState::Decapturing { team: 0, progress: 0.01 },
            0, 1, 1.0,
        );
        assert_eq!(state, ControlPointState::Neutral);
        assert_eq!(scoring, None);
    }

    #[test]
    fn decapturing_defended_back_to_captured() {
        let (state, _) = compute_next_state(
            &ControlPointState::Decapturing { team: 0, progress: 0.99 },
            1, 0, 1.0,
        );
        assert_eq!(state, ControlPointState::Captured { team: 0 });
    }

    #[test]
    fn decapturing_freezes_when_tied() {
        let (state, _) = compute_next_state(
            &ControlPointState::Decapturing { team: 0, progress: 0.5 },
            1, 1, 1.0,
        );
        assert_eq!(state, ControlPointState::Decapturing { team: 0, progress: 0.5 });
    }

    #[test]
    fn decapturing_decays_when_empty() {
        let (state, _) = compute_next_state(
            &ControlPointState::Decapturing { team: 0, progress: 0.5 },
            0, 0, 1.0,
        );
        match state {
            ControlPointState::Decapturing { team, progress } => {
                assert_eq!(team, 0);
                assert!((progress - 0.475).abs() < 1e-6);
            }
            _ => panic!("Expected Decapturing, got {:?}", state),
        }
    }

    #[test]
    fn multi_frame_capture_accumulates() {
        let mut state = ControlPointState::Neutral;
        for _ in 0..4 {
            let (new_state, _) = compute_next_state(&state, 1, 0, 0.25);
            state = new_state;
        }
        match state {
            ControlPointState::Capturing { progress, .. } => {
                assert!((progress - 0.05).abs() < 1e-5);
            }
            _ => panic!("Expected Capturing, got {:?}", state),
        }
    }

    #[test]
    fn team1_can_capture() {
        let (state, _) = compute_next_state(
            &ControlPointState::Neutral,
            0, 3, 1.0,
        );
        match state {
            ControlPointState::Capturing { team, .. } => assert_eq!(team, 1),
            _ => panic!("Expected Capturing for team 1, got {:?}", state),
        }
    }
}
