# game/

Core game state and shared types.

## Files

- `mod.rs` — GameState enum (Setup→Playing), Team component (u8 id, with color()/is_friendly()), GameConfig resource (team_count, players_per_team), Detected marker component (enemy in LOS), EnemyVisibility component (opacity f32 for fade), Health component (hp u16)
