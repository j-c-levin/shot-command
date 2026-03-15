# Nebulous Shot Command — Claude Notes

## Project

Bevy 0.18 space tactical game inspired by Nebulous: Fleet Command. Player maneuvers ships
to locate and destroy enemies. Physics-based movement with momentum, facing control, and
waypoint queuing. Three ship classes with distinct handling.
Flat plugin architecture, designed for future multiplayer expansion.

## Build & workflow

```bash
cargo run              # dev (dynamic linking, fast compile) — requires nightly toolchain
cargo check            # quick compilation check
cargo test             # unit tests only (pure function + World-level, no full App)
cargo build --release  # optimized release build
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

Tests live in `#[cfg(test)]` blocks at the bottom of each module file. Currently 47 tests:

| Module | What's tested |
|---|---|
| `src/game/mod.rs` | Team constants, GameState default, EnemyVisibility default, Health damage/saturation |
| `src/map/mod.rs` | MapBounds contains/clamp/size |
| `src/ship/mod.rs` | Thrust multiplier (facing/away/perpendicular), ship profiles ordering, velocity default, angle math (same/opposite/perpendicular), braking distance, shortest angle delta (positive/negative/wraparound), XZ extraction, facing direction, waypoint queue, steering controller (desired velocity braking/direction/at-target, perpendicular correction, overshoot braking) |
| `src/fog/mod.rs` | Ray-asteroid intersection, LOS range+occlusion, opacity fade in/out/clamp |

## Architecture

Flat plugin structure, registered in `main.rs`:

- `src/game/` — GameState enum (Setup→Playing), Team component, Detected marker, EnemyVisibility (opacity), Health
- `src/map/` — MapBounds resource, asteroids, ground plane (pickable for move commands)
- `src/ship/` — Ship entity, ShipClass enum (Battleship/Destroyer/Scout), ShipProfile (acceleration, thruster_factor, turn_rate, turn_acceleration, top_speed, vision_range, collision_radius), Velocity (linear Vec2 + angular f32), WaypointQueue (VecDeque + braking flag), FacingTarget/FacingLocked, physics systems (turn, thrust, velocity, waypoint arrival, bounds clamp), visual indicators (waypoint markers, facing arrow), spawn_ship with class-specific meshes (cuboid/cone/sphere)
- `src/camera/` — Free camera (WASD pan, scroll zoom, middle-mouse orbit)
- `src/input/` — Ship selection (left-click), move commands (right-click), shift+click waypoint queue, alt+right-click facing lock, alt+click-ship unlock, L key lock mode toggle, LockMode resource, selection indicator torus
- `src/fog/` — Distance+raycast LOS detection (reads vision_range from ShipClass profile), enemy fade in/out (0.5s) via material alpha

### System ordering (Update schedule)

**Ship physics chain:** 1. Update facing targets (auto-set toward waypoint if unlocked) → 2. Turn ships (angular velocity ramp) → 3. Apply thrust (asymmetric, cosine-interpolated) → 4. Apply velocity (position += vel * dt) → 5. Check waypoint arrival (pop + braking) → 6. Clamp to bounds

**Visual indicators** (parallel): waypoint markers, facing direction arrows

**Fog chain** (Playing state): 1. Detect enemies (distance + LOS) → 2. Fade enemies (opacity + material alpha)

### Key patterns

- **Physics model**: Velocity persists (momentum/drift). Steering controller computes desired velocity (direction to target × speed that allows stopping), then thrusts to correct the error between current and desired velocity. Thrust magnitude depends on angle between facing and thrust direction (cosine interpolation from 1.0 to thruster_factor). Angular velocity ramps up/down at turn_acceleration. Ships always brake to stop when queue is empty.
- **Facing lock/unlock**: Unlocked ships auto-face waypoint. Locked ships maintain player-set facing. Alt+right-click to lock, alt+click-ship or L to unlock.
- **Waypoint queue**: Right-click = clear + single waypoint. Shift+right-click = append. Auto-brake on final waypoint.
- **Picking observers**: ship clicks use per-entity `.observe()`, ground click uses global `add_observer()` with component filter
- **`Pickable::IGNORE`** on selection indicator to prevent it intercepting clicks
- **`Detected` marker** is the decoupling point between LOS detection and future combat
- **Team component** uses `u8` id (not Player/Enemy markers) for multiplayer extensibility

### Game setup flow

`main.rs::setup_game` runs on `OnEnter(GameState::Setup)`:
1. Registers `on_ground_clicked` as global observer
2. Spawns 3 player ships (battleship, destroyer, scout) near (-300, -300) with click observers
3. Spawns 5 enemy ships (mixed classes) scattered around map with EnemyVisibility + Health
4. Transitions to `GameState::Playing`

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

## Roadmap

See `docs/plans/2026-03-14-feature-brainstorm-v3.md` for full details.

**Phase 1: Core Simulation — COMPLETE.** Physics-based movement, facing control,
waypoint queuing, ship classes (battleship/destroyer/scout). See design doc at
`docs/plans/2026-03-14-phase1-core-simulation-design.md`.

**Next up: Phase 2 — Multiplayer** (bevy_replicon, client+host from day one)
**Phase 3: Fleet & Loadouts** (mount points, weapon variety, PD, fleet comp screen)
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
