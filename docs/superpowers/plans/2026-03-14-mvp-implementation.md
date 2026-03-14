# Nebulous Shot Command MVP Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Playable MVP where the player moves a ship through fog of war to locate a stationary enemy on a small space map with asteroid obstacles.

**Architecture:** Flat Bevy plugin architecture — one plugin per concern (Camera, Ship, Map, Fog, Input, GameState). All gameplay on a 2D plane (XZ) rendered in 3D with free camera.

**Tech Stack:** Bevy 0.18, Rust edition 2024, rand for asteroid placement

---

## File Structure

- `CLAUDE.md` — project notes, build commands, pre-approvals
- `Cargo.toml` — dependencies (bevy 0.18, rand)
- `.cargo/config.toml` — Mac build optimizations from spaceflight project
- `src/main.rs` — app setup, plugin registration
- `src/game/mod.rs` — GameState enum, Team, core types, win condition system
- `src/map/mod.rs` — MapBounds resource, asteroid spawning, ground plane
- `src/ship/mod.rs` — Ship components, ShipStats, MovementTarget, movement system
- `src/camera/mod.rs` — free camera controller (pan, zoom, rotate)
- `src/input/mod.rs` — ship selection (left-click), move commands (right-click via ground plane picking)
- `src/fog/mod.rs` — VisibilityGrid resource, LOS raycasting, fog overlay, Revealed marker

---

### Task 1: Project Setup

**Files:** CLAUDE.md, Cargo.toml, .cargo/config.toml

- [ ] Create CLAUDE.md with build commands, testing philosophy, pre-approvals for tools/skills
- [ ] Create Cargo.toml based on spaceflight config (bevy 0.18, rand, same profiles)
- [ ] Create .cargo/config.toml with Mac optimizations from spaceflight
- [ ] Run cargo check to verify setup compiles

### Task 2: Core Game Types (src/game/)

**Files:** src/game/mod.rs, src/main.rs (stub)

- [ ] Define GameState states enum (Setup, Playing, Victory)
- [ ] Define Team component, WinEvent
- [ ] Write unit tests for Team equality, GameState default
- [ ] Create minimal main.rs that registers GamePlugin
- [ ] cargo check

### Task 3: Map Plugin (src/map/)

**Files:** src/map/mod.rs

- [ ] Define MapBounds resource, Asteroid marker, AsteroidSize component
- [ ] Spawn ground plane (large dark plane on XZ at Y=0, pickable for move commands)
- [ ] Spawn boundary visual indicators
- [ ] Spawn 10-15 random asteroids (spheres) within bounds
- [ ] Write unit tests for bounds checking helper functions
- [ ] cargo check

### Task 4: Ship Plugin (src/ship/)

**Files:** src/ship/mod.rs

- [ ] Define Ship marker, ShipStats, MovementTarget, Selected components
- [ ] Spawn system: create ship mesh at given position with Team
- [ ] Movement system: move ship toward MovementTarget, remove component on arrival
- [ ] Bounds clamping system
- [ ] Write unit tests for movement math (direction, arrival detection)
- [ ] cargo check

### Task 5: Camera Plugin (src/camera/)

**Files:** src/camera/mod.rs

- [ ] Spawn camera looking down at map center
- [ ] Pan with WASD/arrow keys
- [ ] Zoom with scroll wheel
- [ ] Rotate with middle-mouse drag
- [ ] Clamp camera to reasonable bounds
- [ ] cargo check

### Task 6: Input Plugin (src/input/)

**Files:** src/input/mod.rs

- [ ] Ship selection via left-click on ship mesh (using Bevy picking observers)
- [ ] Move command via right-click on ground plane (get world position, set MovementTarget)
- [ ] Visual selection indicator (highlight ring or color change on Selected ships)
- [ ] cargo check

### Task 7: Fog of War Plugin (src/fog/)

**Files:** src/fog/mod.rs

- [ ] Define VisibilityGrid resource (2D array of cell states: Hidden/Visible/Explored)
- [ ] Define Revealed marker component
- [ ] Grid initialization system
- [ ] LOS update system: from each player ship, cast rays across grid, stop at asteroid cells
- [ ] Entity visibility sync: add/remove Revealed on enemies based on grid cell state
- [ ] Fog overlay: dynamic texture on a plane above the play field
- [ ] Show/hide enemy ship meshes based on Revealed
- [ ] Write unit tests for ray-grid intersection, cell visibility logic
- [ ] cargo check

### Task 8: Win Condition & Integration

**Files:** src/game/mod.rs (update), src/main.rs (finalize)

- [ ] Win condition system: if enemy ship has Revealed marker, trigger victory
- [ ] Victory UI: simple text overlay
- [ ] Wire all plugins into main.rs in correct order
- [ ] Spawn player ship and enemy ship from game setup
- [ ] cargo check, then cargo run to verify

### Task 9: Sub-directory CLAUDE.md Files

**Files:** src/game/CLAUDE.md, src/map/CLAUDE.md, src/ship/CLAUDE.md, src/camera/CLAUDE.md, src/input/CLAUDE.md, src/fog/CLAUDE.md

- [ ] Write accurate CLAUDE.md for each module describing its files and responsibilities
- [ ] Verify all documentation matches actual implementation
