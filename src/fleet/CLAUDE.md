# fleet/

Fleet composition, budget validation, and lobby tracking.

## Files

- `mod.rs` — ShipSpec (class + loadout), FLEET_BUDGET (1000), hull_cost/weapon_cost, ship_spec_cost/fleet_cost, FleetError, validate_fleet, FleetPlugin
- `lobby.rs` — LobbyTracker resource (submissions + countdown), LobbyPlugin, handle_fleet_submission/handle_cancel_submission observers, tick_lobby_countdown system

## Budget

1000pt total. Hull costs: BB 450, DD 200, Scout 140. Weapon costs: Railgun 50, HeavyVLS 45, HeavyCannon 40, SearchRadar 35, LaserPD 30, LightVLS 25, Cannon 20, NavRadar 20, CWIS 15. Mount downsizing allowed.

## Lobby protocol

- FleetSubmission/CancelSubmission (client→server), LobbyStatus/GameStarted/SubmissionCount (server→client)
- LobbyState: AllSubmitted / WaitingForMore / SubmissionsCancelled
- 3s countdown after all submit. Any cancel resets.
- `--fleet N` CLI flag auto-submits preset fleets
