# Nebulous Shot Command — Claude Notes

## Project

Bevy 0.18 space tactical game inspired by Nebulous: Fleet Command. Player maneuvers ships
to locate and destroy enemies. Ships auto-fire projectiles at detected enemies within LOS.
Physics-free MVP — ships move directly toward targets.
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

- **Pure math** (movement, LOS, combat, fade): plain `#[test]`, no imports beyond `bevy::prelude::*`
- **Resource/component presence**: `World::new()` + `world.insert_resource()` / `world.spawn()`
- **Avoid**: spinning up `App` with `DefaultPlugins` in tests

### Test locations

Tests live in `#[cfg(test)]` blocks at the bottom of each module file. Currently 34 tests:

| Module | What's tested |
|---|---|
| `src/game/mod.rs` | Team constants, GameState default, EnemyVisibility default, Health damage/saturation |
| `src/map/mod.rs` | MapBounds contains/clamp/size |
| `src/ship/mod.rs` | XZ position extraction, movement direction, arrival detection |
| `src/fog/mod.rs` | Ray-asteroid intersection, LOS range+occlusion, opacity fade in/out/clamp |
| `src/combat/mod.rs` | Projectile direction normalization, hit detection within/outside/at radius |

## Architecture

Flat plugin structure, registered in `main.rs`:

- `src/game/` — GameState enum (Setup→Playing→Victory), Team component, Detected marker, EnemyVisibility (opacity), Health, win condition (no enemies alive)
- `src/map/` — MapBounds resource, asteroids, ground plane (pickable for move commands)
- `src/ship/` — Ship entity, ShipStats, MovementTarget, movement system, bounds clamping, spawn_ship (enemies get EnemyVisibility + Health)
- `src/camera/` — Free camera (WASD pan, scroll zoom, middle-mouse orbit)
- `src/input/` — Ship selection (left-click observer), move commands (right-click ground), selection indicator torus
- `src/fog/` — Distance+raycast LOS detection, enemy fade in/out (0.5s) via material alpha
- `src/combat/` — Auto-targeting closest detected enemy, projectile spawning (0.5s fire rate), movement, hit detection, damage (3 hits to destroy)

### System ordering (Update schedule, Playing state)

1. Input → 2. Ship movement + bounds clamp → 3. Detection (distance+raycast) → 4. Fade (opacity+material alpha) → 5. Auto-target + fire projectiles → 6. Move projectiles → 7. Check projectile hits → 8. Win condition check

### Key patterns

- **Picking observers**: ship clicks use per-entity `.observe()`, ground click uses global `add_observer()` with component filter
- **`Pickable::IGNORE`** on selection indicator to prevent it intercepting clicks
- **`Detected` marker** is the decoupling point between LOS detection and combat targeting
- **`EnemyVisibility` component** drives fade opacity and material alpha for smooth reveal
- **Team component** uses `u8` id (not Player/Enemy markers) for multiplayer extensibility
- **Enemy ships start `Visibility::Hidden`** with `AlphaMode::Blend`, fade system manages visibility

### Game setup flow

`main.rs::setup_game` runs on `OnEnter(GameState::Setup)`:
1. Registers `on_ground_clicked` as global observer (ground plane doesn't exist yet — spawned in Startup)
2. Spawns player ship at (-350, -350) with `on_ship_clicked` observer + `FireRate`
3. Spawns enemy ship at (350, 350) with `on_ship_clicked` observer, starts hidden, gets `EnemyVisibility` + `Health{hp:3}`
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
