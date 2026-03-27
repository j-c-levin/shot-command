# game/

Core game state and shared types.

## Files

- `mod.rs` — GameState enum (Setup→MainMenu→GameLobby→Connecting→Playing→GameOver / Setup→WaitingForPlayers→Playing→GameOver / Setup→Editor), Team component (`u8` id, with `color()`/`is_friendly()`), GameConfig resource (team_count, players_per_team), PlayerSlot (team + slot index), Detected marker, EnemyVisibility (opacity f32), Health (hull HP, u16), Destroyed marker, DestroyTimer

## Key types

- **GameState**: branching state machine — client path has MainMenu/GameLobby/Connecting, server path has WaitingForPlayers, editor is a dead-end
- **Team**: `u8` id, `color()` returns distinct color per team (up to 8), `is_friendly()` for team comparison
- **GameConfig**: `team_count` (2-8) + `players_per_team` (1-3), set via CLI flags
- **PlayerSlot**: maps each client to a team + slot index via `ClientTeams` resource
