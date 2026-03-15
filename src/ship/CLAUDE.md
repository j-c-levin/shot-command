# ship/

Ship entities, physics-based movement, and visual indicators.

## Files

- `mod.rs` — Ship marker, ShipClass enum (Battleship/Destroyer/Scout), ShipProfile (acceleration, thruster_factor, turn_rate, turn_acceleration, top_speed, vision_range, collision_radius), Velocity (linear Vec2 + angular f32), WaypointQueue (VecDeque + braking flag), FacingTarget/FacingLocked, Selected/SelectionIndicator markers, WaypointMarker/FacingIndicator components

## Pure functions

- `thrust_multiplier(angle, thruster_factor)` — cosine interpolation from 1.0 (facing target) to thruster_factor (facing away)
- `angle_between_directions(a, b)` — unsigned angle 0..PI between unit vectors
- `braking_distance(speed, deceleration)` — v²/(2a)
- `shortest_angle_delta(from, to)` — signed shortest angle -PI..PI
- `ship_xz_position(transform)` — extract XZ as Vec2
- `ship_facing_direction(transform)` — forward direction as Vec2 in XZ
- `ship_heading(transform)` — heading angle in radians

## Systems (chained, Update)

1. `update_facing_targets` — unlocked ships: FacingTarget auto-set toward next waypoint. Locked: skip. No waypoints: remove FacingTarget.
2. `turn_ships` — angular velocity ramps at turn_acceleration, capped at turn_rate. Decelerates to stop at target angle. No target: decelerate angular to zero.
3. `apply_thrust` — braking mode: decelerate using facing-dependent thrust. Has waypoints: accelerate toward next, brake at last. No waypoints: drift.
4. `apply_velocity` — position += linear * dt
5. `check_waypoint_arrival` — pop waypoint within threshold, set braking on last
6. `clamp_ships_to_bounds` — kill velocity component at boundary

## Visual indicator systems (Update, parallel)

- `update_waypoint_markers` — despawn/respawn blue spheres at each player waypoint
- `update_facing_indicators` — despawn/respawn yellow capsule showing locked facing direction

## Spawning

- `spawn_ship(commands, meshes, materials, position, team, color, class)` — cuboid for battleship, cone for destroyer, sphere for scout. Enemies get EnemyVisibility + Health + hidden start.
