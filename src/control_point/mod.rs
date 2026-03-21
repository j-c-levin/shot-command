use std::collections::HashMap;

use bevy::prelude::*;
use bevy_replicon::prelude::*;
use serde::{Deserialize, Serialize};

use crate::game::{Destroyed, GameConfig, GameState, Team};
use crate::net::commands::GameResult;
use crate::net::LocalTeam;
use crate::ship::Ship;

// ── Constants ────────────────────────────────────────────────────────────
pub const BASE_CAPTURE_TIME: f32 = 7.0;
pub const DECAY_RATE: f32 = 0.025;
pub const SCORE_VICTORY_THRESHOLD: f32 = 30.0;
pub const SCORE_TICK_RATE: f32 = 1.0;
pub const DEFAULT_ZONE_RADIUS: f32 = 100.0;

// ── Components ───────────────────────────────────────────────────────────
#[derive(Component, Clone, Debug, Serialize, Deserialize)]
pub struct ControlPoint;

#[derive(Component, Clone, Debug, Serialize, Deserialize)]
pub struct ControlPointRadius(pub f32);

#[derive(Component, Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub enum ControlPointState {
    #[default]
    Neutral,
    Capturing { team: u8, progress: f32 },
    Captured { team: u8 },
    Decapturing { team: u8, progress: f32 },
}

/// Scores per team. Lives as a component on the ControlPoint entity so it
/// replicates automatically via bevy_replicon.
#[derive(Component, Clone, Debug, Default, Serialize, Deserialize)]
pub struct TeamScores {
    pub scores: HashMap<u8, f32>,
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

/// Find the team with plurality (strictly more ships than any other team).
/// Returns (plurality_team, net_advantage) where net_advantage is top - second.
/// Returns (None, 0) if tied or empty.
pub fn find_plurality(team_counts: &HashMap<u8, u32>) -> (Option<u8>, u32) {
    let mut counts: Vec<(u8, u32)> = team_counts
        .iter()
        .map(|(&team, &count)| (team, count))
        .filter(|(_, count)| *count > 0)
        .collect();
    counts.sort_by_key(|b| std::cmp::Reverse(b.1));

    match counts.as_slice() {
        [] => (None, 0),
        [(team, count)] => (Some(*team), *count),
        [(team1, count1), (_, count2), ..] => {
            if count1 > count2 {
                (Some(*team1), count1 - count2)
            } else {
                (None, 0)
            }
        }
    }
}

/// Compute the next control point state given team ship counts and delta time.
/// Returns (new_state, scoring_team) where scoring_team is Some(team_id) if a
/// team should score this frame.
pub fn compute_next_state(
    current: &ControlPointState,
    team_counts: &HashMap<u8, u32>,
    dt: f32,
) -> (ControlPointState, Option<u8>) {
    let total_ships: u32 = team_counts.values().sum();
    let (plurality_team, net) = find_plurality(team_counts);
    let speed = capture_speed(net);

    match current {
        ControlPointState::Neutral => {
            if let Some(team) = plurality_team {
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
            match plurality_team {
                Some(t) if t == *team => {
                    let new_progress = progress + speed * dt;
                    if new_progress >= 1.0 {
                        (ControlPointState::Captured { team: *team }, None)
                    } else {
                        (
                            ControlPointState::Capturing {
                                team: *team,
                                progress: new_progress,
                            },
                            None,
                        )
                    }
                }
                Some(_enemy) => {
                    let new_progress = progress - speed * dt;
                    if new_progress <= 0.0 {
                        (ControlPointState::Neutral, None)
                    } else {
                        (
                            ControlPointState::Capturing {
                                team: *team,
                                progress: new_progress,
                            },
                            None,
                        )
                    }
                }
                None => {
                    if total_ships == 0 {
                        let new_progress = progress - DECAY_RATE * dt;
                        if new_progress <= 0.0 {
                            (ControlPointState::Neutral, None)
                        } else {
                            (
                                ControlPointState::Capturing {
                                    team: *team,
                                    progress: new_progress,
                                },
                                None,
                            )
                        }
                    } else {
                        // Tied with ships present — freeze
                        (
                            ControlPointState::Capturing {
                                team: *team,
                                progress: *progress,
                            },
                            None,
                        )
                    }
                }
            }
        }

        ControlPointState::Captured { team } => match plurality_team {
            Some(t) if t != *team => {
                let new_progress = 1.0 - speed * dt;
                (
                    ControlPointState::Decapturing {
                        team: *team,
                        progress: new_progress.max(0.0),
                    },
                    Some(*team),
                )
            }
            _ => (ControlPointState::Captured { team: *team }, Some(*team)),
        },

        ControlPointState::Decapturing { team, progress } => {
            match plurality_team {
                Some(t) if t != *team => {
                    let new_progress = progress - speed * dt;
                    if new_progress <= 0.0 {
                        (ControlPointState::Neutral, None)
                    } else {
                        (
                            ControlPointState::Decapturing {
                                team: *team,
                                progress: new_progress,
                            },
                            None,
                        )
                    }
                }
                Some(t) if t == *team => {
                    let new_progress = progress + speed * dt;
                    if new_progress >= 1.0 {
                        (ControlPointState::Captured { team: *team }, None)
                    } else {
                        (
                            ControlPointState::Decapturing {
                                team: *team,
                                progress: new_progress,
                            },
                            None,
                        )
                    }
                }
                _ => {
                    if total_ships == 0 {
                        let new_progress = progress - DECAY_RATE * dt;
                        if new_progress <= 0.0 {
                            (ControlPointState::Neutral, None)
                        } else {
                            (
                                ControlPointState::Decapturing {
                                    team: *team,
                                    progress: new_progress,
                                },
                                None,
                            )
                        }
                    } else {
                        // Tied with ships present — freeze
                        (
                            ControlPointState::Decapturing {
                                team: *team,
                                progress: *progress,
                            },
                            None,
                        )
                    }
                }
            }
        }
    }
}

// ── Server plugin ────────────────────────────────────────────────────────

pub struct ControlPointPlugin;

impl Plugin for ControlPointPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (update_control_points, check_score_victory)
                .chain()
                .run_if(in_state(GameState::Playing)),
        );
    }
}

