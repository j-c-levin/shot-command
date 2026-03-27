# control_point/

Presence-based control point capture and scoring.

## Files

- `mod.rs` — ControlPoint/ControlPointState/ControlPointRadius/TeamScores components, capture_speed/compute_next_state pure functions, ControlPointPlugin (server), ControlPointClientPlugin (gizmo sphere, score UI)

## Constants

- BASE_CAPTURE_TIME: 20s
- DECAY_RATE: 0.025
- SCORE_VICTORY_THRESHOLD: 300
- DEFAULT_ZONE_RADIUS: 100m

## State machine

Neutral → Capturing → Captured → Decapturing (two-phase swing: must decapture to neutral before recapturing)

## Key behavior

- Plurality team (most ships in radius) makes progress; ties freeze; empty decays
- Capture speed: sqrt(net_advantage) / BASE_CAPTURE_TIME (diminishing returns)
- Captured points score 1pt/s. First to 300 wins. Annihilation still instant-wins.
- N-team aware: TeamScores uses HashMap<u8, f32>
- Gizmo wireframe sphere, color pulsing during capture/decapture, solid team color when captured
