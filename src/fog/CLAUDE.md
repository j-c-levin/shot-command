# fog/

Line-of-sight detection and visibility management.

## Files

- `mod.rs` — Server: LOS detection (distance + raycast through asteroids) drives replicon visibility filtering. Client: FogClientPlugin with ghost entity fade-out on visibility loss.

## Key functions

- `is_in_los(observer, target, vision_range, asteroids)` — range check + ray-asteroid occlusion
- `ray_blocked_by_asteroid(start, end, asteroids)` — circle-line intersection test
- `fade_opacity(current, target, dt, fade_duration)` — constant-rate fade with clamping

## Key behavior

- Visual LOS range: 400m for all ship classes
- Ghost fade-out: `On<Remove, Ship>` spawns visual-only ghost at same position, fades over 0.5s, then self-destructs
- Visual LOS guarantees track-level radar detection (`apply_visual_los_boost`) — if you can see it, radar tracks it
