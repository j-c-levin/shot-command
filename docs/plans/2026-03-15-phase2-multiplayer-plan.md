# Phase 2: Multiplayer Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Headless authoritative server + client binary, two players controlling separate teams over the network.

**Architecture:** Shared library crate with two thin binaries (`src/bin/server.rs`, `src/bin/client.rs`). Server runs simulation, clients render and send commands via `bevy_replicon` triggers. Server-side visibility filtering ensures clients only see what their team detects.

**Tech Stack:** `bevy_replicon` 0.38 for entity replication, `bevy_replicon_renet` for UDP transport, `serde` for serialization, `clap` for CLI args.

**Design doc:** `docs/plans/2026-03-15-phase2-multiplayer-design.md`

---

## Chunk 1: Foundation

### Task 1: Add dependencies and binary scaffolding

**Files:**
- Modify: `Cargo.toml`
- Create: `src/lib.rs`
- Create: `src/bin/server.rs`
- Create: `src/bin/client.rs`
- Modify: `src/main.rs`

- [ ] **Step 1: Add dependencies to Cargo.toml**

Add `bevy_replicon`, `bevy_replicon_renet`, `serde` (with `derive` feature), and `clap` (with `derive` feature) to `[dependencies]`. Add two `[[bin]]` sections: one named `server` pointing to `src/bin/server.rs`, one named `client` pointing to `src/bin/client.rs`.

- [ ] **Step 2: Create `src/lib.rs`**

Move all `mod` declarations (`camera`, `fog`, `game`, `input`, `map`, `ship`) from `src/main.rs` into `src/lib.rs` as `pub mod` declarations. This makes the game a library crate that both binaries can import.

- [ ] **Step 3: Update `src/main.rs` to use the library crate**

Change `src/main.rs` to import modules from `nebulous_shot_command::` instead of local `mod` declarations. Everything else stays the same — this preserves the existing single-player mode as a development fallback.

- [ ] **Step 4: Create stub server binary**

Create `src/bin/server.rs` that builds a Bevy app with `MinimalPlugins` and a `ScheduleRunnerPlugin` (for headless tick loop). Add `RepliconPlugins` and the renet server plugin. Parse `--bind` CLI arg with clap (default `127.0.0.1:5000`). The app should start and immediately exit cleanly for now.

- [ ] **Step 5: Create stub client binary**

Create `src/bin/client.rs` that builds a Bevy app with `DefaultPlugins` and `MeshPickingPlugin` (same as current `main.rs`). Add `RepliconPlugins` and the renet client plugin. Parse `--connect` CLI arg with clap (default `127.0.0.1:5000`). Should open a window with nothing in it.

- [ ] **Step 6: Verify all three targets compile**

Run: `cargo check --bin server && cargo check --bin client && cargo check`
Expected: all three pass.

- [ ] **Step 7: Run tests**

Run: `cargo test`
Expected: all 47 existing tests pass (library crate tests are unaffected).

- [ ] **Step 8: Commit**

Commit message: "feat: add library crate and stub server/client binaries"

---

### Task 2: Add serde derives to replicated components

**Files:**
- Modify: `src/game/mod.rs`
- Modify: `src/ship/mod.rs`

- [ ] **Step 1: Add `Serialize, Deserialize` derives to game components**

Add serde derives to: `GameState`, `Team`, `Health`. Add `use serde::{Serialize, Deserialize};` at the top. Skip `Detected` and `EnemyVisibility` — these are server-only and don't need serialization.

- [ ] **Step 2: Add `Serialize, Deserialize` derives to ship components**

Add serde derives to: `ShipClass`, `Velocity`, `WaypointQueue`, `FacingTarget`, `FacingLocked`. Bevy's `Vec2` implements serde traits when the `serialize` feature is enabled (already in Cargo.toml). `VecDeque` implements serde natively.

- [ ] **Step 3: Verify compilation and tests**

Run: `cargo check && cargo test`
Expected: all pass. Serde derives are purely additive.

- [ ] **Step 4: Commit**

Commit message: "feat: add serde derives to replicated components"

---

### Task 3: Split ShipPlugin into physics and visuals

**Files:**
- Modify: `src/ship/mod.rs`

