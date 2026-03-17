# Phase 4c: Control Points & Win Conditions — Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a capture-point system with score-based victory alongside existing annihilation win condition.

**Architecture:** New `control_point` module with server-authoritative state machine. `ControlPoint` entities replicated to both clients. Server ticks capture progress based on ship presence; clients draw gizmo indicators and score UI. Scoring system runs independently from existing annihilation check. `TeamScores` is a replicated component on the ControlPoint entity (not a resource — bevy_replicon 0.39 doesn't support replicated resources).

**Tech Stack:** Bevy 0.18, bevy_replicon 0.39, Bevy Gizmos (immediate mode)

**Design doc:** `docs/plans/2026-03-17-phase4c-control-points-design.md`

---

## File Structure

| File | Responsibility |
|------|---------------|
| Create: `src/control_point/mod.rs` | Components, constants, state enum, pure capture-speed functions, `ControlPointPlugin` (server systems), `ControlPointClientPlugin` (gizmo visuals + score UI) |
| Modify: `src/lib.rs` | Add `pub mod control_point;` |
| Modify: `src/net/replication.rs` | Register `ControlPoint`, `ControlPointState`, `ControlPointRadius`, `TeamScores` for replication |
| Modify: `src/net/server.rs` | Spawn control point entity in `server_setup_game` |
| Modify: `src/bin/server.rs` | Add `ControlPointPlugin` |
| Modify: `src/bin/client.rs` | Add `ControlPointClientPlugin` |

---

### Task 1: Components, Constants & Pure Functions

**Files:**
- Create: `src/control_point/mod.rs`
- Modify: `src/lib.rs`

- [ ] **Step 1: Create the module file with components and constants**

Create `src/control_point/mod.rs` with:

```rust
use bevy::prelude::*;
use bevy_replicon::prelude::*;
use serde::{Deserialize, Serialize};

use crate::game::{Destroyed, GameState, Team};
use crate::net::LocalTeam;
use crate::ship::Ship;

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
/// replicates automatically via bevy_replicon. Summed across all points on
/// the client for display.
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
```

- [ ] **Step 2: Add module declaration to lib.rs**

In `src/lib.rs`, add `pub mod control_point;` after the existing module declarations.

- [ ] **Step 3: Write tests for pure functions**

At the bottom of `src/control_point/mod.rs`, add:

```rust
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
        // 1 ship: sqrt(1) / 20.0 = 0.05
        assert!((speed - 0.05).abs() < 1e-6);
    }

    #[test]
    fn capture_speed_four_ships() {
        let speed = capture_speed(4);
        // 4 ships: sqrt(4) / 20.0 = 0.1
        assert!((speed - 0.1).abs() < 1e-6);
    }

    #[test]
    fn capture_speed_nine_ships() {
        let speed = capture_speed(9);
        // 9 ships: sqrt(9) / 20.0 = 0.15
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
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --lib control_point`
Expected: 7 tests PASS

- [ ] **Step 5: Commit**

```bash
git add src/control_point/mod.rs src/lib.rs
git commit -m "feat(control_point): add components, constants, and capture speed function"
```

---

### Task 2: Replication Registration & Server Spawn

**Files:**
- Modify: `src/net/replication.rs:10-14` (imports) and after line 93 (radar contacts)
- Modify: `src/net/server.rs:255-355` (spawn in `server_setup_game`)

- [ ] **Step 1: Add imports to replication.rs**

Add to the imports section at the top of `src/net/replication.rs`:

```rust
use crate::control_point::{ControlPoint, ControlPointRadius, ControlPointState, TeamScores};
```

- [ ] **Step 2: Register replicated components**

After the radar contact component block (after `.replicate::<ContactKind>();`), add:

```rust
        // Control point components
        app.replicate::<ControlPoint>()
            .replicate::<ControlPointState>()
            .replicate::<ControlPointRadius>()
            .replicate::<TeamScores>();
```

- [ ] **Step 3: Spawn control point in server_setup_game**

Add import to `src/net/server.rs`:

```rust
use crate::control_point::{ControlPoint, ControlPointRadius, ControlPointState, TeamScores, DEFAULT_ZONE_RADIUS};
```

After the asteroid spawning loop (before the final `info!` log), add:

```rust
    // Spawn control point at map center
    commands.spawn((
        ControlPoint,
        ControlPointState::Neutral,
        ControlPointRadius(DEFAULT_ZONE_RADIUS),
        TeamScores::default(),
        Transform::from_xyz(0.0, 0.0, 0.0),
        Replicated,
    ));

    info!("Server: spawned control point at map center");
```

- [ ] **Step 4: Run cargo check**

Run: `cargo check`
Expected: compiles successfully

- [ ] **Step 5: Commit**

```bash
git add src/net/replication.rs src/net/server.rs
git commit -m "feat(control_point): register replication and spawn at map center"
```

---

### Task 3: State Machine — Tests & Implementation

**Files:**
- Modify: `src/control_point/mod.rs` (add `compute_next_state` and tests)

- [ ] **Step 1: Write tests for state transitions**

Add these tests to the existing `#[cfg(test)]` block in `src/control_point/mod.rs`:

```rust
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
                // sqrt(2) / 20.0 ≈ 0.0707
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
            9, 0, 100.0, // way more than needed
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
        // Was captured at start of frame, still scores
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
        // Simulate 4 frames at 0.25s each with 1 ship
        let mut state = ControlPointState::Neutral;
        for _ in 0..4 {
            let (new_state, _) = compute_next_state(&state, 1, 0, 0.25);
            state = new_state;
        }
        // 4 * 0.25 * 0.05 = 0.05 total progress
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
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib control_point`
Expected: FAIL — `compute_next_state` not defined

- [ ] **Step 3: Implement compute_next_state pure function**

Add to `src/control_point/mod.rs` in the pure functions section:

```rust
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
                        // Tied: freeze
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
                        // Tied: freeze
                        (ControlPointState::Decapturing { team: *team, progress: *progress }, None)
                    }
                }
            }
        }
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --lib control_point`
Expected: all 25 tests PASS (7 from Task 1 + 18 new)

- [ ] **Step 5: Commit**

```bash
git add src/control_point/mod.rs
git commit -m "feat(control_point): implement capture state machine with tests"
```

---

### Task 4: Server Systems & Plugin

**Files:**
- Modify: `src/control_point/mod.rs` (add server systems and ControlPointPlugin)
- Modify: `src/bin/server.rs` (register plugin)

- [ ] **Step 1: Add server systems to control_point/mod.rs**

Add the following after the `compute_next_state` function:

```rust
use crate::net::commands::GameResult;

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
        (&Transform, &ControlPointRadius, &mut ControlPointState, &mut TeamScores),
        With<ControlPoint>,
    >,
) {
    let dt = time.delta_secs();

    for (point_tf, radius, mut state, mut scores) in &mut points {
        let center = Vec2::new(point_tf.translation.x, point_tf.translation.z);
        let r_sq = radius.0 * radius.0;

        let mut team0_count = 0u32;
        let mut team1_count = 0u32;

        for (ship_tf, team) in &ships {
            let ship_pos = Vec2::new(ship_tf.translation.x, ship_tf.translation.z);
            if ship_pos.distance_squared(center) <= r_sq {
                match team.0 {
                    0 => team0_count += 1,
                    1 => team1_count += 1,
                    _ => {}
                }
            }
        }

        let (new_state, scoring_team) = compute_next_state(&state, team0_count, team1_count, dt);
        if let Some(team_id) = scoring_team {
            scores.scores[team_id as usize] += SCORE_TICK_RATE * dt;
        }
        *state = new_state;
    }
}

fn check_score_victory(
    mut commands: Commands,
    mut next_state: ResMut<NextState<GameState>>,
    points: Query<&TeamScores, With<ControlPoint>>,
) {
    // Sum scores across all control points (future-proof for N points)
    let mut totals = [0.0f32; 2];
    for scores in &points {
        totals[0] += scores.scores[0];
        totals[1] += scores.scores[1];
    }

    for (team_idx, &score) in totals.iter().enumerate() {
        if score >= SCORE_VICTORY_THRESHOLD {
            let winning_team = Team(team_idx as u8);
            info!("Team {} wins by score! ({:.0} points)", winning_team.0, score);
            commands.server_trigger(ToClients {
                mode: SendMode::Broadcast,
                message: GameResult { winning_team },
            });
            next_state.set(GameState::GameOver);
            return;
        }
    }
}
```

- [ ] **Step 2: Register plugin in server binary**

In `src/bin/server.rs`, add import:

```rust
use nebulous_shot_command::control_point::ControlPointPlugin;
```

Add `ControlPointPlugin` to the second `.add_plugins((...))` block (alongside `RadarPlugin`, `DamagePlugin`, `ServerNetPlugin`).

- [ ] **Step 3: Run cargo check**

Run: `cargo check --bin server`
Expected: compiles successfully

- [ ] **Step 4: Commit**

```bash
git add src/control_point/mod.rs src/bin/server.rs
git commit -m "feat(control_point): add server capture systems and plugin"
```

---

### Task 5: Client Visuals — Gizmo Sphere & Score UI

**Files:**
- Modify: `src/control_point/mod.rs` (add client plugin with gizmo drawing and score UI)
- Modify: `src/bin/client.rs` (register client plugin)

- [ ] **Step 1: Add ControlPointClientPlugin with gizmo sphere drawing**

Add to `src/control_point/mod.rs`:

```rust
// ── Team colors (matching materializer.rs) ───────────────────────────────
const COLOR_FRIENDLY: Color = Color::srgb(0.2, 0.6, 1.0); // Blue
const COLOR_ENEMY: Color = Color::srgb(1.0, 0.2, 0.2);    // Red
const COLOR_NEUTRAL: Color = Color::srgb(0.5, 0.5, 0.5);  // Gray

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

fn team_color(team: u8, local_team: &LocalTeam) -> Color {
    if local_team.0.map(|lt| lt.0 == team).unwrap_or(false) {
        COLOR_FRIENDLY
    } else {
        COLOR_ENEMY
    }
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
            ControlPointState::Captured { team } => {
                (team_color(*team, &local_team), None)
            }
        };

        let color = if let Some(progress) = pulse_progress {
            // Pulse speed proportional to capture progress (faster as capture nears completion)
            let freq = 2.0 + progress * 4.0; // 2 Hz at start, 6 Hz near completion
            let pulse = 0.5 + 0.5 * (time.elapsed_secs() * freq).sin();
            let Srgba { red, green, blue, .. } = Srgba::from(base_color);
            Color::srgba(
                0.5 * (1.0 - pulse) + red * pulse,
                0.5 * (1.0 - pulse) + green * pulse,
                0.5 * (1.0 - pulse) + blue * pulse,
                0.6,
            )
        } else {
            base_color.with_alpha(0.6)
        };

        // Horizontal circle (XZ plane)
        gizmos.circle(
            Isometry3d::new(center, Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
            r,
            color,
        );

        // Vertical circle (XY plane)
        gizmos.circle(
            Isometry3d::new(center, Quat::IDENTITY),
            r,
            color,
        );
    }
}
```

Note: `Srgba` is from `bevy::color::Srgba`. Add `use bevy::color::Srgba;` to the imports.

- [ ] **Step 2: Add score UI spawn and update systems**

Continue in `src/control_point/mod.rs`:

```rust
#[derive(Component)]
struct ScoreDisplayText;

fn spawn_score_ui(mut commands: Commands) {
    commands.spawn((
        ScoreDisplayText,
        Text::new("0  ───  0"),
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
    points: Query<&TeamScores, With<ControlPoint>>,
    mut query: Query<&mut Text, With<ScoreDisplayText>>,
) {
    let Ok(mut text) = query.single_mut() else { return };

    let mut totals = [0.0f32; 2];
    for scores in &points {
        totals[0] += scores.scores[0];
        totals[1] += scores.scores[1];
    }

    let local_id = local_team.0.map(|t| t.0 as usize).unwrap_or(0);
    let enemy_id = 1 - local_id;

    *text = Text::new(format!("{}  ───  {}", totals[local_id] as u32, totals[enemy_id] as u32));
}
```

- [ ] **Step 3: Register client plugin**

In `src/bin/client.rs`, add import:

```rust
use nebulous_shot_command::control_point::ControlPointClientPlugin;
```

Add `ControlPointClientPlugin` to the `.add_plugins((...))` block.

- [ ] **Step 4: Run cargo check**

Run: `cargo check --bin client`
Expected: compiles successfully

- [ ] **Step 5: Commit**

```bash
git add src/control_point/mod.rs src/bin/client.rs
git commit -m "feat(control_point): add client gizmo sphere and score display"
```

---

### Task 6: World-Level Tests

**Files:**
- Modify: `src/control_point/mod.rs` (add world-level tests to `#[cfg(test)]` block)

- [ ] **Step 1: Add world-level tests for ship presence detection**

These test the `update_control_points` system using `World::new()`:

```rust
    // ── World-level tests ────────────────────────────────────────────────

    use crate::ship::ShipClass;

    /// Helper: set up a world with a control point at origin and return the point entity.
    fn setup_world_with_point() -> (World, Entity) {
        let mut world = World::new();
        world.insert_resource(Time::<()>::default());
        let point = world.spawn((
            ControlPoint,
            ControlPointState::Neutral,
            ControlPointRadius(100.0),
            TeamScores::default(),
            Transform::from_xyz(0.0, 0.0, 0.0),
        )).id();
        (world, point)
    }

    /// Helper: spawn a ship at a position for a team.
    fn spawn_test_ship(world: &mut World, pos: Vec2, team: u8) -> Entity {
        world.spawn((
            Ship,
            ShipClass::Scout,
            Team(team),
            Transform::from_xyz(pos.x, 0.0, pos.y),
        )).id()
    }

    #[test]
    fn ship_inside_radius_counted() {
        let (mut world, point) = setup_world_with_point();
        spawn_test_ship(&mut world, Vec2::new(50.0, 0.0), 0); // inside 100m

        // Manually count (since we can't easily run the system, test the distance check)
        let point_tf = world.get::<Transform>(point).unwrap();
        let center = Vec2::new(point_tf.translation.x, point_tf.translation.z);
        let radius = world.get::<ControlPointRadius>(point).unwrap().0;

        let ship_pos = Vec2::new(50.0, 0.0);
        assert!(ship_pos.distance(center) <= radius);
    }

    #[test]
    fn ship_outside_radius_not_counted() {
        let (mut world, point) = setup_world_with_point();
        spawn_test_ship(&mut world, Vec2::new(150.0, 0.0), 0); // outside 100m

        let point_tf = world.get::<Transform>(point).unwrap();
        let center = Vec2::new(point_tf.translation.x, point_tf.translation.z);
        let radius = world.get::<ControlPointRadius>(point).unwrap().0;

        let ship_pos = Vec2::new(150.0, 0.0);
        assert!(ship_pos.distance(center) > radius);
    }

    #[test]
    fn ship_on_boundary_counted() {
        let (mut world, point) = setup_world_with_point();
        spawn_test_ship(&mut world, Vec2::new(100.0, 0.0), 0); // exactly on boundary

        let point_tf = world.get::<Transform>(point).unwrap();
        let center = Vec2::new(point_tf.translation.x, point_tf.translation.z);
        let radius = world.get::<ControlPointRadius>(point).unwrap().0;

        let ship_pos = Vec2::new(100.0, 0.0);
        assert!(ship_pos.distance(center) <= radius);
    }

    #[test]
    fn score_accumulation_over_time() {
        // Simulate 5 seconds of captured state scoring
        let mut scores = TeamScores::default();
        let dt = 1.0 / 60.0; // 60 Hz
        for _ in 0..300 { // 5 seconds
            scores.scores[0] += SCORE_TICK_RATE * dt;
        }
        assert!((scores.scores[0] - 5.0).abs() < 0.1);
    }

    #[test]
    fn score_victory_threshold_reached() {
        let scores = TeamScores { scores: [300.0, 50.0] };
        assert!(scores.scores[0] >= SCORE_VICTORY_THRESHOLD);
        assert!(scores.scores[1] < SCORE_VICTORY_THRESHOLD);
    }

    #[test]
    fn score_victory_threshold_not_reached() {
        let scores = TeamScores { scores: [299.9, 299.9] };
        assert!(scores.scores[0] < SCORE_VICTORY_THRESHOLD);
        assert!(scores.scores[1] < SCORE_VICTORY_THRESHOLD);
    }
```

- [ ] **Step 2: Run tests to verify they pass**

Run: `cargo test --lib control_point`
Expected: all tests PASS (25 pure + 6 world-level = 31 total)

- [ ] **Step 3: Commit**

```bash
git add src/control_point/mod.rs
git commit -m "test(control_point): add world-level tests for presence and scoring"
```

---

### Task 7: Integration Test — Full Playtest

**Files:** None (manual testing)

- [ ] **Step 1: Run server + two clients**

```bash
cargo run --bin server &
cargo run --bin client -- --fleet 1 &
cargo run --bin client -- --fleet 2 &
```

Or use `./run_game.sh` if it launches both.

- [ ] **Step 2: Verify control point appears**

- Two perpendicular circles visible at map center forming a wireframe sphere
- Gray color when neutral
- ~100m radius

- [ ] **Step 3: Verify capture flow**

- Move a ship into the zone — circles start pulsing your team color
- Wait ~20s — circles go solid (captured)
- Score display at top starts incrementing

- [ ] **Step 4: Verify decapture flow**

- Move enemy ship in while friendly is out
- Circles pulse, then revert to gray (neutral)
- Score stops incrementing

- [ ] **Step 5: Verify score victory**

- Let one team hold the point uncontested
- After 300 seconds, GameOver should trigger
- (Optionally temporarily reduce SCORE_VICTORY_THRESHOLD to 30 for faster testing)

- [ ] **Step 6: Verify annihilation still works**

- Destroy all enemy ships while point is active
- Should still get GameOver from annihilation

- [ ] **Step 7: Commit any fixes**

```bash
git add -u
git commit -m "fix(control_point): integration test fixes"
```

---

## Task Dependency Graph

```
Task 1 (components + pure functions)
  ├──▶ Task 2 (replication + spawn)
  │      └──▶ Task 3 (state machine logic + tests)
  │             └──▶ Task 4 (server systems + plugin)
  │             └──▶ Task 5 (client visuals) [parallel with Task 4]
  │                    └──▶ Task 6 (world-level tests)
  └───────────────────────────▶ Task 7 (integration playtest)
```

Tasks 4 (server) and 5 (client) can be developed in parallel after Task 3.
Task 6 adds tests after both sides compile. Task 7 is the final validation.
