# fog/

Line-of-sight detection and enemy fade in/out.

## Files

- `mod.rs` — Distance+raycast LOS detection between player and enemy ships, enemy fade in/out via EnemyVisibility opacity and material alpha updates

## System chain (runs during Playing state)

1. `detect_enemies` — for each enemy, check if any player ship has LOS (distance ≤ vision_range + unblocked by asteroids); insert/remove Detected marker on state change only
2. `fade_enemies` — lerp EnemyVisibility.opacity toward 1.0 (detected) or 0.0 (not) over FADE_DURATION (0.5s); update Visibility and material base_color alpha

## Key functions

- `is_in_los(observer, target, vision_range, asteroids)` — pure function combining range check + ray occlusion
- `fade_opacity(current, target, dt, fade_duration)` — pure function for constant-rate fade with clamping
- `ray_blocked_by_asteroid(start, end, asteroids)` — circle-line intersection test