- [ ] **Step 1: Create `ShipPhysicsPlugin` and `ShipVisualsPlugin`**

Split the current `ShipPlugin` into two plugins:
- `ShipPhysicsPlugin`: the chained Update systems only (update_facing_targets, turn_ships, apply_thrust, apply_velocity, check_waypoint_arrival, clamp_ships_to_bounds)
- `ShipVisualsPlugin`: init_indicator_assets, update_waypoint_markers, update_facing_indicators

Keep `ShipPlugin` as a convenience that adds both, so existing `main.rs` still works unchanged.

- [ ] **Step 2: Verify all targets compile and tests pass**

Run: `cargo check && cargo test`

- [ ] **Step 3: Commit**

Commit message: "refactor: split ShipPlugin into physics and visuals plugins"

---

### Task 4: Define command types and networking module

**Files:**
- Create: `src/net/mod.rs`
- Create: `src/net/commands.rs`
- Modify: `src/lib.rs`

- [ ] **Step 1: Create `src/net/mod.rs`**

Module root for networking code. Re-exports `commands` submodule. Define a `LocalTeam` resource that stores the team assigned to this client (wraps `Option<Team>` — `None` on server, `Some(team)` on client after assignment).

- [ ] **Step 2: Create `src/net/commands.rs`**

Define four event structs, all with `Serialize, Deserialize, Event` derives:
- `MoveCommand` — fields: `ship: Entity`, `destination: Vec2`, `append: bool`
- `FacingLockCommand` — fields: `ship: Entity`, `direction: Vec2`
- `FacingUnlockCommand` — fields: `ship: Entity`
- `TeamAssignment` — fields: `team: Team` (server → client trigger to tell client which team it controls)

The first three are client → server triggers. `TeamAssignment` is server → client.

- [ ] **Step 3: Add `pub mod net;` to `src/lib.rs`**

- [ ] **Step 4: Verify compilation**

Run: `cargo check && cargo test`
Expected: passes. No serialization round-trip tests — `bevy_replicon` handles entity mapping internally and `Entity` doesn't round-trip via standard serde. Trust the framework.

- [ ] **Step 5: Commit**

Commit message: "feat: define multiplayer command types and net module"

---

### Task 5: Extend GameState for multiplayer lifecycle

**Files:**
- Modify: `src/game/mod.rs`

- [ ] **Step 1: Add new GameState variants**

Add `WaitingForPlayers` and `Connecting` to the `GameState` enum. Keep `Setup` and `Playing`. Update `Default` to remain `Setup`.

- [ ] **Step 2: Update existing tests**

The `default_game_state_is_setup` test should still pass. Add a test that all new variants are distinct.

- [ ] **Step 3: Verify**

Run: `cargo test`

- [ ] **Step 4: Commit**

Commit message: "feat: add WaitingForPlayers and Connecting game states"

---

## Chunk 2: Server

### Task 6: Server networking, connection handling, and replication registration

**Files:**
- Create: `src/net/server.rs`
- Modify: `src/net/mod.rs`
- Modify: `src/bin/server.rs`

- [ ] **Step 1: Create `src/net/server.rs` with `ServerNetPlugin`**

Define a `ServerNetPlugin` that:
- Configures renet server transport to listen on a bind address (passed via a `ServerConfig` resource)
- Registers all replicated component types: `Ship`, `ShipClass`, `Team`, `Transform`, `Velocity`, `WaypointQueue`, `FacingTarget`, `FacingLocked`, `Health`
- Registers client→server triggers: `MoveCommand`, `FacingLockCommand`, `FacingUnlockCommand` (all `ChannelKind::Ordered`)
- Registers server→client trigger: `TeamAssignment`
- Adds a system that watches for new client connections
- On first connection: assigns `Team(0)`, sends `TeamAssignment` trigger to that client
- On second connection: assigns `Team(1)`, sends `TeamAssignment` trigger
- Stores client→team mapping in a `ClientTeams` resource (HashMap of client ID → Team)
- After 2 clients connected: transitions to `GameState::Playing`

- [ ] **Step 2: Register the plugin in `src/net/mod.rs`**

Add `pub mod server;` and re-export `ServerNetPlugin`.

- [ ] **Step 3: Wire up the server binary**

