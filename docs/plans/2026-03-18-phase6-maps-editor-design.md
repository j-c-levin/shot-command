# Phase 6: Maps & Editor — Design

Dev-tool map editor launched via `--editor` flag on the client binary. Saves/loads
designed maps as RON files. Server loads maps via `--map` flag. No procedural
generation, no lobby map picker — just hand-crafted maps and the tooling to make them.

---

## Map File Format

RON files in `assets/maps/`. Plain data structs in `src/map/data.rs`, all
`Serialize + Deserialize`.

```ron
MapData(
    bounds: (half_x: 500.0, half_y: 500.0),
    spawns: [
        (team: 0, position: (-300.0, -300.0)),
        (team: 1, position: (300.0, 300.0)),
    ],
    asteroids: [
        (position: (100.0, -50.0), radius: 30.0),
        (position: (-200.0, 150.0), radius: 20.0),
    ],
    control_points: [
        (position: (0.0, 0.0), radius: 100.0),
    ],
)
```

### Structs

- `MapData` — top-level container: bounds, spawns, asteroids, control_points
- `SpawnPoint` — team (u8) + position (f32, f32)
- `AsteroidDef` — position (f32, f32) + radius (f32)
- `ControlPointDef` — position (f32, f32) + radius (f32)

---

## GameState & Entry

Add `Editor` variant to `GameState`:

```rust
pub enum GameState {
    #[default]
    Setup,
    WaitingForPlayers,
    Connecting,
    FleetComposition,
    Playing,
    GameOver,
    Editor,
}
```

### Client `--editor` flag

- Skip all networking setup
- Transition `Setup → Editor`
- Spawn camera, ground plane, boundary gizmos
- Optionally `--editor --map chokepoint.ron` to load existing map for editing
- Without `--map`, starts with empty map

### Server `--map` flag

- `cargo run --bin server -- --map chokepoint.ron`
- Loads RON from `assets/maps/`, spawns entities from it
- MapBounds set from file, spawn corners from file, exclusion zones auto-derived
- Without `--map`, falls back to current random generation (backwards compatible)

---

## Editor UI

### Left panel — Entity palette

Vertical list of placeable entity types:
- Asteroid
- Control Point
- Spawn Point

Click to select the active placement tool. Visual highlight on active tool. Same
visual style as fleet builder ship list panel. Hotkeys 1-4 as shortcuts (1=Select,
2=Asteroid, 3=Control Point, 4=Spawn Point).

### Top bar — File operations

- Save button + Load button
- Current file name displayed ("Untitled" for new maps)
- Save shows in-game popup with text input for file name, saves to `assets/maps/`
- Load shows popup listing available `.ron` files in `assets/maps/`

### Bottom-left — Tool indicator

Text showing current tool name, same pattern as input mode indicator in-game.

---

## Editor Interactions

### Resources

- `EditorState { tool: EditorTool, selected: Option<Entity> }` — current tool and selection
- `EditorTool` enum: `Select`, `PlaceAsteroid`, `PlaceControlPoint`, `PlaceSpawn`

### Select tool (default, hotkey 1)

- Left-click entity to select (highlight with gizmo ring)
- Left-click drag to move selected entity on ground plane
- Scroll wheel on selected asteroid to resize radius (clamped 10..80)
- `Delete` key removes selected entity

### Place tools (hotkeys 2-4)

- Left-click ground to place entity at cursor position
- Asteroids spawn with default radius 25
- Control points spawn with default radius 100
- Spawn points: max 2 (one per team). Third placement replaces oldest.

### Hotkeys

- `1` — Select tool
- `2` — Place asteroid
- `3` — Place control point
- `4` — Place spawn point
- `Ctrl+S` — Save
- `Ctrl+O` — Load
- `Delete` — Remove selected entity

### Camera

Reuses existing camera controls (WASD pan, scroll zoom, right-drag orbit). Left-drag
pan gated to non-Editor states since left-click is used for select/place in editor.

---

## Visual Feedback

- **Map bounds:** bright gizmo lines (white/cyan rectangle at Y=0), clearly visible
  for spatial reference while placing entities
- **Selected entity:** highlight ring gizmo
- **Spawn points:** colored markers (team 0 blue, team 1 red)
- **Asteroids:** rendered as spheres (reuse existing asteroid material)
- **Control points:** wireframe sphere gizmo (reuse existing control point visuals)
- **Tool indicator:** bottom-left text

---

## Server Map Loading

### `--map` provided

1. Read `assets/maps/<name>.ron`
2. Parse into `MapData`
3. Insert `MapBounds` from `map_data.bounds`
4. Spawn asteroids, control points from definitions
5. Use spawn positions for fleet placement + auto-derive exclusion zones

### No `--map`

Fall back to current behavior: 1000x1000, 12 random asteroids, 1 control point at
center, hardcoded corners. Zero breaking changes.

### Shared spawning

Extract entity spawning into `spawn_map_entities(commands, map_data)` used by both
server game setup and editor loading.

---

## Out of Scope

- Procedural map generation (separate future feature)
- Map selection in lobby UI (future — currently CLI `--map` only)
- Undo/redo (dev tool scope — reload file to reset)
- Copy/paste or multi-select
- Asteroid shape variety (all spheres, different radii)
- Map validation in editor (server handles missing data with defaults)
