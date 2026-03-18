# Nebulous Shot Command — Claude Notes

## Project

Bevy 0.18 space tactical game inspired by Nebulous: Fleet Command. Player maneuvers ships
to locate and destroy enemies. Physics-based movement with momentum, facing control, and
waypoint queuing. Three ship classes with distinct handling.
Client/server multiplayer architecture with `bevy_replicon` + `bevy_replicon_renet`.

## Build & workflow

```bash
cargo run --bin server                # dev server (headless, 60Hz tick loop)
cargo run --bin client                # dev client (lobby mode, needs Firebase emulator)
cargo run --bin client -- --name Me --lobby-api http://localhost:5001  # lobby with custom name/API
cargo run --bin server -- --bind 0.0.0.0:5000  # server on custom address
cargo run --bin client -- --connect 1.2.3.4:5000  # client to remote server
cargo check                           # quick compilation check
cargo test                            # unit tests only (pure function + World-level, no full App)
cargo build --release --bin server    # optimized server for deployment
cargo run --bin client -- --fleet 1   # auto-submit preset fleet 1 (BB with radar)
cargo run --bin client -- --fleet 2   # auto-submit preset fleet 2 (Scout with nav radar)
./run_game.sh                         # quick dev: server + fleet 1 vs fleet 2
cargo run --bin client -- --editor             # map editor (no networking)
cargo run --bin client -- --editor --map x.ron  # edit existing map file
cargo run --bin server -- --map chokepoint.ron  # server loads designed map
cd infra && firebase emulators:start --only functions,firestore  # local lobby backend
```

Requires **nightly Rust** (`rust-toolchain.toml`). The `.cargo/config.toml` uses `-Z` flags
for share-generics and multi-threaded compilation, plus `build-std` for std rebuilds.

First build from clean is ~4-5 minutes (Bevy is large). Subsequent builds are fast.
**Never run `cargo clean` unless absolutely necessary.**

## Testing

### Philosophy

All tests are **pure-function or World-level only** — no full App, no render context, no asset
server. This keeps `cargo test` fast and avoids GPU/window dependencies. Currently 293 tests.

- **Pure math** (physics, LOS, fade): plain `#[test]`, no imports beyond `bevy::prelude::*`
- **Resource/component presence**: `World::new()` + `world.insert_resource()` / `world.spawn()`
- **Avoid**: spinning up `App` with `DefaultPlugins` in tests

### Test locations

Tests live in `#[cfg(test)]` blocks at the bottom of each module file:

| Module | # | What's tested |
|---|---|---|
| `src/ship/mod.rs` | 50 | Thrust multiplier (facing/away/perpendicular), ship profiles ordering, velocity default, angle math (same/opposite/perpendicular), braking distance, shortest angle delta (positive/negative/wraparound), XZ extraction, facing direction, waypoint queue, steering controller (desired velocity braking/direction/at-target, perpendicular correction, overshoot braking), default mounts per class, squad offset computation (positive/negative), squad move destination (with offset/zero), ship number assignment, ship number default, squad speed limit (caps/no effect), RCS ordering (BB>DD>Scout), RCS range (0..1), EngineHealth (new/floor/is_offline/offline-timer-expired), RepairCooldown, HP pool values (BB/DD/Scout hull+engine), asteroid collision (inside/outside/boundary/class-specific radius) |
| `src/radar/mod.rs` | 26 | Aspect factor (broadside/nose-on/tail-on/range/symmetry/quarter-angle), SNR (distance/RCS/aspect/zero-distance), detection scenarios (BB broadside tracked at 800m, Scout nose-on not tracked, DD broadside at 300m with nav radar, missile detection), ContactTracker ID allocation (sequential, per-team), range cap, destroyer scenarios (signature nose-on, tracked broadside), RWR range (in/out/boundary) |
| `src/weapon/mod.rs` | 27 | Weapon profiles (heavy cannon, cannon, railgun, HeavyVLS, LightVLS, LaserPD, CWIS, SearchRadar, NavRadar values), mount size mapping, weapon categories (incl. Sensor), VLS tube reload, MountSize::fits (same/smaller/rejects larger), MountSize::hp (Large/Medium/Small), Mount::new (all fields/with weapon) |
| `src/weapon/damage.rs` | 34 | Hit zone classification (front/rear/broadside with boundary angles), damage routing (70/30 split per zone), railgun precision routing (90/10 component/hull), damage conservation (all angles + railgun), apply_damage_to_ship end-to-end (front→hull, rear→engines, broadside→component, no cross-contamination), engine/component offline at 0 HP, offline cooldown by mount size (10/15/20s), damage spill (dead engines→hull, no components→hull), repair cooldown reset, repair healing toward floor |
| `src/fleet/mod.rs` | 21 | Hull costs, weapon costs (incl. SearchRadar 35, NavRadar 20), ship spec cost (full/empty), fleet cost, fleet validation (valid/over budget/wrong slots/weapon too large/empty/downsized ok) |
| `src/weapon/missile.rs` | 18 | Intercept point (stationary, moving, zero speed), seeker cone (inside/outside/ahead/behind), spawn_missile components+velocity, flat flight, seeker acquisition, asteroid collision |
| `src/camera/mod.rs` | 19 | CameraLookAt resource, strategic zoom (cursor zoom-in, center zoom-out), camera pan controls, orbit math (yaw/pitch preserves distance, rotates correctly, zero deltas unchanged) |
| `src/fog/mod.rs` | 11 | Ray-asteroid intersection, LOS range+occlusion, opacity fade in/out/clamp |
| `src/weapon/firing.rs` | 9 | Lead calculation (stationary, moving, zero speed), firing arc (turret, forward cone), fire_delay tick, cannon stagger |
| `src/game/mod.rs` | 8 | Team constants, GameState default/variants/fleet_composition, EnemyVisibility default, Health damage/saturation |
| `src/net/server.rs` | 15 | Asteroid exclusion zones (near corners, outside, boundary), rotate_offset (0/90/180/-90 degrees), build_default_map_data (bounds, 2 spawns, 12 asteroids with valid radii/bounds/exclusion zones, 1 control point) |
| `src/weapon/projectile.rs` | 6 | Projectile spawning, direction normalization, advancement, bounds despawn |
| `src/control_point/mod.rs` | 30 | Capture speed (zero/one/four/nine/diminishing), state machine (neutral→capturing→captured→decapturing, all transitions, freezing, decay, multi-frame accumulation, team1 capture), world-level (ship inside/outside/boundary radius, score accumulation, threshold reached/not-reached) |
| `src/map/mod.rs` | 6 | MapBounds contains/clamp/size |
| `src/map/data.rs` | 3 | MapData RON roundtrip serialization, default values, file save/load |
| `src/weapon/pd.rs` | 3 | PD cylinder detection (inside/outside), altitude-independent cylinder check |

