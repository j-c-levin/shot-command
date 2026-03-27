# lobby/

Game lobby and matchmaking via Firebase Cloud Functions.

## Files

- `mod.rs` — LobbyPlugin, LobbyConfig/PlayerName/CurrentGameId resources, GameInfo/GameDetail/PlayerInfo data types
- `api.rs` — HTTP client functions (background threads + mpsc channels): list_games, create_game, get_game, join_game, launch_game, delete_game, fetch_maps
- `main_menu.rs` — MainMenu UI: game list with 3s polling, create game dialog (map picker), join button, direct connect, refresh
- `game_lobby.rs` — GameLobby UI: player list sidebar, embedded fleet builder, launch button (creator only), 2s polling, auto-connect on server ready

## Key behavior

- Creator launches → Edgegap Deploy API (or localhost in dev mode)
- When server ready, clients auto-connect with pre-built fleets via AutoFleet resource
- `--lobby-api` CLI flag sets Firebase endpoint
