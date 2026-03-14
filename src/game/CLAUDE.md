# game/

Core game state and shared types.

## Files

- `mod.rs` — GameState enum (Setup→Playing→Victory), Team component (u8 id, PLAYER=0, ENEMY=1), Detected marker component (enemy in LOS), EnemyVisibility component (opacity f32 for fade), Health component (hp u8), check_victory system (no enemies alive → Victory, guards against empty query), spawn_victory_ui (text overlay)
