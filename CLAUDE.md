# Nebulous Shot Command — Claude Notes

## Project

Bevy 0.18 space tactical game inspired by Nebulous: Fleet Command. Player maneuvers ships
through fog of war to locate enemies. Physics-free MVP — ships move directly toward targets.
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

- **Pure math** (movement, LOS, grid logic): plain `#[test]`, no imports beyond `bevy::prelude::*`
- **Resource/component presence**: `World::new()` + `world.insert_resource()` / `world.spawn()`
- **Avoid**: spinning up `App` with `DefaultPlugins` in tests

### Test locations

Tests live in `#[cfg(test)]` blocks at the bottom of each module file. Currently 25 tests:

| Module | What's tested |
|---|---|
| `src/game/mod.rs` | Team constants, GameState default |
| `src/map/mod.rs` | MapBounds contains/clamp/size |
| `src/ship/mod.rs` | XZ position extraction, movement direction, arrival detection |
| `src/fog/mod.rs` | Grid world↔grid roundtrip, visibility state transitions, ray-asteroid intersection |

## Architecture

Flat plugin structure, registered in `main.rs`:

- `src/game/` — GameState enum (Setup→Playing→Victory), Team component, Revealed marker, win condition
- `src/map/` — MapBounds resource, asteroids, ground plane (pickable for move commands)
- `src/ship/` — Ship entity, ShipStats, MovementTarget, movement system, bounds clamping
- `src/camera/` — Free camera (WASD pan, scroll zoom, middle-mouse orbit)
- `src/input/` — Ship selection (left-click observer), move commands (right-click ground), selection indicator torus
- `src/fog/` — VisibilityGrid (100x100), LOS raycasting against asteroids, dynamic fog texture, Revealed sync

### System ordering (Update schedule, Playing state)

1. Input → 2. Ship movement + bounds clamp → 3. Fog grid update → 4. Entity visibility sync → 5. Ship rendering → 6. Fog overlay texture → 7. Win condition check

### Key patterns

- **Picking observers** are attached per-entity (`.observe(on_ship_clicked)`), not global systems
- **`Pickable::IGNORE`** on fog overlay and selection indicator to prevent them intercepting clicks
- **`Revealed` component** is the decoupling point between fog logic, rendering, and win condition
- **Team component** uses `u8` id (not Player/Enemy markers) for multiplayer extensibility
- **Enemy ships start `Visibility::Hidden`**, toggled by fog system based on Revealed

### Game setup flow

`main.rs::setup_game` runs on `OnEnter(GameState::Setup)`:
1. Attaches `on_ground_clicked` observer to the ground plane (spawned in MapPlugin Startup)
2. Spawns player ship at (-350, -350) with `on_ship_clicked` observer
3. Spawns enemy ship at (350, 350) with `on_ship_clicked` observer, starts hidden
4. Transitions to `GameState::Playing`

## Bevy 0.18 notes

- `hotpatching` and `reflect_auto_register` features disabled (Cranelift incompatibility on macOS)
- Picking uses observer pattern: `.observe(|event: On<Pointer<Click>>| { ... })`
- Use `event.event_target()` not `event.target()` in picking observers
- Meshes: `Mesh3d(handle)`, Materials: `MeshMaterial3d(handle)`
- States: `#[derive(States)]` with `init_state::<T>()`
- Ambient light: `GlobalAmbientLight` as resource, NOT `AmbientLight` as entity
- `Image::new_fill` requires 5th arg: `RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD`
- `image.data` is `Option<Vec<u8>>` — must unwrap before indexing

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
