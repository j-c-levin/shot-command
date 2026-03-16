# Nebulous Shot Command — Claude Notes

## Project

Bevy 0.18 space tactical game inspired by Nebulous: Fleet Command. Player maneuvers ships
to locate and destroy enemies. Physics-based movement with momentum, facing control, and
waypoint queuing. Three ship classes with distinct handling.
Client/server multiplayer architecture with `bevy_replicon` + `bevy_replicon_renet`.

## Build & workflow

```bash
cargo run --bin server                # dev server (headless, 60Hz tick loop)
cargo run --bin client                # dev client (rendering window)
cargo run --bin server -- --bind 0.0.0.0:5000  # server on custom address
cargo run --bin client -- --connect 1.2.3.4:5000  # client to remote server
cargo check                           # quick compilation check
cargo test                            # unit tests only (pure function + World-level, no full App)
cargo build --release --bin server    # optimized server for deployment
./run_game.sh                         # quick dev: launches server + client in background
```

Requires **nightly Rust** (`rust-toolchain.toml`). The `.cargo/config.toml` uses `-Z` flags
for share-generics and multi-threaded compilation, plus `build-std` for std rebuilds.

First build from clean is ~4-5 minutes (Bevy is large). Subsequent builds are fast.
**Never run `cargo clean` unless absolutely necessary.**

## Testing

### Philosophy

All tests are **pure-function or World-level only** — no full App, no render context, no asset
server. This keeps `cargo test` fast and avoids GPU/window dependencies.

- **Pure math** (physics, LOS, fade): plain `#[test]`, no imports beyond `bevy::prelude::*`
- **Resource/component presence**: `World::new()` + `world.insert_resource()` / `world.spawn()`
- **Avoid**: spinning up `App` with `DefaultPlugins` in tests

### Test locations

Tests live in `#[cfg(test)]` blocks at the bottom of each module file. Currently 110 tests:

| Module | # | What's tested |
|---|---|---|
| `src/ship/mod.rs` | 28 | Thrust multiplier (facing/away/perpendicular), ship profiles ordering, velocity default, angle math (same/opposite/perpendicular), braking distance, shortest angle delta (positive/negative/wraparound), XZ extraction, facing direction, waypoint queue, steering controller (desired velocity braking/direction/at-target, perpendicular correction, overshoot braking), default mounts per class |
| `src/weapon/missile.rs` | 18 | Intercept point (stationary, moving, zero speed), seeker cone (inside/outside/ahead/behind), spawn_missile components+velocity, flat flight, seeker acquisition, asteroid collision |
| `src/camera/mod.rs` | 14 | CameraLookAt resource, strategic zoom (cursor zoom-in, center zoom-out), camera pan controls |
| `src/fog/mod.rs` | 11 | Ray-asteroid intersection, LOS range+occlusion, opacity fade in/out/clamp |
| `src/weapon/mod.rs` | 10 | Weapon profiles (heavy cannon, cannon, railgun, HeavyVLS, LightVLS, LaserPD, CWIS values), mount size mapping, weapon categories, VLS tube reload |
| `src/game/mod.rs` | 7 | Team constants, GameState default/variants, EnemyVisibility default, Health damage/saturation |
| `src/weapon/firing.rs` | 7 | Lead calculation (stationary, moving, zero speed), firing arc (turret, forward cone) |
| `src/weapon/projectile.rs` | 6 | Projectile spawning, direction normalization, advancement, bounds despawn |
| `src/map/mod.rs` | 6 | MapBounds contains/clamp/size |
| `src/weapon/pd.rs` | 3 | PD cylinder detection (inside/outside), altitude-independent cylinder check |

## Architecture

Library crate (`src/lib.rs`) with two binaries:

- **`src/bin/server.rs`** — headless authoritative server (`MinimalPlugins`, 60Hz tick loop, `--bind` CLI)
- **`src/bin/client.rs`** — rendering client (`DefaultPlugins`, `--connect` CLI)

### Modules