## Architecture

Library crate (`src/lib.rs`) with two binaries:

- **`src/bin/server.rs`** — headless authoritative server (`MinimalPlugins`, 60Hz tick loop, `--bind` CLI, Edgegap env var support, self-termination on GameOver)
- **`src/bin/client.rs`** — rendering client (`DefaultPlugins`, `--connect` for direct mode, `--lobby-api` + `--name` for lobby mode)

### Modules

- `src/control_point/` — Control point capture & scoring:
  - `mod.rs` — ControlPoint/ControlPointState/ControlPointRadius/TeamScores components, capture_speed/compute_next_state pure functions, ControlPointPlugin (server: update_control_points, check_score_victory), ControlPointClientPlugin (gizmo sphere, score UI), constants (BASE_CAPTURE_TIME 20s, DECAY_RATE 0.025, SCORE_VICTORY_THRESHOLD 300, DEFAULT_ZONE_RADIUS 100m)
- `src/fleet/` — Fleet composition module:
  - `mod.rs` — ShipSpec (class + loadout), FLEET_BUDGET (1000), hull_cost/weapon_cost, ship_spec_cost/fleet_cost, FleetError, validate_fleet, FleetPlugin
  - `lobby.rs` — LobbyTracker resource (submissions + countdown), LobbyPlugin, handle_fleet_submission/handle_cancel_submission observers, tick_lobby_countdown system
- `src/ui/` — Client UI module:
  - `mod.rs` — FleetUiPlugin (spawn/despawn on FleetComposition state, systems run in both FleetComposition and GameLobby), FleetStatusPlugin
  - `fleet_builder.rs` — FleetBuilderState resource, FleetBuilderMode (Online/Lobby), two-panel fleet builder UI (ship list + ship detail), popup system (ship picker, weapon picker), submit/cancel toggle, budget display, lobby status text. `spawn_fleet_builder_content()` reusable for embedding in GameLobby.
  - `fleet_status.rs` — In-game fleet status sidebar (left edge, ~200px). Ship cards with hull/engine health bars, weapon mount status (online/offline dots, ammo counts), cooldown reload bars. Click card to select ship. Destroyed ships grayed out. Spawned on Playing, despawned on exit.
- `src/lobby/` — Game lobby & matchmaking module:
  - `mod.rs` — LobbyPlugin, LobbyConfig/PlayerName/CurrentGameId resources, GameInfo/GameDetail/PlayerInfo data types
  - `api.rs` — HTTP client functions (background threads + mpsc channels): list_games, create_game, get_game, join_game, launch_game, delete_game, fetch_maps
  - `main_menu.rs` — MainMenu UI: game list with 3s polling, create game dialog (map picker), join button, direct connect, refresh
  - `game_lobby.rs` — GameLobby UI: player list sidebar, embedded fleet builder, launch button (creator only), 2s polling, auto-connect on server ready
- `src/game/` — GameState enum (Setup→MainMenu→GameLobby→Connecting→Playing→GameOver / Setup→WaitingForPlayers→Playing→GameOver / Setup→Editor), Team component (`u8` id), Detected marker, EnemyVisibility (opacity), Health (hull HP, u16), Destroyed marker, DestroyTimer
- `src/map/` — MapBounds resource, Asteroid/AsteroidSize components, GroundPlane marker
  - `data.rs` — MapData/BoundsDef/SpawnPoint/AsteroidDef/ControlPointDef structs (Serialize/Deserialize), save_map_data/load_map_data RON functions
  - `editor.rs` — MapEditorPlugin (gated on GameState::Editor): EditorState/EditorTool resources, EditorAsteroid/EditorControlPoint/EditorSpawn markers, click-to-place/drag-to-move/scroll-resize/delete interactions, left panel UI (entity palette + file ops), save/load popup, bounds gizmos, entity highlight gizmos, editor_camera_zoom_or_resize (zoom when no asteroid selected, resize when asteroid selected)
