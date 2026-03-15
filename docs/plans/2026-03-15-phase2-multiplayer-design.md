# Phase 2: Multiplayer Foundation — Design

Headless authoritative server + dedicated client binaries. Two players connect
over the network, each controlling one team's fleet. Server runs all simulation;
clients render and send commands.

---

## Binary & Crate Structure

Shared library crate + two thin binaries. All existing modules stay in `src/`
as library code.

### `src/bin/server.rs`

- `MinimalPlugins` (no rendering, no window)
- `RepliconPlugins` + `RenetServerPlugin`
- Game logic plugins: `GamePlugin`, `MapPlugin`, `ShipPlugin` (physics only)
- `ServerNetPlugin`: accepts connections, assigns teams, spawns fleets,
  server-side visibility filtering
- CLI: `--bind 0.0.0.0:5000` (default `127.0.0.1:5000`)

### `src/bin/client.rs`

- `DefaultPlugins` + `MeshPickingPlugin`
- `RepliconPlugins` + `RenetClientPlugin`
- Rendering plugins: `CameraPlugin`, `InputPlugin` (rewritten to emit
  triggers), `FogPlugin` (fade only)
- `ClientNetPlugin`: connects to server, sends commands, receives replicated
  entities, spawns meshes for new entities
- CLI: `--connect 127.0.0.1:5000`

### Build commands

```bash
cargo run --bin client              # dev client
cargo run --bin server              # dev server
cargo build --release --bin server  # deployable server
```

---

## Component Replication

### Replicated to all clients

| Component | Why |
|-----------|-----|
| `Ship` | Marker — clients know what's a ship |
| `ShipClass` | Clients need this for mesh rendering |
| `Team` | Clients need to know ownership |
| `Transform` | Position and rotation |
| `Health` | UI (health bars) |

### Replicated to owning team only

| Component | Why |
|-----------|-----|
| `Velocity` | Enemy velocity would reveal movement intent |
| `WaypointQueue` | Enemy waypoints would leak tactical plans |
| `FacingTarget` | Enemy facing targets reveal intent |
| `FacingLocked` | Ditto |

### Server-only (never replicated)

| Component | Why |
|-----------|-----|
| `Detected` | Server uses for visibility filtering decisions |
| `EnemyVisibility` | Replaced by replication visibility |

### Client-only (never on server)

| Component | Why |
|-----------|-----|
| `Selected` | Local UI state |
| `SelectionIndicator` | Local UI state |
| `WaypointMarker` | Local rendering |
| `FacingIndicator` | Local rendering |

---

## Command Channel (Client → Server)

Clients send commands as `bevy_replicon` triggers. Server validates ownership
and executes.

### Commands

| Command | Data | Server action |
|---------|------|---------------|
| `MoveCommand` | `ship: Entity, destination: Vec2, append: bool` | Validate team ownership. Set or append `WaypointQueue`. |
| `FacingLockCommand` | `ship: Entity, direction: Vec2` | Validate ownership. Set `FacingTarget` + `FacingLocked`. |
| `FacingUnlockCommand` | `ship: Entity` | Validate ownership. Remove `FacingLocked`. |

### Validation

Every command handler checks `Team` on the target ship against the client's
assigned team. Reject silently if mismatched.

### Channel

`ChannelKind::Ordered` — commands for the same ship must arrive in order.

### Client input changes

Input handlers no longer mutate ECS directly. They fire network triggers
(`MoveCommand`, `FacingLockCommand`, `FacingUnlockCommand`). Server receives
and applies.

---

## System Ownership

### Server runs

- **Physics chain**: `update_facing_targets` → `turn_ships` → `apply_thrust`
  → `apply_velocity` → `check_waypoint_arrival` → `clamp_ships_to_bounds`
- **LOS detection**: distance + raycast → `Detected` marker → drives
  replication visibility filtering
- **Command handlers**: receives triggers, validates, mutates ECS
- **Game setup**: spawns map, asteroids, symmetric fleets
- **Connection handling**: assigns teams to connecting clients

### Client runs

- **Rendering**: meshes, materials (entity materializer spawns visuals for
  replicated entities)
- **Camera**: `CameraPlugin` unchanged
- **Input**: `InputPlugin` rewritten to emit network triggers
- **Visual indicators**: waypoint markers, facing arrows, selection torus
- **Entity fade**: simplified fog that fades entities in/out as server
  adds/removes them from view

---

## Connection Flow & Game Lifecycle

### GameState enum

```
Server: Setup → WaitingForPlayers → Playing
Client: Setup → Connecting → Playing
```

### Server startup

1. Bind to `--bind` address
2. Enter `WaitingForPlayers`
3. Wait for 2 clients
4. Assign Team 0 to first connection, Team 1 to second
5. Spawn map (ground plane, asteroids)
6. Spawn symmetric fleets: 1 battleship, 1 destroyer, 1 scout per team,
   mirrored positions
7. Mark all ships with `Replicated`
8. Transition to `Playing`

### Client startup

1. Connect to `--connect` address
2. Open window, set up rendering
3. Enter `Connecting`
4. Receive team assignment
5. Replicated entities arrive — materializer spawns meshes based on
   `ShipClass` + `Team`
6. Transition to `Playing`

### Disconnection

- Client disconnect: server keeps running, ships drift and brake
- No reconnection. Restart both if needed.

---

## Dependencies

```toml
[dependencies]
bevy_replicon = "0.38"
bevy_replicon_renet = "0.38"
serde = { version = "1", features = ["derive"] }
clap = { version = "4", features = ["derive"] }
```

Replicated components and triggers need `#[derive(Serialize, Deserialize)]`.

Binary definitions:

```toml
[[bin]]
name = "server"
path = "src/bin/server.rs"

[[bin]]
name = "client"
path = "src/bin/client.rs"
```

---

## Out of Scope

- No lobby / matchmaking — direct IP:port connection
- No reconnection — disconnect = restart
- No client-side prediction / interpolation — order-based gameplay masks
  latency naturally
- No chat, player names, or new UI
- No fleet customization — symmetric hardcoded fleets
- No encryption / auth