- `src/game/` — GameState enum (Setup→WaitingForPlayers→Playing→GameOver / Setup→Connecting→Playing→GameOver), Team component (`u8` id), Detected marker, EnemyVisibility (opacity), Health (u16), Destroyed marker, DestroyTimer
- `src/map/` — MapBounds resource, Asteroid/AsteroidSize components, GroundPlane marker
- `src/ship/` — Ship marker, ShipClass enum (Battleship/Destroyer/Scout), ShipProfile (incl. hp, collision_radius), Velocity, WaypointQueue, FacingTarget/FacingLocked, TargetDesignation, ShipSecrets/ShipSecretsOwner (per-component visibility), ShipPhysicsPlugin (server) / ShipVisualsPlugin (client), spawn_server_ship
- `src/weapon/` — Weapon system:
  - `mod.rs` — MountSize, WeaponType (HeavyCannon/Cannon/Railgun/HeavyVLS/LightVLS/LaserPD/CWIS), WeaponCategory (Cannon/Missile/PointDefense), FiringArc, WeaponProfile, WeaponState (incl. tubes_loaded, tube_reload_timer for VLS), Mount, Mounts component, MissileQueue/MissileQueueEntry
  - `projectile.rs` — Projectile/ProjectileVelocity/ProjectileDamage/ProjectileOwner/CwisRound components, spawn_projectile, ProjectilePlugin (advance, bounds, hit detection, CWIS hit detection)
  - `firing.rs` — compute_lead_position, is_in_firing_arc, tick_weapon_cooldowns, auto_fire system
  - `missile.rs` — Missile/MissileTarget/MissileVelocity/MissileDamage/MissileOwner components, compute_intercept_point, is_in_seeker_cone, spawn_missile, MissilePlugin (flat flight, seeker cone acquisition, asteroid collision, ship collision, bounds cleanup). Simplified: no altitude phases, flat flight with seeker cone, destroyed by asteroids.
  - `pd.rs` — Point defense systems: is_in_pd_cylinder (vertical cylinder check), probability-based kills (no missile HP), LaserBeam/LaserBeamTarget/LaserBeamTimer entities (visible beam tracking missile in real-time, delayed kill 0.15s after beam appears), CWIS visual tracers. LaserPD range 300m, CWIS 100m kill / 150m visual. 0.2s retarget delay. PdPlugin
  - `damage.rs` — DamagePlugin: mark_destroyed (with 1s delay timer), despawn_destroyed (cleanup ShipSecrets), check_win_condition (broadcast GameResult)
- `src/camera/` — CameraLookAt resource, strategic zoom (cursor zoom-in, center zoom-out), WASD pan (S is stop-only, not camera pan), middle-mouse orbit
- `src/input/` — Ship selection (left-click, left-click ground = deselect), move commands (right-click in all modes), shift+click waypoint queue, alt+right-click facing lock, alt+click-ship unlock, L+left-click facing lock mode, K+left-click target designation, K clear target, M toggle missile mode (M gated by VLS presence, right-click ground = missile at point, click enemy = missile at entity). Selecting new ship resets modes. All commands emit network triggers (MoveCommand, FacingLockCommand, FacingUnlockCommand, TargetCommand, ClearTargetCommand, MissileCommand).
- `src/fog/` — Server: LOS detection (distance+raycast) drives replicon visibility filtering. Client: FogClientPlugin with ghost entity fade-out on visibility loss.
- `src/net/` — Networking module:
  - `mod.rs` — LocalTeam resource, PROTOCOL_ID constant
  - `commands.rs` — MoveCommand, FacingLockCommand, FacingUnlockCommand, TargetCommand, ClearTargetCommand (client→server with MapEntities), TeamAssignment, GameResult (server→client)
  - `server.rs` — ServerNetPlugin: renet transport, connection/auth handling, team assignment, replication registration, fleet/asteroid spawning, command handlers with team validation (move, facing, target), LOS visibility filtering, ShipSecrets sync (waypoints, facing, targeting), target visibility clearing, disconnection handling
  - `client.rs` — ClientNetPlugin: renet transport, team assignment observer, ground plane setup, materializer/asteroid registration
  - `materializer.rs` — Spawns meshes for replicated Ship, Asteroid, Projectile, and Missile entities on client. Targeting indicator system. Selection indicator (green, larger, at ship center Y). F3 debug visuals toggle (seeker cone visualization). Explosion effects (two sizes: ship impact vs PD kill). LaserBeam visual tracking.

