# Phase 4c: Control Points & Win Conditions

## Overview

Add control points that force map engagement and create a score-based win condition
alongside existing annihilation. Ships must enter zones to capture them, captured zones
generate score over time, first team to 300 wins.

Start with 1 central point. System designed to support N points with no code changes.

## Control Point State Machine

```
Neutral ──(team majority enters)──▶ Capturing(team, progress)
Capturing ──(progress reaches 1.0)──▶ Captured(team) [starts scoring]
Captured ──(enemy majority enters)──▶ Decapturing(owner, progress drains)
Decapturing ──(progress reaches 0.0)──▶ Neutral
Decapturing ──(owner reclaims majority)──▶ progress climbs back
Decapturing ──(progress reaches 1.0)──▶ Captured(owner) [defended]
```

Two-phase swing: enemy must first decapture (drain to neutral), then capture (fill to
owned). No instant flips.

## Components

- `ControlPoint` — marker component
- `ControlPointState` — enum:
  - `Neutral`
  - `Capturing { team: u8, progress: f32 }`
  - `Captured { team: u8 }`
  - `Decapturing { team: u8, progress: f32 }`
- `ControlPointRadius(f32)` — default 100m

All replicated to both clients. Server-authoritative.

## Presence Detection & Capture Logic

Server system `update_control_points`, runs every frame during `Playing`.

### Ship counting

Query all alive (non-Destroyed) ships within `ControlPointRadius` of each control point.
Count per team. Determine majority team and net advantage (`majority_count - minority_count`).
If tied, `net = 0`.

### Capture speed

`speed = sqrt(net) / BASE_CAPTURE_TIME` per second.

`BASE_CAPTURE_TIME = 20.0` seconds. So 1 ship = 0.05/s (20s), 4 ships = 0.1/s (10s),
9 ships = 0.15/s (~7s).

### State transitions

| Current State | Condition | Action |
|---|---|---|
| Neutral | One team has majority | → Capturing { team, progress += speed × dt } |
| Neutral | No ships or tied | No change |
| Capturing | Same team majority | progress += speed × dt |
| Capturing | Tied | Freeze (no change) |
| Capturing | No ships | progress -= DECAY_RATE × dt (toward 0, then Neutral) |
| Capturing | Enemy majority | progress -= speed × dt (if ≤ 0 → Neutral) |
| Capturing | progress ≥ 1.0 | → Captured { team } |
| Captured | Enemy majority | → Decapturing { team (original owner), progress = 1.0 } then progress -= speed × dt |
| Captured | Anything else | No change (scoring) |
| Decapturing | Enemy majority | progress -= speed × dt |
| Decapturing | Owner majority | progress += speed × dt (defending) |
| Decapturing | Tied | Freeze |
| Decapturing | No ships | progress -= DECAY_RATE × dt (toward 0, then Neutral) |
| Decapturing | progress ≥ 1.0 | → Captured { team } (defended successfully) |
| Decapturing | progress ≤ 0.0 | → Neutral |

### Unattended decay

`DECAY_RATE = 0.025/s`. Partial captures with no ships present decay to neutral in 40s
from full. Prevents drive-by partial caps from persisting.

## Scoring

### Resource

`TeamScores` — score per team. Replicated resource so clients can display.

### System

`tick_control_point_scoring` runs after `update_control_points`. For each point in
`Captured { team }` state, adds `1.0 × dt` to that team's score.

### Win condition

When any team's score reaches `SCORE_VICTORY_THRESHOLD = 300`, broadcast `GameResult`
and transition to `GameOver`. Annihilation (existing `check_win_condition`) still wins
instantly — no change to that system.

System ordering: `update_control_points` → `tick_control_point_scoring` → `check_score_victory`.
Existing `check_win_condition` (annihilation) runs independently.

## Spawning

Server spawns control point during fleet spawning (alongside asteroids). For 1 point:
center of map (0, 0, 0). Existing asteroid exclusion zone (`min_distance_from_center = 100m`)
already keeps asteroids out of the capture radius.

Bundle: `ControlPoint`, `ControlPointState::Neutral`, `ControlPointRadius(100.0)`,
`Transform::from_xyz(0.0, 0.0, 0.0)`.

Future: spawn from a map definition resource for multiple points.

## Visuals

### Control point indicator

Two perpendicular gizmo circles centered at the point position (y=0), radius matching
`ControlPointRadius`:

- Circle 1: XZ plane (horizontal)
- Circle 2: XY plane (vertical)

Color coding:
- Neutral: gray
- Capturing / Decapturing: pulsing between gray and team color (pulse speed proportional
  to capture rate)
- Captured: solid team color

### Score display

Top center UI text, updated each frame from `TeamScores`:

```
[0]  142  ───  87  [1]
```

Team-colored text if team colors are established, otherwise white.

## Constants

| Name | Value | Purpose |
|---|---|---|
| BASE_CAPTURE_TIME | 20.0s | Time for 1 ship to fully capture |
| DECAY_RATE | 0.025/s | Unattended partial capture decay |
| SCORE_VICTORY_THRESHOLD | 300 | Score needed to win |
| SCORE_TICK_RATE | 1.0/s | Points per second per captured zone |
| DEFAULT_ZONE_RADIUS | 100.0m | Capture zone radius |

## Module location

New module `src/control_point/mod.rs` with:
- Components and state enum
- `ControlPointPlugin` (server systems)
- `ControlPointClientPlugin` (gizmo visuals, score UI)
- Pure functions for capture speed calculation (testable)

## Testing

Pure function tests:
- Capture speed scaling (1 ship, 4 ships, 9 ships)
- State transitions (all table rows above)
- Score threshold check

World-level tests:
- Ship inside radius detected
- Ship outside radius not counted
- Capture completes after correct duration
- Contested point freezes
- Decapture flows through neutral before recapture
- Annihilation still wins during active scoring