- `src/ship/` — Ship marker, ShipClass enum (Battleship/Destroyer/Scout), ShipProfile (incl. hp, engine_hp, rcs, collision_radius), Velocity, WaypointQueue, FacingTarget/FacingLocked, TargetDesignation, ShipNumber(u8) (1-9 per team), SquadMember { leader, offset } (squad formation), ShipSecrets/ShipSecretsOwner (per-component visibility, incl. RadarActiveSecret and RwrBearings), EngineHealth (hp/max_hp/offline_timer, replicated), RepairCooldown, ShipPhysicsPlugin (server, apply_thrust gates on EngineHealth) / ShipVisualsPlugin (client), spawn_server_ship (takes &ShipSpec + ship_number, inits EngineHealth + per-mount HP), spawn_server_ship_default (convenience with default loadout)
- `src/radar/` — Radar & detection module:
  - `mod.rs` — Constants (SIGNATURE_THRESHOLD 0.1, TRACK_THRESHOLD 0.4, SIGNATURE_FUZZ_RADIUS 75m, MISSILE_RCS, PROJECTILE_RCS), compute_aspect_factor/compute_snr pure functions, RadarActive (server-only marker), RadarActiveSecret (on ShipSecrets), ContactLevel (Signature/Track), RadarContact/ContactSourceShip/ContactTeam/ContactId/ContactKind components, ContactTracker resource, RadarPlugin (server) / RadarClientPlugin (client)
  - `contacts.rs` — update_radar_contacts (SNR-based contact creation for ships+missiles+projectiles), cleanup_stale_contacts, fuzz_offset, best_radar_range helpers
  - `rwr.rs` — RwrBearings component (on ShipSecrets), is_in_rwr_range pure function, update_rwr_bearings server system (with asteroid LOS blocking)
  - `visuals.rs` — Client gizmos: draw_radar_status_gizmos (range circle when active), draw_radar_signature_gizmos (pulsing orange), draw_radar_track_gizmos (red diamond), draw_tracked_missile_gizmos (orange X), draw_rwr_gizmos (yellow bearing lines)
- `src/weapon/` — Weapon system:
  - `mod.rs` — MountSize (with hp(): Large=150, Medium=100, Small=75), WeaponType (HeavyCannon/Cannon/Railgun/HeavyVLS/LightVLS/LaserPD/CWIS/SearchRadar/NavRadar), WeaponCategory (Cannon/Missile/PointDefense/Sensor), FiringArc, WeaponProfile, WeaponState (incl. tubes_loaded, tube_reload_timer for VLS), Mount (with hp/max_hp/offline_timer), Mount::new(), Mounts component, MissileQueue/MissileQueueEntry
  - `projectile.rs` — Projectile/ProjectileVelocity/ProjectileDamage/ProjectileOwner/RailgunRound/CwisRound components, spawn_projectile, ProjectilePlugin (advance, bounds, directional hit detection with apply_damage_to_ship, CWIS hit detection)
  - `firing.rs` — compute_lead_position, is_in_firing_arc, tick_weapon_cooldowns, auto_fire system
  - `missile.rs` — Missile/MissileTarget/MissileVelocity/MissileDamage/MissileOwner components, compute_intercept_point, is_in_seeker_cone, spawn_missile, MissilePlugin (flat flight, seeker cone acquisition, asteroid collision, ship collision, bounds cleanup). Simplified: no altitude phases, flat flight with seeker cone, destroyed by asteroids.
  - `pd.rs` — Point defense systems: is_in_pd_cylinder (vertical cylinder check), probability-based kills (no missile HP), LaserBeam/LaserBeamTarget/LaserBeamTimer entities (visible beam tracking missile in real-time, delayed kill 0.15s after beam appears), CWIS visual tracers. LaserPD range 300m, CWIS 100m kill / 150m visual (doubled to 200m/300m for radar-tracked missiles). 0.2s retarget delay. PdPlugin
  - `damage.rs` — HitZone (Front/Rear/Broadside), DamageTarget (Hull/Engines/Component/HullOrEngines), classify_hit_zone (angle-based zone from impact dir + ship forward), route_damage (70/30 split), apply_damage_to_ship (directional routing with railgun override), apply_to_target (per-pool damage with offline triggers), offline_cooldown_secs (by mount size: Small=10s, Medium=15s, Large=20s), ENGINE_OFFLINE_COOLDOWN_SECS (15s), REPAIR_DELAY_SECS (5s), REPAIR_RATE_HP_PER_SEC (20), tick_repair (passive auto-repair to 10% floor), DamagePlugin: tick_repair → mark_destroyed (with 1s delay timer) → despawn_destroyed (cleanup ShipSecrets) → check_win_condition (broadcast GameResult)