fn update_control_points(
    time: Res<Time>,
    ships: Query<(&Transform, &Team), (With<Ship>, Without<Destroyed>)>,
    mut points: Query<
        (
            &Transform,
            &ControlPointRadius,
            &mut ControlPointState,
            &mut TeamScores,
        ),
        With<ControlPoint>,
    >,
) {
    let dt = time.delta_secs();

    for (point_tf, radius, mut state, mut scores) in &mut points {
        let center = Vec2::new(point_tf.translation.x, point_tf.translation.z);
        let r_sq = radius.0 * radius.0;

        let mut team_counts: HashMap<u8, u32> = HashMap::new();
        for (ship_tf, team) in &ships {
            let ship_pos = Vec2::new(ship_tf.translation.x, ship_tf.translation.z);
            if ship_pos.distance_squared(center) <= r_sq {
                *team_counts.entry(team.0).or_default() += 1;
            }
        }

        let (new_state, scoring_team) = compute_next_state(&state, &team_counts, dt);
        if let Some(team_id) = scoring_team {
            *scores.scores.entry(team_id).or_default() += SCORE_TICK_RATE * dt;
        }
        *state = new_state;
    }
}

fn check_score_victory(
    mut commands: Commands,
    mut next_state: ResMut<NextState<GameState>>,
    points: Query<&TeamScores, With<ControlPoint>>,
) {
    let mut totals: HashMap<u8, f32> = HashMap::new();
    for scores in &points {
        for (&team_id, &score) in &scores.scores {
            *totals.entry(team_id).or_default() += score;
        }
    }

    for (&team_idx, &score) in &totals {
        if score >= SCORE_VICTORY_THRESHOLD {
            let winning_team = Team(team_idx);
            info!(
                "Team {} wins by score! ({:.0} points)",
                winning_team.0, score
            );
            commands.server_trigger(ToClients {
                mode: SendMode::Broadcast,
                message: GameResult { winning_team: Some(winning_team) },
            });
            next_state.set(GameState::GameOver);
            return;
        }
    }
}

