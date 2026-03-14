# combat/

Auto-targeting and projectile combat.

## Files

- `mod.rs` — ProjectileAssets resource (shared mesh/material), FireRate component (0.5s repeating timer on player ships), Projectile component (direction, speed, damage), auto-targeting closest detected enemy, projectile movement with bounds despawn, hit detection with health decrement and enemy despawn at 0 hp

## System chain (runs during Playing state)

1. `auto_target` — tick fire timer (only when enemies detected, reset otherwise), find closest Detected enemy, spawn projectile aimed at its position
2. `move_projectiles` — translate by direction × speed × dt, despawn out of bounds
3. `check_projectile_hits` — distance check (radius 10), decrement hp, despawn enemy at 0, tracks destroyed to avoid double despawn

## Key functions

- `projectile_direction(from, to)` — normalized direction, returns Vec3::ZERO if same position
- `is_hit(proj_pos, target_pos, radius)` — strict less-than distance check
