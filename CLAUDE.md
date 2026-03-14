# Nebulous Shot Command — Claude Notes

## Project

Bevy 0.18 space tactical game. Player maneuvers ships through fog of war to locate enemies.
Physics-free MVP — ships move directly toward targets. Flat plugin architecture.

## Build & workflow

```bash
cargo run              # dev (dynamic linking, fast compile)
cargo check            # quick compilation check
cargo test             # unit tests only (pure function + World-level, no full App)
cargo build --release  # optimized release build
```

## Testing

### Philosophy

All tests are **pure-function or World-level only** — no full App, no render context, no asset
server. This keeps `cargo test` fast and avoids GPU/window dependencies.

- **Pure math** (movement, LOS, grid logic): plain `#[test]`, no imports beyond `bevy::prelude::*`
- **Resource/component presence**: `World::new()` + `world.insert_resource()` / `world.spawn()`
- **Avoid**: spinning up `App` with `DefaultPlugins` in tests

### Test locations

Tests live in `#[cfg(test)]` blocks at the bottom of each module file.

## Architecture

Flat plugin structure:
- `src/game/` — GameState enum, Team, win condition
- `src/map/` — MapBounds, asteroids, ground plane
- `src/ship/` — Ship entity, movement, selection components
- `src/camera/` — Free camera controller
- `src/input/` — Click selection, move commands
- `src/fog/` — Visibility grid, LOS raycasting, fog overlay

## Bevy 0.18 notes

- hotpatching and reflect_auto_register disabled (Cranelift incompatibility on macOS)
- Picking uses observer pattern: `.observe(|event: On<Pointer<Click>>| { ... })`
- Meshes: `Mesh3d(handle)`, Materials: `MeshMaterial3d(handle)`
- States: `#[derive(States)]` with `init_state::<T>()`

## Pre-approvals

The following tools and skills are pre-approved for autonomous use:
- All file read/write/edit operations
- All bash commands for building, testing, and running
- All glob and grep searches
- All LSP operations
- All MCP tools (context7, firebase, playwright)
- All skills (superpowers, bevy, domain-driven-design, etc.)
- All agent/subagent dispatching

## Known warnings

`WARN bevy_egui: Bindless textures not supported on Metal` — upstream wgpu limitation, safe to ignore.