// ── Team colors ──────────────────────────────────────────────────────────
const COLOR_NEUTRAL: Color = Color::srgb(0.5, 0.5, 0.5); // Gray

// ── Client plugin ────────────────────────────────────────────────────────

pub struct ControlPointClientPlugin;

impl Plugin for ControlPointClientPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (draw_control_point_gizmos, update_score_display)
                .run_if(in_state(GameState::Playing)),
        )
        .add_systems(OnEnter(GameState::Playing), spawn_score_ui);
    }
}

fn team_color(team: u8, _local_team: &LocalTeam) -> Color {
    Team(team).color()
}

fn draw_control_point_gizmos(
    mut gizmos: Gizmos,
    time: Res<Time>,
    local_team: Res<LocalTeam>,
    points: Query<(&Transform, &ControlPointRadius, &ControlPointState), With<ControlPoint>>,
) {
    for (tf, radius, state) in &points {
        let center = tf.translation;
        let r = radius.0;

        let (base_color, pulse_progress) = match state {
            ControlPointState::Neutral => (COLOR_NEUTRAL, None),
            ControlPointState::Capturing { team, progress } => {
                (team_color(*team, &local_team), Some(*progress))
            }
            ControlPointState::Decapturing { team, progress } => {
                (team_color(*team, &local_team), Some(*progress))
            }
            ControlPointState::Captured { team } => (team_color(*team, &local_team), None),
        };

        let color = if let Some(progress) = pulse_progress {
            // Gentle pulse — slow ramp from 1Hz to 2Hz as capture nears completion
            let freq = 1.0 + progress * 1.0;
            let pulse = 0.5 + 0.5 * (time.elapsed_secs() * freq).sin();
            let Srgba {
                red, green, blue, ..
            } = Srgba::from(base_color);
            Color::srgba(
                0.5 * (1.0 - pulse) + red * pulse,
                0.5 * (1.0 - pulse) + green * pulse,
                0.5 * (1.0 - pulse) + blue * pulse,
                0.6,
            )
        } else {
            base_color.with_alpha(0.6)
        };

        gizmos.sphere(Isometry3d::new(center, Quat::IDENTITY), r, color);
    }
}

#[derive(Component)]
struct ScoreDisplayText;

fn spawn_score_ui(mut commands: Commands) {
    commands.spawn((
        ScoreDisplayText,
        Text::new("0  -  NEUTRAL  -  0"),
        TextFont {
            font_size: 22.0,
            ..default()
        },
        TextColor(Color::WHITE),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(12.0),
            left: Val::Percent(50.0),
            margin: UiRect {
                left: Val::Px(-80.0),
                ..default()
            },
            ..default()
        },
        Pickable::IGNORE,
    ));
}

