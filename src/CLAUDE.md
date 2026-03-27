# Architecture

Library crate (`src/lib.rs`) with two binaries:

- **`src/bin/server.rs`** — headless authoritative server (`MinimalPlugins`, 60Hz tick loop, `--bind` CLI, Edgegap env var support, self-termination on GameOver)
- **`src/bin/client.rs`** — rendering client (`DefaultPlugins`, `--connect` for direct mode, `--lobby-api` + `--name` for lobby mode)

## Module map

| Module | Role |
|---|---|
| `game/` | GameState enum, Team(`u8`), GameConfig, PlayerSlot, Health, Destroyed |
| `ship/` | Ship physics, ShipClass/ShipProfile, Velocity, WaypointQueue, facing, squads, EngineHealth |
| `weapon/` | Mounts, WeaponType/Profile, projectiles, missiles, point defense, damage & repair |
| `radar/` | SNR-based detection, RadarContact entities, RWR bearings, contact tracking |
| `control_point/` | Presence-based capture, scoring, state machine (Neutral/Capturing/Captured/Decapturing) |
| `fleet/` | ShipSpec, budget validation, lobby tracker (submissions + countdown) |
| `input/` | Ship selection, move/target/missile/join/lock modes, enemy numbering, squad commands |
| `camera/` | Strategic zoom, WASD pan, left-drag pan, right-drag orbit |
| `fog/` | Server LOS detection, client ghost fade-out on visibility loss |
| `net/` | Replicon networking: commands, server authority, visibility filtering, materializer |
| `ui/` | Fleet builder UI, in-game fleet status sidebar |
| `lobby/` | Firebase lobby API client, MainMenu + GameLobby UI |
| `map/` | MapBounds, asteroids, MapData (RON), map editor |

## System ordering (Update schedule)

**Server — Ship physics:** update_facing_targets → turn_ships → apply_thrust → apply_velocity (+ space drag) → ship-asteroid collision → check_waypoint_arrival → clamp_to_bounds

**Server — Weapons:** tick_weapon_cooldowns → auto_fire → process_missile_queue

**Server — Missile flight:** advance_missiles → seeker_scan → check_missile_asteroid_hits → check_missile_hits → check_missile_bounds

**Server — Point Defense:** laser_pd_fire → update_laser_beams (track + delayed kill) → cwis_fire

**Server — Projectiles:** advance_projectiles → check_projectile_bounds → check_projectile_hits → check_cwis_hits

**Server — Damage:** tick_repair → mark_destroyed → despawn_destroyed → check_win_condition

**Server — Radar:** update_radar_contacts → cleanup_stale_contacts → update_rwr_bearings

**Server — Networking:** sync_ship_secrets → server_update_visibility (LOS + RadarBit) → clear_lost_targets

**Server — Lobby:** handle_fleet_submission (observer) → handle_cancel_submission (observer) → tick_lobby_countdown

**Client — Visuals** (parallel): waypoint markers, facing arrows, targeting indicators, radar gizmos, fog fade

## Connection flow

**Server:** Setup → WaitingForPlayers (bind, listen, lobby) → Playing (all fleets submitted + 3s countdown)

**Client (lobby):** Setup → MainMenu → GameLobby → Connecting → FleetComposition (auto-submit) → Playing → GameOver → MainMenu

**Client (direct):** Setup → Connecting (`--connect`) → FleetComposition (on TeamAssignment) → Playing → GameOver

**Editor:** Setup → Editor (no networking, dead-end state)

**Lobby flow:** Firebase Cloud Functions API. Creator launches → Edgegap Deploy (or localhost dev). Clients auto-connect with pre-built fleets via AutoFleet resource.

**Direct flow:** `--connect` bypasses MainMenu. Server sends TeamAssignment on connect. All submit → 3s countdown → spawn → Playing.

## Key cross-cutting patterns

- **Client/server split**: Server runs all physics and game logic. Client renders and sends commands via `bevy_replicon` triggers. Server validates team ownership on all commands.
- **Entity replication**: Components registered with `app.replicate::<T>()`. Server uses `FilterRegistry::register_scope::<Entity>()` + `ClientVisibility::set()` for per-client LOS filtering.
- **ShipSecrets pattern**: WaypointQueue/FacingTarget/FacingLocked live on Ship entities (for physics) but replicate via separate ShipSecrets entities (for per-component visibility). ShipSecrets are always visible to owning team, never to enemy. NOTE: ShipSecrets is NOT a Bevy child — standalone with ShipSecretsOwner back-reference, because true children inherit parent visibility.
- **Ghost fade-out**: `On<Remove, Ship>` spawns visual-only ghost that fades out over 0.5s then self-destructs.
- **Entity materializer**: Replicated entities arrive without meshes. Client materializer watches `Added<Ship>` / `Added<Asteroid>` and spawns mesh children.
- **Authorization**: Must use `On<Add, AuthorizedClient>` (not `ConnectedClient`) for sending messages.
- **Team component**: `u8` id for N-team multiplayer. No `opponent()` or `PLAYER`/`ENEMY` constants — all logic is N-team aware.
- **Visual indicators**: All use Bevy Gizmos (immediate mode) — no mesh-based indicators remain.
- **Space drag**: Ships lose ~26% velocity/second. Not realistic but makes ships controllable.