Update `src/bin/server.rs` to:
- Parse `--bind` with clap into a `ServerConfig` resource
- Add `ServerNetPlugin`
- Add `GamePlugin` (for GameState)
- Add `MapPlugin` (for map bounds resource — no rendering on server)
- Set initial state to `GameState::WaitingForPlayers`

- [ ] **Step 4: Test server starts and listens**

Run: `cargo run --bin server -- --bind 127.0.0.1:5000`
Expected: process starts, logs "Waiting for players", does not crash. Ctrl+C to stop.

- [ ] **Step 5: Commit**

Commit message: "feat: server binary with connection handling, team assignment, and replication registration"

---

### Task 7: Server-side fleet spawning

**Files:**
- Modify: `src/ship/mod.rs`
- Modify: `src/net/server.rs`

- [ ] **Step 1: Create a `spawn_server_ship` function in `src/ship/mod.rs`**

The existing `spawn_ship` depends on mesh and material assets which don't exist on the server. Create a `spawn_server_ship` function that spawns a ship entity with only data components: `Ship`, `ShipClass`, `Team`, `Velocity`, `WaypointQueue`, `Transform`, `Health`, and the `Replicated` marker from `bevy_replicon`. No mesh, no material, no visibility component.

- [ ] **Step 2: Add server game setup system in `src/net/server.rs`**

Add a system that runs on `OnEnter(GameState::Playing)`. It spawns:
- Map bounds resource (reuse existing `MapBounds`)
- Asteroids — same as current map setup, but with `Replicated` marker so clients can render them
- Symmetric fleets: for each team, 1 battleship, 1 destroyer, 1 scout
- Team 0 spawns near (-300, -300), Team 1 spawns mirrored near (300, 300)
- All ship entities use `spawn_server_ship` (which includes the `Replicated` marker)

- [ ] **Step 3: Add `ShipPhysicsPlugin` to server binary**

