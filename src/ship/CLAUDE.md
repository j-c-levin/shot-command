# ship/

Ship entities, physics-based movement, and squad formations.

## Files

- `mod.rs` — Ship marker, ShipClass (Battleship/Destroyer/Scout), ShipProfile (incl. hp, engine_hp, rcs, collision_radius), Velocity, WaypointQueue, FacingTarget/FacingLocked, TargetDesignation, ShipNumber(u8), SquadMember/SquadSpeedLimit, ShipSecrets/ShipSecretsOwner, EngineHealth, RepairCooldown

## Physics model

- Velocity persists (momentum/drift). Steering controller computes desired velocity, then thrusts to correct.
- Worst-case deceleration (thruster_factor) used for braking. Ships brake to stop when queue is empty.
- Space drag: ~26% velocity/second bleed.
- Ship-asteroid collision: pushed to edge + velocity zeroed.

## Physics chain (Update)

1. update_facing_targets → 2. turn_ships → 3. apply_thrust (gated on EngineHealth) → 4. apply_velocity → 5. ship-asteroid collision → 6. check_waypoint_arrival → 7. clamp_to_bounds

## Key patterns

- **Facing**: Unlocked ships auto-face waypoint. Locked ships maintain player-set facing.
- **Waypoints**: Right-click = clear + single. Shift+right-click = append.
- **Squads**: SquadMember { leader, offset }. SquadSpeedLimit caps all stats to min across squad. Leader move propagates with rotated offsets. Cycle prevention (10 hops). Orphan cleanup on leader destroyed.
- **Engine offline**: EngineHealth at 0 → apply_thrust skipped → drift. After offline timer + floor restore, 10% capacity.

## Pure functions

- `thrust_multiplier(angle, thruster_factor)` — cosine interpolation
- `braking_distance(speed, deceleration)` — v²/(2a)
- `shortest_angle_delta(from, to)` — signed shortest angle -PI..PI
- `rotate_offset(offset, angle)` — formation rotation for facing commands

## Ship stats

| Class | Hull HP | Engine HP | RCS |
|---|---|---|---|
| Battleship | 1200 | 300 | highest |
| Destroyer | 600 | 180 | medium |
| Scout | 300 | 120 | lowest |
