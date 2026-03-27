# ui/

Client UI — fleet builder and in-game fleet status.

## Files

- `mod.rs` — FleetUiPlugin (spawn/despawn on FleetComposition, systems run in both FleetComposition and GameLobby), FleetStatusPlugin
- `fleet_builder.rs` — FleetBuilderState resource, FleetBuilderMode (Online/Lobby), two-panel layout (ship list + ship detail), popup system (ship picker, weapon picker), submit/cancel toggle, budget display, lobby status text. `spawn_fleet_builder_content()` reusable for embedding in GameLobby.
- `fleet_status.rs` — In-game fleet status sidebar (left edge, ~200px). Ship cards with hull/engine health bars, weapon mount status dots (green/red/gray), ammo counts, cooldown reload bars. Click card to select ship. Destroyed ships grayed out.

## Key behavior

- Fleet builder resets on state exit (FleetBuilderState is client-local)
- Fleet status sidebar spawned on Playing, despawned on exit
- Clone button in fleet builder duplicates ship spec