The server runs physics. Add `ShipPhysicsPlugin` (from Task 3's split) to the server app.

- [ ] **Step 4: Verify server spawns entities**

Run the server with two mock clients (or just check logs). Server should log fleet spawning and begin running physics.

- [ ] **Step 5: Commit**

Commit message: "feat: server spawns symmetric fleets and asteroids with replication"

---

## Chunk 3: Client

### Task 8: Client networking and connection

**Files:**
- Create: `src/net/client.rs`
- Modify: `src/net/mod.rs`
- Modify: `src/bin/client.rs`

- [ ] **Step 1: Create `src/net/client.rs` with `ClientNetPlugin`**

Define a `ClientNetPlugin` that:
- Configures renet client transport to connect to a server address (via `ClientConfig` resource)
- Registers replicated component types (must mirror server registration)
- Registers client→server triggers (same types as server)
- Adds observer for `TeamAssignment` server trigger — when received, stores the team in `LocalTeam` resource
- Transitions to `GameState::Playing` once team assignment is received

- [ ] **Step 2: Register the plugin in `src/net/mod.rs`**

Add `pub mod client;` and re-export `ClientNetPlugin`.

- [ ] **Step 3: Wire up the client binary**

Update `src/bin/client.rs` to:
- Parse `--connect` with clap into a `ClientConfig` resource
- Add `ClientNetPlugin`
- Add `GamePlugin`, `CameraPlugin`
- Add `ShipVisualsPlugin` (from Task 3's split — for waypoint markers and facing indicators)
- Set initial state to `GameState::Connecting`

- [ ] **Step 4: Test client connects to server**

Run server in one terminal, client in another:
- `cargo run --bin server`
- `cargo run --bin client`
Expected: client connects, server logs connection and team assignment. Client shows a window (empty scene — entities replicate but no meshes yet).

- [ ] **Step 5: Commit**

Commit message: "feat: client binary with server connection and team assignment"

---

### Task 9: Client entity materializer

**Files:**
- Create: `src/net/materializer.rs`
- Modify: `src/net/client.rs`

- [ ] **Step 1: Create entity materializer system**

Create a system that watches for newly replicated ship entities using an `Added<Ship>` query filter. When a `Ship` entity appears without a mesh child, spawn the appropriate mesh and material as a child based on `ShipClass` (cuboid for battleship, cone for destroyer, sphere for scout) and `Team` color (blue if matches `LocalTeam`, red otherwise).

- [ ] **Step 2: Add asteroid materializer**

Same pattern for asteroid entities — when an asteroid entity arrives via replication, spawn its mesh. Asteroids will need a marker component (e.g., `Asteroid`) registered for replication.

- [ ] **Step 3: Add ground plane and lighting on client**

The client needs its own ground plane (for click-to-move picking), ambient light, and camera. Add a client setup system that runs on `OnEnter(GameState::Playing)` to spawn these. The ground plane is client-local — it's not replicated.

- [ ] **Step 4: Test: server + client shows ships**

Run server, then client. Initially replicate all ships to all clients (no visibility filtering yet — that's Task 12). Client should show all 6 ships with correct meshes and team colors.

- [ ] **Step 5: Commit**

Commit message: "feat: client materializer spawns meshes for replicated entities"

---

## Chunk 4: Commands & Input

### Task 10: Rewrite input to emit network triggers

**Files:**
- Modify: `src/input/mod.rs`

- [ ] **Step 1: Refactor `on_ground_clicked` to emit commands**

Instead of directly inserting `WaypointQueue` or `FacingTarget`/`FacingLocked` components, the ground click handler fires network triggers:
- Right-click → `MoveCommand` with `append: false`
- Shift+right-click → `MoveCommand` with `append: true`
- Alt+right-click → `FacingLockCommand`
- Lock mode right-click → `FacingLockCommand`

The `Selected` component stays client-local.

- [ ] **Step 2: Refactor `on_ship_clicked` for unlock**

Alt+right-click on own ship fires `FacingUnlockCommand` trigger instead of directly removing `FacingLocked`.

Ship selection (left-click) stays entirely client-local.

- [ ] **Step 3: Guard input by team ownership**

Before sending any command, check that the selected ship's `Team` matches the `LocalTeam` resource. Reject client-side if mismatched.

- [ ] **Step 4: Test input**

Server + client. Select a ship, right-click to move. Verify server logs the command and the ship starts moving (position updates replicate back to client).

- [ ] **Step 5: Commit**

Commit message: "feat: input module emits network triggers instead of direct mutations"

---

### Task 11: Server-side command handlers

**Files:**
- Modify: `src/net/server.rs`

- [ ] **Step 1: Add `MoveCommand` handler**

Add an observer that receives `MoveCommand` triggers from clients. Look up `ClientTeams` to get sender's team. Verify the target ship entity exists and has matching `Team`. If valid and `append` is true, push destination to `WaypointQueue`; if false, clear and set single waypoint. If invalid, log warning and ignore.

- [ ] **Step 2: Add `FacingLockCommand` handler**

Validate ownership, set `FacingTarget` and insert `FacingLocked`.

- [ ] **Step 3: Add `FacingUnlockCommand` handler**

Validate ownership, remove `FacingLocked`.

- [ ] **Step 4: Test full command loop**

Server + client. Select ship, move it, lock facing, unlock facing. Verify all commands work through the network round-trip.

- [ ] **Step 5: Commit**

Commit message: "feat: server-side command handlers with team validation"

---

## Chunk 5: Visibility & Polish

### Task 12: Server-side visibility filtering

**Files:**
- Modify: `src/fog/mod.rs`
- Modify: `src/net/server.rs`

- [ ] **Step 1: Generalize LOS detection for both teams**

The existing `detect_enemies` system checks LOS from player ships to enemies. Generalize: for each team, compute which opposing-team ships are in LOS of any friendly ship. Produce a set of (observing_team, visible_entity) pairs. This runs on the server only.

- [ ] **Step 2: Wire LOS into replicon visibility**

Use `bevy_replicon`'s visibility API to control per-client entity replication. Each ship always replicates to its owning team's client. Enemy ships only replicate to a client when that team has LOS on them.

Check `bevy_replicon` docs for the exact API — likely involves `ConnectedClient` component and `ClientVisibility` methods.

- [ ] **Step 3: Handle per-component visibility limitation**

Check if `bevy_replicon` supports per-component visibility. If yes, restrict `Velocity`, `WaypointQueue`, `FacingTarget`, `FacingLocked` to owning team only. If no (entity-level only), accept the information leak for now. This is a known limitation — add a comment in the code noting it for future resolution.

- [ ] **Step 4: Test visibility filtering**

Server + two clients. Move Team 0's scout toward Team 1's ships. Verify Team 0's client sees enemies appear. Verify Team 1's client does NOT see Team 0's ships when out of range.

- [ ] **Step 5: Commit**

Commit message: "feat: server-side LOS visibility filtering for replication"

---

### Task 13: Client-side entity fade

**Files:**
- Modify: `src/fog/mod.rs`

- [ ] **Step 1: Create `FogClientPlugin`**

Split fog module into server-side (LOS detection, used by Task 12) and client-side. `FogClientPlugin` handles visual fade only:
- Fade-in: watch for entities with `Added<Ship>` where `Team != LocalTeam`. Start material alpha at 0, ramp to 1 over 0.5s.
- Fade-out: `bevy_replicon` despawns entities immediately when visibility is lost. Accept instant disappearance for now — smooth fade-out requires intercepting the despawn (e.g., with a `RemovedComponents` listener that delays despawn). Add a comment noting this as future polish.

- [ ] **Step 2: Add `FogClientPlugin` to client binary**

- [ ] **Step 3: Test fade-in**

Move a scout into detection range. Verify enemies fade in smoothly on the detecting client.

- [ ] **Step 4: Commit**

Commit message: "feat: client-side entity fade for appearing ships"

---

### Task 14: Disconnection handling

**Files:**
- Modify: `src/net/server.rs`

- [ ] **Step 1: Detect client disconnection**

Add a system that watches for renet client disconnection events. When a client disconnects, log which team lost connection. The disconnected team's ships remain in the world — physics keeps running, so they will drift and brake to a stop naturally (waypoint queue empties, braking kicks in).

- [ ] **Step 2: Handle graceful state**

If a client disconnects, the server continues running. The remaining client can keep playing. No reconnection logic — this is documented as out of scope.

- [ ] **Step 3: Test disconnection**

Start server + two clients. Close one client window. Verify server keeps running, remaining client keeps playing, disconnected team's ships drift to a stop.

- [ ] **Step 4: Commit**

Commit message: "feat: handle client disconnection gracefully"

---

### Task 15: Remove legacy single-player path

**Files:**
- Delete: `src/main.rs`
- Modify: `Cargo.toml`

- [ ] **Step 1: Remove `src/main.rs`**

The single-player `main.rs` is superseded by server+client binaries. Remove it. Remove any default binary target from `Cargo.toml`.

- [ ] **Step 2: Verify binaries and tests**

Run: `cargo check --bin server && cargo check --bin client && cargo test`
Expected: all pass. Library crate tests don't depend on main.rs.

- [ ] **Step 3: Update CLAUDE.md**

Update build/run commands: replace `cargo run` with `cargo run --bin server` and `cargo run --bin client`. Document the two-binary workflow.

- [ ] **Step 4: Commit**

Commit message: "chore: remove legacy single-player main.rs, update docs"

---

### Task 16: End-to-end integration test

No new files — manual verification.

- [ ] **Step 1: Start server**

Run: `cargo run --bin server -- --bind 127.0.0.1:5000`

- [ ] **Step 2: Start client 1**

Run: `cargo run --bin client -- --connect 127.0.0.1:5000`

- [ ] **Step 3: Start client 2**

Run: `cargo run --bin client -- --connect 127.0.0.1:5000`

- [ ] **Step 4: Verify gameplay**

Checklist:
- Both clients show their own team's 3 ships (battleship, destroyer, scout)
- Selecting a ship and right-clicking moves it (command goes through server)
- Shift+right-click appends waypoints
- Alt+right-click on ground locks facing direction
- Alt+right-click on own ship unlocks facing
- L key toggles lock mode / unlocks locked ships
- Moving a ship into enemy detection range causes enemy ships to fade in
- Moving away causes enemies to disappear
- Each client can only control their own team's ships
- Server stays running if a client disconnects
- Disconnected team's ships drift and brake to a stop

- [ ] **Step 5: Commit any fixes from integration testing**

Commit message: "fix: integration test fixes for multiplayer"
