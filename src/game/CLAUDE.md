# game/

Core game state and shared types.

## Files

- `mod.rs` — GameState enum (Setup→Playing→Victory), Team component (u8 id, PLAYER=0, ENEMY=1), Revealed marker component, check_victory system (enemy Revealed → Victory), spawn_victory_ui (text overlay)