- `src/camera/` — CameraLookAt resource, LeftDragState resource (drag detection for pan vs click discrimination), strategic zoom (cursor zoom-in, center zoom-out), WASD pan (S is stop-only, not camera pan), left-click drag pan, right-click drag orbit (yaw+pitch, Normal mode only)
- `src/input/` — Ship selection (left-click without drag, left-click ground = deselect; left-click drag = camera pan via LeftDragState). 2x collision radius invisible picking sphere for easier selection. Multi-select via Shift+click or Shift+number-key (toggles ship in/out of selection group). Move commands (right-click only in move mode): quick click = move, hold+drag = move + face direction (formation offsets rotate). Space toggle move mode. Shift+right-click = append waypoint. Alt+right-click facing lock (works on any surface incl. asteroids). L facing lock mode. K target mode (number keys target numbered enemies, incl. radar tracks via TargetByContactCommand). M missile mode (number keys fire at numbered enemies). K/M mode with multiple selected ships fires coordinated volleys. J join mode (click friendly or press number to assign squad). R key toggles radar on/off for selected ships (rejected if all radar mounts offline). 1-9 number-key ship selection. InputMode enum (Normal/Move/Lock/Target/Missile/Join) — all modes mutually exclusive. Systems chained: handle_keyboard → update_enemy_numbers → handle_number_keys. MoveGestureState tracks right-click drag for facing commands with preview gizmos. EnemyNumbers resource dynamically assigns 1-9 to visible enemies + radar tracks in K/M mode (stable numbers via source_numbers map, survives contact re-detection). Friendly ship numbers hidden in K/M mode. ModeIndicatorText shows current mode in bottom-left. SquadHighlight marker. S key = full stop (propagates to squad). All commands emit network triggers.
- `src/fog/` — Server: LOS detection (distance+raycast) drives replicon visibility filtering. Client: FogClientPlugin with ghost entity fade-out on visibility loss.
- `src/net/` — Networking module:
  - `mod.rs` — LocalTeam resource, PROTOCOL_ID constant
  - `commands.rs` — MoveCommand, FacingLockCommand, FacingUnlockCommand, TargetCommand, ClearTargetCommand, JoinSquadCommand, RadarToggleCommand, TargetByContactCommand, FleetSubmission, CancelSubmission (client→server with MapEntities), TeamAssignment, GameResult, LobbyStatus, GameStarted (server→client), LobbyState enum (incl. OpponentSubmitted/OpponentComposing)
  - `server.rs` — ServerNetPlugin: renet transport, connection/auth handling, team assignment, replication registration, fleet/asteroid spawning, command handlers with team validation (move, facing, target, target-by-contact, radar toggle, join squad), squad move propagation (leader move → followers move with offset), orphan squad cleanup, LosBit + RadarBit visibility filtering (ships by LOS, contacts by team, missiles by LOS+radar), ShipSecrets sync (waypoints, facing, targeting, squad, radar active), target clearing (requires loss of both LOS and radar track), disconnection handling
  - `client.rs` — ClientNetPlugin: renet transport, team assignment observer (→FleetComposition), lobby status observer, game started observer (→Playing), ground plane setup, materializer/asteroid registration, CurrentLobbyState resource
  - `materializer.rs` — Spawns meshes for replicated Ship, Asteroid, Projectile, and Missile entities on client. Ship number labels (below ship, font 14, hidden in K/M mode). Enemy number labels (white, below enemy ships + radar contacts, active in K/M mode). Squad connection lines (gizmo lines from follower to leader), squad info labels ("Following: N" / "Squad: N"). Targeting gizmos (red line from ship to target, works for radar-tracked targets via contact position fallback). `]` key debug visuals toggle (seeker cones, PD range circles, visual LOS range, radar-boosted CWIS ranges). Explosion effects (two sizes: ship impact vs PD kill). LaserBeam visual tracking. All indicators use Bevy Gizmos (immediate mode) — no mesh-based indicators remain.

### System ordering (Update schedule)

**Server — Ship physics chain:** 1. Update facing targets → 2. Turn ships → 3. Apply thrust → 4. Apply velocity (with space drag) → 5. Ship-asteroid collision (push out + zero velocity) → 6. Check waypoint arrival → 7. Clamp to bounds

**Server — Weapons:** tick_weapon_cooldowns → auto_fire (spawn projectiles)

**Server — Missiles:** process_missile_queue (after auto_fire in weapon chain)

**Server — Missile flight:** advance_missiles → seeker_scan → check_missile_asteroid_hits → check_missile_hits → check_missile_bounds

**Server — Point Defense:** laser_pd_fire → update_laser_beams (track + delayed kill), cwis_fire

**Server — Projectiles:** advance_projectiles → check_projectile_bounds → check_projectile_hits → check_cwis_hits

**Server — Damage:** tick_repair → mark_destroyed → despawn_destroyed → check_win_condition

**Server — Radar:** update_radar_contacts → cleanup_stale_contacts → update_rwr_bearings

**Server — Networking:** sync_ship_secrets (incl. RadarActiveSecret) → server_update_visibility (LOS + RadarBit per-client) → clear_lost_targets (checks both LOS and radar track)

**Client — Visual indicators** (parallel): waypoint markers, facing direction arrows (read from ShipSecrets), targeting indicators (incl. radar contact fallback)

**Client — Radar visuals** (parallel): radar range circle, signature pulse, track diamond, tracked missiles, RWR bearings

**Server — Lobby (WaitingForPlayers):** handle_fleet_submission (observer) → handle_cancel_submission (observer) → tick_lobby_countdown (Update)

**Client — Fleet UI (FleetComposition):** rebuild_fleet_list, rebuild_ship_detail, spawn_popup, handle clicks, update_budget_text, update_status_text, update_submit_button

**Client — Fog:** fade_out_ghosts (fading ghost entities from visibility loss)

### Key patterns