### System ordering (Update schedule)

**Server — Ship physics chain:** 1. Update facing targets → 2. Turn ships → 3. Apply thrust → 4. Apply velocity (with space drag) → 5. Check waypoint arrival → 6. Clamp to bounds

**Server — Weapons:** tick_weapon_cooldowns → auto_fire (spawn projectiles)

**Server — Missiles:** process_missile_queue (after auto_fire in weapon chain)

**Server — Missile flight:** advance_missiles → seeker_scan → check_missile_asteroid_hits → check_missile_hits → check_missile_bounds

**Server — Point Defense:** laser_pd_fire → update_laser_beams (track + delayed kill), cwis_fire

**Server — Projectiles:** advance_projectiles → check_projectile_bounds → check_projectile_hits → check_cwis_hits

**Server — Damage:** mark_destroyed → despawn_destroyed → check_win_condition

**Server — Networking:** sync_ship_secrets → server_update_visibility (LOS per-client) → clear_lost_targets

**Client — Visual indicators** (parallel): waypoint markers, facing direction arrows (read from ShipSecrets), targeting indicators

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
- **Uniform vision range**: 400m for all ship classes. Sensor/radar differentiation is Phase 4.
- **Weapon system**: Mounts are sized slots (Large/Medium/Small) per ship class. Weapons auto-fire at designated targets when in range+arc. Projectiles are independent server entities with velocity — no hitscan. Cooldown ticks every frame regardless of targeting. Lead calculation predicts target position. Railguns require forward-facing (±10°). Missile launchers (HeavyVLS/LightVLS) fire from MissileQueue. VLS uses tubes_loaded + tube_reload_timer on WeaponState (10s per-tube reload, queue capped at loaded tubes). Point defense (LaserPD/CWIS) auto-engages incoming missiles — probability-based kills, no missile HP.
- **Missile system**: M key toggles missile mode (gated by VLS presence). Right-click ground fires missiles at a point, click enemy fires at entity (with tracking). Simplified flat flight with seeker cone acquisition — no altitude/avoidance phases. Missiles destroyed by asteroid collision. MissileQueue lives on Ship entities and syncs to ShipSecrets.
- **Point defense**: LaserPD range 300m with visible beam (LaserBeam/LaserBeamTarget/LaserBeamTimer entities track missile in real-time, delayed kill 0.15s after beam appears). CWIS 100m kill radius / 150m visual tracer range. Probability-based kills. 0.2s retarget delay between engagements.
- **Ground plane**: Invisible (transparent), 3x map bounds for click targeting.
- **Explosions**: Two sizes — ship impact (large) vs PD kill (small).
- **Targeting**: K+left-click enemy designates target. K again clears. Target auto-clears when enemy leaves LOS. TargetDesignation synced via ShipSecrets (team-private).
- **Destruction**: Ships at 0 HP get Destroyed marker + 1s delay timer, then despawn (ship + ShipSecrets). Ghost fade-out fires on despawn. Win condition: all enemy ships destroyed → GameResult broadcast → GameOver state.

### Connection flow

**Server:** Setup → WaitingForPlayers (bind, listen) → Playing (when 2 clients authorized)
**Client:** Setup → Connecting (connect to server) → Playing (when TeamAssignment received)

Server spawns symmetric fleets (1 battleship, 1 destroyer, 1 scout per team, mirrored positions) and 12 random asteroids on entering Playing.

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

**Next up: Phase 3c — Fleet Composition Screen** (pre-game loadout with point budget)
**Phase 4: Sensors, EW & Win Conditions** (radar/passive/RWR, lock vs track, control points)
**Phase 5: Depth** (directional damage, repair, beams)

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

1Password GPG signing may fail in Claude sessions. Use `git -c commit.gpgsign=false commit` if needed.

## Reference projects

- Spaceflight (build config): `/Users/joshuajosai-levin/Code/spaceflight`
- Bevy 3D template (original): `/Users/joshuajosai-levin/Code/bevy_new_3d_rpg`
- Bevy 0.18 examples: `~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/bevy-0.18.1/examples/`