fn update_score_display(
    local_team: Res<LocalTeam>,
    config: Option<Res<GameConfig>>,
    points: Query<(&TeamScores, &ControlPointState), With<ControlPoint>>,
    mut query: Query<(&mut Text, &mut TextColor), With<ScoreDisplayText>>,
) {
    let Ok((mut text, mut text_color)) = query.single_mut() else {
        return;
    };

    let mut totals: HashMap<u8, f32> = HashMap::new();
    let mut capture_info = String::new();

    let local_id = local_team.0.map(|t| t.0).unwrap_or(0);

    for (scores, state) in &points {
        for (&team_id, &score) in &scores.scores {
            *totals.entry(team_id).or_default() += score;
        }

        capture_info = match state {
            ControlPointState::Neutral => "NEUTRAL".to_string(),
            ControlPointState::Capturing { team, progress } => {
                let pct = (progress * 100.0) as u32;
                if *team == local_id {
                    format!("CAPTURING {pct}%")
                } else {
                    format!("ENEMY CAP {pct}%")
                }
            }
            ControlPointState::Decapturing { team, progress } => {
                let pct = (progress * 100.0) as u32;
                if *team == local_id {
                    format!("LOSING {pct}%")
                } else {
                    format!("CONTESTING {pct}%")
                }
            }
            ControlPointState::Captured { team } => {
                if *team == local_id {
                    "HELD".to_string()
                } else {
                    "ENEMY HELD".to_string()
                }
            }
        };
    }

    // Build score string showing all teams (use GameConfig if available)
    let team_count = match config {
        Some(ref cfg) => cfg.team_count,
        None => totals.keys().copied().max().map(|m| m + 1).unwrap_or(2),
    };
    let score_parts: Vec<String> = (0..team_count)
        .map(|t| format!("{}", totals.get(&t).copied().unwrap_or(0.0) as u32))
        .collect();
    let scores_str = score_parts.join(" | ");

    *text = Text::new(format!("{scores_str} \u{2014} {capture_info}"));

    // Color based on local team leading
    let my_score = totals.get(&local_id).copied().unwrap_or(0.0);
    let max_enemy = totals
        .iter()
        .filter(|&(&t, _)| t != local_id)
        .map(|(_, &s)| s)
        .fold(0.0f32, f32::max);

    text_color.0 = if my_score > max_enemy {
        Color::srgb(0.3, 1.0, 0.3)
    } else if max_enemy > my_score {
        Color::srgb(1.0, 0.3, 0.3)
    } else {
        Color::WHITE
    };
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
        let expected = 1.0 / BASE_CAPTURE_TIME;
        assert!((speed - expected).abs() < 1e-6);
    }

    #[test]
    fn capture_speed_four_ships() {
        let speed = capture_speed(4);
        let expected = 2.0 / BASE_CAPTURE_TIME;
        assert!((speed - expected).abs() < 1e-6);
    }

    #[test]
    fn capture_speed_nine_ships() {
        let speed = capture_speed(9);
        let expected = 3.0 / BASE_CAPTURE_TIME;
        assert!((speed - expected).abs() < 1e-6);
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
        assert_eq!(scores.scores.get(&0).copied().unwrap_or(0.0), 0.0);
        assert_eq!(scores.scores.get(&1).copied().unwrap_or(0.0), 0.0);
    }

    // ── State machine tests ──────────────────────────────────────────────

    #[test]
    fn neutral_to_capturing_with_majority() {
        let counts = HashMap::from([(0, 2u32), (1, 0)]);
        let (state, _) = compute_next_state(&ControlPointState::Neutral, &counts, 1.0);
        match state {
            ControlPointState::Capturing { team, progress } => {
                assert_eq!(team, 0);
                let expected = (2.0_f32).sqrt() / BASE_CAPTURE_TIME;
                assert!(
                    (progress - expected).abs() < 0.01,
                    "progress={progress}, expected={expected}"
                );
            }
            _ => panic!("Expected Capturing, got {:?}", state),
        }
    }

    #[test]
    fn neutral_stays_neutral_when_tied() {
        let counts = HashMap::from([(0, 1u32), (1, 1)]);
        let (state, _) = compute_next_state(&ControlPointState::Neutral, &counts, 1.0);
        assert_eq!(state, ControlPointState::Neutral);
    }

    #[test]
    fn neutral_stays_neutral_when_empty() {
        let counts = HashMap::new();
        let (state, _) = compute_next_state(&ControlPointState::Neutral, &counts, 1.0);
        assert_eq!(state, ControlPointState::Neutral);
    }

    #[test]
    fn neutral_to_captured_in_one_step_with_huge_dt() {
        let counts = HashMap::from([(0, 9u32)]);
        let (state, _) = compute_next_state(&ControlPointState::Neutral, &counts, 100.0);
        assert_eq!(state, ControlPointState::Captured { team: 0 });
    }

    #[test]
    fn capturing_completes_at_full_progress() {
        let counts = HashMap::from([(0, 1u32)]);
        let (state, _) = compute_next_state(
            &ControlPointState::Capturing {
                team: 0,
                progress: 0.99,
            },
            &counts,
            1.0,
        );
        assert_eq!(state, ControlPointState::Captured { team: 0 });
    }

    #[test]
    fn capturing_freezes_when_tied() {
        let counts = HashMap::from([(0, 1u32), (1, 1)]);
        let (state, _) = compute_next_state(
            &ControlPointState::Capturing {
                team: 0,
                progress: 0.5,
            },
            &counts,
            1.0,
        );
        assert_eq!(
            state,
            ControlPointState::Capturing {
                team: 0,
                progress: 0.5
            }
        );
    }

    #[test]
    fn capturing_decays_when_empty() {
        let counts = HashMap::new();
        let (state, _) = compute_next_state(
            &ControlPointState::Capturing {
                team: 0,
                progress: 0.5,
            },
            &counts,
            1.0,
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
        let counts = HashMap::from([(1, 1u32)]);
        let (state, _) = compute_next_state(
            &ControlPointState::Capturing {
                team: 0,
                progress: 0.01,
            },
            &counts,
            1.0,
        );
        assert_eq!(state, ControlPointState::Neutral);
    }

    #[test]
    fn captured_to_decapturing_on_enemy_majority() {
        let counts = HashMap::from([(1, 1u32)]);
        let (state, scoring_team) =
            compute_next_state(&ControlPointState::Captured { team: 0 }, &counts, 1.0);
        match state {
            ControlPointState::Decapturing { team, progress } => {
                assert_eq!(team, 0);
                let expected = 1.0 - (1.0 / BASE_CAPTURE_TIME);
                assert!(
                    (progress - expected).abs() < 1e-6,
                    "progress={progress}, expected={expected}"
                );
            }
            _ => panic!("Expected Decapturing, got {:?}", state),
        }
        assert_eq!(scoring_team, Some(0));
    }

    #[test]
    fn captured_stays_captured_no_enemies() {
        let counts = HashMap::new();
        let (state, scoring_team) =
            compute_next_state(&ControlPointState::Captured { team: 0 }, &counts, 1.0);
        assert_eq!(state, ControlPointState::Captured { team: 0 });
        assert_eq!(scoring_team, Some(0));
    }

    #[test]
    fn captured_stays_captured_with_owner_present() {
        let counts = HashMap::from([(0, 2u32)]);
        let (state, scoring_team) =
            compute_next_state(&ControlPointState::Captured { team: 0 }, &counts, 1.0);
        assert_eq!(state, ControlPointState::Captured { team: 0 });
        assert_eq!(scoring_team, Some(0));
    }

    #[test]
    fn decapturing_to_neutral() {
        let counts = HashMap::from([(1, 1u32)]);
        let (state, scoring) = compute_next_state(
            &ControlPointState::Decapturing {
                team: 0,
                progress: 0.01,
            },
            &counts,
            1.0,
        );
        assert_eq!(state, ControlPointState::Neutral);
        assert_eq!(scoring, None);
    }

    #[test]
    fn decapturing_defended_back_to_captured() {
        let counts = HashMap::from([(0, 1u32)]);
        let (state, _) = compute_next_state(
            &ControlPointState::Decapturing {
                team: 0,
                progress: 0.99,
            },
            &counts,
            1.0,
        );
        assert_eq!(state, ControlPointState::Captured { team: 0 });
    }

    #[test]
    fn decapturing_freezes_when_tied() {
        let counts = HashMap::from([(0, 1u32), (1, 1)]);
        let (state, _) = compute_next_state(
            &ControlPointState::Decapturing {
                team: 0,
                progress: 0.5,
            },
            &counts,
            1.0,
        );
        assert_eq!(
            state,
            ControlPointState::Decapturing {
                team: 0,
                progress: 0.5
            }
        );
    }

    #[test]
    fn decapturing_decays_when_empty() {
        let counts = HashMap::new();
        let (state, _) = compute_next_state(
            &ControlPointState::Decapturing {
                team: 0,
                progress: 0.5,
            },
            &counts,
            1.0,
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
        let counts = HashMap::from([(0, 1u32)]);
        let mut state = ControlPointState::Neutral;
        for _ in 0..4 {
            let (new_state, _) = compute_next_state(&state, &counts, 0.25);
            state = new_state;
        }
        match state {
            ControlPointState::Capturing { progress, .. } => {
                let expected = 1.0 / BASE_CAPTURE_TIME;
                assert!(
                    (progress - expected).abs() < 1e-5,
                    "progress={progress}, expected={expected}"
                );
            }
            _ => panic!("Expected Capturing, got {:?}", state),
        }
    }

    #[test]
    fn team1_can_capture() {
        let counts = HashMap::from([(1, 3u32)]);
        let (state, _) = compute_next_state(&ControlPointState::Neutral, &counts, 1.0);
        match state {
            ControlPointState::Capturing { team, .. } => assert_eq!(team, 1),
            _ => panic!("Expected Capturing for team 1, got {:?}", state),
        }
    }

    // ── 3-team plurality tests ───────────────────────────────────────────

    #[test]
    fn three_teams_plurality_captures() {
        // Team 0 has 3 ships, team 1 has 1, team 2 has 1 → team 0 plurality with net 2
        let counts = HashMap::from([(0, 3u32), (1, 1), (2, 1)]);
        let (state, _) = compute_next_state(&ControlPointState::Neutral, &counts, 1.0);
        match state {
            ControlPointState::Capturing { team, progress } => {
                assert_eq!(team, 0);
                // net = 3 - 1 = 2, speed = sqrt(2) / BASE_CAPTURE_TIME
                let expected = (2.0_f32).sqrt() / BASE_CAPTURE_TIME;
                assert!(
                    (progress - expected).abs() < 0.01,
                    "progress={progress}, expected={expected}"
                );
            }
            _ => panic!("Expected Capturing, got {:?}", state),
        }
    }

    #[test]
    fn three_teams_top_two_tied_freezes() {
        // Teams 0 and 1 tied at 2, team 2 has 1 → no plurality → freeze
        let counts = HashMap::from([(0, 2u32), (1, 2), (2, 1)]);
        let (state, _) = compute_next_state(&ControlPointState::Neutral, &counts, 1.0);
        assert_eq!(state, ControlPointState::Neutral);
    }

    #[test]
    fn three_teams_all_equal_freezes() {
        // All three teams have 1 ship each → no plurality → freeze
        let counts = HashMap::from([(0, 1u32), (1, 1), (2, 1)]);
        let (state, _) = compute_next_state(&ControlPointState::Neutral, &counts, 1.0);
        assert_eq!(state, ControlPointState::Neutral);
    }

    // ── find_plurality tests ─────────────────────────────────────────────

    #[test]
    fn find_plurality_empty() {
        let counts = HashMap::new();
        assert_eq!(find_plurality(&counts), (None, 0));
    }

    #[test]
    fn find_plurality_single_team() {
        let counts = HashMap::from([(2, 3u32)]);
        assert_eq!(find_plurality(&counts), (Some(2), 3));
    }

    #[test]
    fn find_plurality_clear_winner() {
        let counts = HashMap::from([(0, 5u32), (1, 2)]);
        assert_eq!(find_plurality(&counts), (Some(0), 3));
    }

    #[test]
    fn find_plurality_tied() {
        let counts = HashMap::from([(0, 3u32), (1, 3)]);
        assert_eq!(find_plurality(&counts), (None, 0));
    }

    // ── World-level tests ────────────────────────────────────────────────

    use crate::ship::ShipClass;

    #[test]
    fn ship_inside_radius_counted() {
        let mut world = World::new();
        let point = world
            .spawn((
                ControlPoint,
                ControlPointState::Neutral,
                ControlPointRadius(100.0),
                TeamScores::default(),
                Transform::from_xyz(0.0, 0.0, 0.0),
            ))
            .id();
        world.spawn((
            Ship,
            ShipClass::Scout,
            Team(0),
            Transform::from_xyz(50.0, 0.0, 0.0),
        ));

        let point_tf = world.get::<Transform>(point).unwrap();
        let center = Vec2::new(point_tf.translation.x, point_tf.translation.z);
        let radius = world.get::<ControlPointRadius>(point).unwrap().0;
        let ship_pos = Vec2::new(50.0, 0.0);
        assert!(ship_pos.distance(center) <= radius);
    }

    #[test]
    fn ship_outside_radius_not_counted() {
        let mut world = World::new();
        let point = world
            .spawn((
                ControlPoint,
                ControlPointState::Neutral,
                ControlPointRadius(100.0),
                TeamScores::default(),
                Transform::from_xyz(0.0, 0.0, 0.0),
            ))
            .id();
        world.spawn((
            Ship,
            ShipClass::Scout,
            Team(0),
            Transform::from_xyz(150.0, 0.0, 0.0),
        ));

        let point_tf = world.get::<Transform>(point).unwrap();
        let center = Vec2::new(point_tf.translation.x, point_tf.translation.z);
        let radius = world.get::<ControlPointRadius>(point).unwrap().0;
        let ship_pos = Vec2::new(150.0, 0.0);
        assert!(ship_pos.distance(center) > radius);
    }

    #[test]
    fn ship_on_boundary_counted() {
        let mut world = World::new();
        let point = world
            .spawn((
                ControlPoint,
                ControlPointState::Neutral,
                ControlPointRadius(100.0),
                TeamScores::default(),
                Transform::from_xyz(0.0, 0.0, 0.0),
            ))
            .id();
        world.spawn((
            Ship,
            ShipClass::Scout,
            Team(0),
            Transform::from_xyz(100.0, 0.0, 0.0),
        ));

        let point_tf = world.get::<Transform>(point).unwrap();
        let center = Vec2::new(point_tf.translation.x, point_tf.translation.z);
        let radius = world.get::<ControlPointRadius>(point).unwrap().0;
        let ship_pos = Vec2::new(100.0, 0.0);
        assert!(ship_pos.distance(center) <= radius);
    }

    #[test]
    fn score_accumulation_over_time() {
        let mut scores = TeamScores::default();
        let dt = 1.0 / 60.0;
        for _ in 0..300 {
            // 5 seconds at 60Hz
            *scores.scores.entry(0).or_default() += SCORE_TICK_RATE * dt;
        }
        assert!((scores.scores[&0] - 5.0).abs() < 0.1);
    }

    #[test]
    fn score_victory_threshold_reached() {
        let scores = TeamScores {
            scores: HashMap::from([(0, 30.0), (1, 10.0)]),
        };
        assert!(scores.scores[&0] >= SCORE_VICTORY_THRESHOLD);
        assert!(scores.scores[&1] < SCORE_VICTORY_THRESHOLD);
    }

    #[test]
    fn score_victory_threshold_not_reached() {
        let scores = TeamScores {
            scores: HashMap::from([(0, 29.9), (1, 29.9)]),
        };
        assert!(scores.scores[&0] < SCORE_VICTORY_THRESHOLD);
        assert!(scores.scores[&1] < SCORE_VICTORY_THRESHOLD);
    }
}