- **Client/server split**: Server runs all physics and game logic. Client renders and sends commands via `bevy_replicon` triggers. Server validates team ownership on all commands.
- **Entity replication**: `bevy_replicon` 0.39 + `bevy_replicon_renet` 0.15. Components registered with `app.replicate::<T>()`. Server uses `FilterRegistry::register_scope::<Entity>()` + `ClientVisibility::set()` for per-client LOS filtering.
- **ShipSecrets pattern**: WaypointQueue/FacingTarget/FacingLocked live on Ship entities (for physics) but replicate via separate ShipSecrets entities (for per-component visibility). ShipSecrets are always visible to owning team, never to enemy. Server syncs Ship→ShipSecrets each frame. NOTE: ShipSecrets is NOT a Bevy child entity — standalone with ShipSecretsOwner back-reference, because true children inherit parent visibility.
- **Ghost fade-out**: When replicon despawns an enemy ship (visibility lost), `On<Remove, Ship>` observer spawns a visual-only ghost entity at the same position that fades out over 0.5s, then self-destructs.
- **Entity materializer**: Replicated entities arrive without meshes. Client materializer watches `Added<Ship>` / `Added<Asteroid>` and spawns appropriate mesh children + `Visibility::Visible`.
- **Authorization**: Must use `On<Add, AuthorizedClient>` (not `ConnectedClient`) for sending messages — clients can't receive messages until protocol check completes.
- **Space drag**: Ships lose ~26% velocity/second. Not realistic but makes ships feel controllable and assists braking.
- **Physics model**: Velocity persists (momentum/drift). Steering controller computes desired velocity, then thrusts to correct. Worst-case deceleration (thruster_factor) used for braking calculations. Ships brake to stop when queue is empty.
- **Facing lock/unlock**: Unlocked ships auto-face waypoint. Locked ships maintain player-set facing. Alt+right-click to lock, alt+click-ship or L to unlock.
- **Waypoint queue**: Right-click = clear + single waypoint. Shift+right-click = append.
- **Team component** uses `u8` id for multiplayer. First client = Team(0), second = Team(1).
- **Visual LOS range**: 400m for all ship classes. Ship models only render within visual LOS. Radar extends awareness beyond visual range via RadarContact entities. Visual LOS guarantees track-level radar detection (`apply_visual_los_boost`) — if you can see it, radar tracks it regardless of RCS/aspect.
- **Weapon system**: Mounts are sized slots (Large/Medium/Small) per ship class, each with HP (150/100/75 by size). Weapons auto-fire at designated targets when in range+arc. Offline mounts (hp==0) cannot fire. Projectiles are independent server entities with velocity — no hitscan. Cooldown ticks every frame regardless of targeting. Lead calculation predicts target position. Railguns require forward-facing (±10°), fire RailgunRound marker for precision component targeting. Damage values: HC 25×3=75/burst, CN 20/shot, RG 50, missiles 80. Missile launchers (HeavyVLS/LightVLS) fire from MissileQueue. VLS uses tubes_loaded + tube_reload_timer on WeaponState (3s per-tube reload, queue capped at loaded tubes). Point defense (LaserPD/CWIS) auto-engages incoming missiles — probability-based kills, no missile HP.
- **Missile system**: M key toggles missile mode (gated by VLS presence). Right-click ground fires missiles at a point, click enemy fires at entity (with tracking). Simplified flat flight with seeker cone acquisition — no altitude/avoidance phases. Missiles destroyed by asteroid collision. MissileQueue lives on Ship entities and syncs to ShipSecrets.
- **Point defense**: LaserPD range 300m with visible beam (LaserBeam/LaserBeamTarget/LaserBeamTimer entities track missile in real-time, delayed kill 0.15s after beam appears). CWIS 100m kill radius / 150m visual tracer range (both doubled to 200m/300m when engaging radar-tracked missiles). Probability-based kills. 0.2s retarget delay between engagements.
- **Ship-asteroid collision**: Ships collide with asteroids using `asteroid_radius + ship_collision_radius`. On collision, ship is pushed to the asteroid's edge and velocity is zeroed (hard stop). Runs after velocity application in the physics chain.
- **Ground plane**: Invisible (transparent), 3x map bounds for click targeting.
- **Explosions**: Two sizes — ship impact (large) vs PD kill (small).
- **Targeting**: K+number key targets enemy (ship or radar track). K again clears. Target auto-clears when enemy loses both visual LOS AND radar track (signature alone not enough). TargetDesignation synced via ShipSecrets (team-private). Radar-only targets use TargetByContactCommand (server resolves contact → source ship).
- **Directional damage**: Three HP pools: Hull (permanent, no repair), Engines (EngineHealth), Components (per-mount HP based on MountSize). Hit angle vs ship facing determines zone: Front (±45°) → 70% hull / 30% component, Rear (±45° from tail) → 70% engines / 30% component, Broadside (45–135°) → 70% component / 30% hull-or-engines. Railgun bypasses zones: 90% component, 10% hull. Binary performance — fully operational until 0 HP, then offline. Component HP by mount size: Large=150, Medium=100, Small=75.
- **Passive repair**: 5s after last hit (RepairCooldown), damaged pools auto-repair toward 10% floor at 20hp/s. At 0 HP, offline timer counts down (Small=10s, Medium=15s, Large=20s, engines=15s), then HP restores to floor. Hull never repairs. `Without<Destroyed>` filter on tick_repair.
- **Engine offline**: EngineHealth at 0 HP → apply_thrust skipped → ship drifts on space drag (~26%/s velocity bleed). After offline timer + floor restore, ship can thrust again at 10% capacity.
- **Destruction**: Ships at 0 hull HP get Destroyed marker + 1s delay timer, then despawn (ship + ShipSecrets). Ghost fade-out fires on despawn. Win condition: all enemy ships destroyed → GameResult broadcast → GameOver state.
- **Fleet status sidebar**: Left-edge panel (FleetStatusPlugin) with ship cards showing hull/engine bars, weapon mount status dots (green/red/gray), ammo counts, cooldown reload bars. Click card to select ship. Destroyed ships grayed out. Spawned on Playing.
- **Move mode**: Space key enters move mode. Right-click only moves in move mode. Quick click = move only. Hold+drag right-click = move + face direction (MoveCommand.facing field). All modes (Space/K/M/J/L) are mutually exclusive. Mode indicator text in bottom-left. Gesture preview shows destination circle + facing line + follower predicted positions during drag.
- **Formation rotation**: When leader gets move+facing, follower offsets are rotated by the heading delta (rotate_offset pure function in ship module). Followers get rotated destinations + same facing lock.
- **Cannon stagger**: fire_delay field on WeaponState, 0.5s between each cannon firing on a ship (WeaponCategory::Cannon only, CANNON_STAGGER_DELAY constant).
- **Enemy numbering**: K or M mode dynamically assigns numbers 1-9 to visible enemies + radar-tracked contacts. Numbers are stable via source_numbers map — survives contact entity re-creation when ships leave/re-enter radar range. Number keys in K mode target enemy, in M mode fire missile at enemy. White labels below enemy ships and radar track diamonds. Friendly numbers hidden in K/M mode to avoid confusion.
- **Squad formation**: J key enters join mode; click friendly ship or press its number to assign. SquadMember { leader, offset } on followers (uses #[derive(MapEntities)] with #[entities] for replication entity mapping). SquadSpeedLimit { top_speed, acceleration, turn_rate, turn_acceleration } caps all movement stats to minimum across squad. Leader move orders propagate to followers with offset applied. S key stop propagates to followers + unlocks their facing. Direct move to a follower breaks formation. Squad cycles prevented (chain walk up to 10 hops). Leader joining another squad reassigns followers to new leader. Orphan cleanup on leader destroyed.
- **Ship numbers**: ShipNumber(1-9) assigned from fleet list index. Press 1-9 to select by number. Number labels float below friendly ships. Clone button in fleet builder duplicates ship spec.
- **Fleet composition**: 1000pt budget. Hull costs: BB 450, DD 200, Scout 140. Weapon costs: Railgun 50, HeavyVLS 45, HeavyCannon 40, SearchRadar 35, LaserPD 30, LightVLS 25, Cannon 20, NavRadar 20, CWIS 15. Mount downsizing allowed. Server-authoritative lobby validates and stores submissions. FleetBuilderState is client-local, reset on state exit. `--fleet N` CLI flag auto-submits preset fleets. Ship HP: BB 1200 hull / 300 engine, DD 600 / 180, Scout 300 / 120.
- **Radar system**: SearchRadar (medium mount, 800m, 35pts) and NavRadar (small mount, 500m, 20pts). Radar starts OFF, R key toggles. SNR formula: `(BaseRange²/Distance²) × RCS × AspectFactor`. Three awareness layers: (1) Signature (low SNR, pulsing orange circle, fuzzed position), (2) Track (high SNR, red diamond, precise position, full fire control), (3) Visual LOS (400m, ship model). RadarContact entities are standalone (like ShipSecrets), replicated to detecting team only via RadarBit. RadarActive is server-only; client reads via RadarActiveSecret on ShipSecrets. RWR gives yellow bearing lines toward enemy radar sources (free with radar hardware). Missiles/projectiles always instantly tracked if inside radar range. Asteroids block radar LOS. Team-shared: any teammate's track is everyone's track.
- **Visual indicators**: All in-game indicators use Bevy Gizmos (immediate mode). Green circles for selection, gray circles for squad highlights, red lines for targeting (incl. radar contacts), blue lines for waypoints, yellow lines for facing lock, cyan lines for squad connections, weapon range circles in K mode, blue circle for active radar range, orange pulsing circles for radar signatures, red diamonds for radar tracks, orange X for tracked missiles, yellow lines for RWR bearings. `]` key toggles debug visuals (PD ranges, visual LOS, radar-boosted CWIS ranges). No mesh-based indicators remain.
- **Control points**: ControlPoint entity at map center with ControlPointState (Neutral→Capturing→Captured→Decapturing), ControlPointRadius(100m), TeamScores component. Presence-based capture: count alive ships in radius, majority makes progress, ties freeze, empty decays. Speed = sqrt(net_advantage) / 20s. Two-phase swing: must decapture to neutral before recapturing. Captured points score 1pt/s, first to 300 wins. Annihilation still instant-wins. Gizmo wireframe sphere (two perpendicular circles), color pulsing during capture/decapture, solid team color when captured. Score display at top center.
- **Lobby protocol**: FleetSubmission/CancelSubmission (client→server), LobbyStatus/GameStarted (server→client). LobbyTracker resource tracks submissions + countdown. Server stays in WaitingForPlayers throughout. LobbyState includes OpponentSubmitted (opponent done) and OpponentComposing (opponent cancelled).
- **Map editor**: Dev tool launched via `--editor` flag. GameState::Editor is a dead-end state (never transitions to Playing). Editor skips all networking plugins. MapData struct (RON format) stores bounds, spawns, asteroids, control points. Editor entities use EditorAsteroid/EditorControlPoint/EditorSpawn markers (distinct from game Asteroid/ControlPoint components). Entity-data sync uses position proximity matching. Camera zoom and left-drag pan are gated out of Editor state; editor provides its own scroll handler (editor_camera_zoom_or_resize) that resizes asteroids or zooms camera depending on selection.
- **Map files**: RON files in `assets/maps/`. Server `--map name.ron` loads designed maps; without `--map`, falls back to random generation. spawn_map_entities() is shared between server and editor. EditorMapData resource holds the live MapData being edited; changes sync to data on drag-release, delete, and placement.

### Connection flow

**Server:** Setup → WaitingForPlayers (bind, listen, lobby) → Playing (when both fleets submitted + 3s countdown)
**Client (lobby mode):** Setup → MainMenu (browse/create/join games) → GameLobby (build fleet, wait for launch) → Connecting → FleetComposition (auto-submit) → Playing → GameOver → MainMenu
**Client (direct mode):** Setup → Connecting (--connect flag) → FleetComposition (on TeamAssignment) → Playing → GameOver
**Editor:** Setup → Editor (no networking, no state transitions)

**Lobby flow:** Client creates or joins a game via Firebase Cloud Functions API. In GameLobby, players build fleets locally and creator launches when 2 players present. Launch calls Edgegap Deploy API (or returns localhost in dev mode). When server is ready, clients auto-connect with pre-built fleets via AutoFleet resource.

**Direct flow:** `--connect` flag bypasses MainMenu, connects immediately to specified address. FleetComposition state remains for this path.

Server sends TeamAssignment immediately on connect. Clients enter FleetComposition independently (no waiting for opponent). Both submit fleets → 3s countdown → server spawns from specs → Playing. Either can cancel during countdown to re-edit. Server spawns fleets from LobbyTracker submissions (or default fleet as fallback) + 12 random asteroids with exclusion zones around spawn corners.

## Bevy 0.18 notes

- `MeshPickingPlugin` is NOT in `DefaultPlugins` — must add explicitly alongside DefaultPlugins
- `OnEnter` for default state fires before `Startup` commands are flushed — cannot query Startup-spawned entities
- Use `commands.add_observer(fn)` (global) when target entities may not exist yet; filter by component inside
- `hotpatching` and `reflect_auto_register` features disabled (Cranelift incompatibility on macOS)
- Picking uses observer pattern: `.observe(|event: On<Pointer<Click>>| { ... })`
- Use `event.event_target()` not `event.target()` in picking observers
- Meshes: `Mesh3d(handle)`, Materials: `MeshMaterial3d(handle)`
- States: `#[derive(States)]` with `init_state::<T>()`
- Ambient light: `GlobalAmbientLight` as resource, NOT `AmbientLight` as entity
- `Image::new_fill` requires 5th arg: `RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD`
- `emissive` field on `StandardMaterial` takes `LinearRgba`, not `Color` — use `LinearRgba::new(r, g, b, a)`
- `MinimalPlugins` does NOT include `StatesPlugin` — add it explicitly when using states on server

## bevy_replicon 0.39 notes

- `ConnectedClient` vs `AuthorizedClient`: messages/replication only work after auth. Use `On<Add, AuthorizedClient>` for post-connect logic.
- `ReplicationRegistry::despawn` hook: called AFTER entity is removed from entity map. Cannot keep entity alive for fade — use ghost entities instead.
- `FilterRegistry::register_scope::<Entity>()` for manual entity-level visibility. Call `ClientVisibility::set(entity, bit, visible)` each frame.
- Client events: `add_mapped_client_event::<T>(Channel::Ordered)` + `MapEntities` derive with `#[entities]` on Entity fields.
- Server events: `add_server_event::<T>(Channel::Ordered)`. Send via `commands.server_trigger(ToClients { mode, message })`.
- Client sends triggers via `commands.client_trigger(event)` (from `ClientTriggerExt` trait).
- Server receives client events as `On<FromClient<T>>` observers.

## Roadmap

See `docs/plans/2026-03-14-feature-brainstorm-v3.md` for full details.

**Phase 1: Core Simulation — COMPLETE.** Physics-based movement, facing control,
waypoint queuing, ship classes (battleship/destroyer/scout). See design doc at
`docs/plans/2026-03-14-phase1-core-simulation-design.md`.

**Phase 2: Multiplayer — COMPLETE.** Headless authoritative server + client binaries,
bevy_replicon entity replication, per-client LOS visibility filtering, command channel
with team validation, ghost entity fade-out, ShipSecrets per-component visibility,
space drag, uniform vision range. See design doc at
`docs/plans/2026-03-15-phase2-multiplayer-design.md`.

**Phase 3a: Mount Points & Cannons — COMPLETE.** Three cannon types (heavy cannon,
cannon, railgun), K-key targeting, simulated projectile entities, HP damage,
ship destruction with delayed despawn, win/lose condition. See design doc at
`docs/plans/2026-03-15-phase3a-weapons-design.md`.

**Phase 3b: Missiles & Point Defense — COMPLETE.** Missile launchers (HeavyVLS/LightVLS)
with per-tube reload (10s), simplified flat-flight missiles with seeker cone, asteroid
collision, probability-based PD (LaserPD 300m with visible beam tracking + delayed kill,
CWIS 100m kill / 150m visual), M-key missile mode, explosion effects, F3 debug visuals,
strategic camera zoom, selection indicator improvements. See design doc at
`docs/plans/2026-03-15-phase3b-missiles-pd-design.md`.

**Phase 3c: Fleet Composition Screen — COMPLETE.** Pre-game fleet builder with 1000pt
budget, clickable Bevy UI (two-panel layout: ship list + weapon slots), server-authoritative
lobby with submit/cancel/countdown, spec-based fleet spawning, mount downsizing, asteroid
exclusion zones. See design doc at `docs/plans/2026-03-16-phase3c-fleet-composition-design.md`.

**QoL Features — COMPLETE.** Clone ship in fleet builder, squad formations (J key join,
SquadMember with offset, SquadSpeedLimit with all 4 stats, move propagation with formation
rotation, cycle prevention, leader reassignment), cannon stagger (0.5s delay), ship number
keys (1-9), explicit move mode (Space), right-click drag for facing, enemy numbering in
K/M modes (dynamic, stable), all indicators converted to Bevy Gizmos, weapon range circles
in K mode, formation preview during drag. See design docs at
`docs/plans/2026-03-16-qol-features-design.md`,
`docs/plans/2026-03-17-input-overhaul-design.md`,
`docs/plans/2026-03-17-formation-facing-design.md`.

**Phase 4a: Radar & Detection — COMPLETE.** Mountable radar equipment (SearchRadar 800m
medium, NavRadar 500m small), SNR-based detection with RCS and aspect angle, signature/track
thresholds, RWR bearing lines, RadarContact entities for beyond-visual-range awareness,
PD radar integration (CWIS 2x range for radar-tracked), K/M mode targeting of radar tracks,
stable enemy numbering, `--fleet` CLI presets. See design doc at
`docs/plans/2026-03-17-phase4a-radar-detection-design.md`.

**Phase 4b — Fire Control Integration: WON'T DO.** Current radar system already
gates targeting at Track level; additional accuracy degradation adds frustration
without meaningful decisions.

**Phase 4c — Control Points & Win Conditions — COMPLETE.** Single control point
at map center, presence-based capture with sqrt(n) diminishing returns, two-phase
swing (decapture then recapture), 1pt/s scoring to 300 win threshold, annihilation
still wins instantly, gizmo wireframe sphere indicator with color pulsing, score
display UI. See design doc at
`docs/plans/2026-03-17-phase4c-control-points-design.md`.

**Phase 5: Directional Damage & Repair — COMPLETE.** Three HP pools (hull/engines/
per-mount components), directional hit zones (front/rear/broadside with 70/30 split),
railgun precision component targeting (90/10), binary performance (online/offline),
passive auto-repair to 10% floor with mount-size-based offline cooldowns (Small=10s,
Medium=15s, Large=20s), engine offline = adrift. Fleet status sidebar UI (left-edge
ship cards with health bars, weapon status, ammo, cooldown bars, click-to-select).
RWR asteroid LOS blocking fix. See design doc at
`docs/plans/2026-03-17-phase5-damage-repair-design.md` and
`docs/plans/2026-03-17-fleet-status-sidebar-design.md`.

**Phase 6: Maps & Editor — COMPLETE.** Map editor dev tool (`--editor` flag),
RON map files (`assets/maps/`), server `--map` loading, entity palette UI,
click-to-place/drag-to-move/scroll-resize/delete interactions, save/load popup,
bounds gizmos. See design doc at
`docs/plans/2026-03-18-phase6-maps-editor-design.md`.

**Phase 7: Cloud Deployment & Game Lobby — IN PROGRESS.** Firebase lobby backend
(Cloud Functions + Firestore), Edgegap game servers, Pulumi infrastructure, CI/CD
with GitHub Actions, client main menu + game lobby UI with fleet building in lobby.
Server Edgegap support (env var port binding, self-termination, Dockerfile) complete.
Firebase Cloud Functions, Pulumi infra, CI/CD pipeline, client lobby UI all implemented.
Needs end-to-end testing with Firebase emulator + Edgegap account setup. See design at
`docs/plans/2026-03-18-phase7-cloud-deployment-design.md` and implementation plan at
`docs/plans/2026-03-18-phase7-implementation-plan.md`.

**Dropped:** Beam weapons (from original Phase 5 brainstorm).

**Known bugs:**
- (none currently)

**Recently completed (this session):**
- Phase 7 implementation: server Edgegap support, Firebase Cloud Functions lobby API, Pulumi infrastructure, GitHub Actions CI/CD, client MainMenu + GameLobby UI with fleet building

## Pre-approvals

The following tools and skills are pre-approved for autonomous use:
- All file read/write/edit operations
- All bash commands for building, testing, and running
- All glob and grep searches
- All LSP operations
- All MCP tools (context7, firebase, playwright)
- All skills (superpowers, bevy, domain-driven-design, etc.)
- All agent/subagent dispatching

## Git notes

GPG signing may not be available in all environments. Use `git -c commit.gpgsign=false commit` if needed.

## Reference projects

- Bevy 0.18 examples: `~/.cargo/registry/src/index.crates.io-*/bevy-0.18.*/examples/`
