# ship/

Ship entities and movement.

## Files

- `mod.rs` — Ship marker, ShipStats (speed=80, vision_range=200), MovementTarget (destination Vec2, added on move order, removed on arrival), Selected marker, SelectionIndicator marker, move_ships system (lerp toward target with look_at), clamp_ships_to_bounds system, spawn_ship helper (cone mesh, team-based color, enemy starts Visibility::Hidden)
