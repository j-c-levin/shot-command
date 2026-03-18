# Phase 6: Maps & Editor Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a map editor dev-tool (launched via `--editor` flag on client binary) that saves/loads designed maps as RON files, and update the server to load map files via `--map` flag.

**Architecture:** New `MapData` struct in `src/map/data.rs` defines the serializable map format. New `src/map/editor.rs` module provides `MapEditorPlugin` with all editor systems gated on `GameState::Editor`. Server's `server_setup_game` is refactored to optionally load from a `MapData` file instead of hardcoded random generation. Editor reuses existing camera controls and Bevy UI patterns from the fleet builder.

**Tech Stack:** Bevy 0.18, RON (serde), clap CLI

**Design doc:** `docs/plans/2026-03-18-phase6-maps-editor-design.md`

---

## File Structure

| File | Responsibility |
|------|---------------|
| `src/map/data.rs` (create) | `MapData`, `SpawnPoint`, `AsteroidDef`, `ControlPointDef` structs + load/save functions |
| `src/map/editor.rs` (create) | `MapEditorPlugin`, `EditorState`, `EditorTool`, all editor UI and interaction systems |
| `src/map/mod.rs` (modify) | Add `pub mod data; pub mod editor;` declarations |
| `src/game/mod.rs` (modify) | Add `Editor` variant to `GameState` enum |
| `src/bin/client.rs` (modify) | Add `--editor` and `--map` CLI flags, conditional plugin loading |
| `src/bin/server.rs` (modify) | Add `--map` CLI flag |
| `src/net/server.rs` (modify) | Refactor `server_setup_game` to use `MapData`, add `ServerMapPath` resource |
| `src/camera/mod.rs` (modify) | Gate left-drag pan to non-Editor states |
| `assets/maps/` (create dir) | Directory for saved map files |

---

## Task 1: Map Data Structs and Serialization

**Files:**
- Create: `src/map/data.rs`
- Modify: `src/map/mod.rs` (add `pub mod data;`)

- [ ] **Step 1: Add `ron` crate dependency**

Run: `cargo add ron`

- [ ] **Step 2: Write tests for MapData serialization roundtrip**

Add to `src/map/data.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn map_data_roundtrip_ron() {
        let map = MapData {
            bounds: BoundsDef {
                half_x: 500.0,
                half_y: 400.0,
            },
            spawns: vec![
                SpawnPoint {
                    team: 0,
                    position: (−300.0, −300.0),
                },
                SpawnPoint {
                    team: 1,
                    position: (300.0, 300.0),
                },
            ],
            asteroids: vec![AsteroidDef {
                position: (100.0, −50.0),
                radius: 30.0,
            }],
            control_points: vec![ControlPointDef {
                position: (0.0, 0.0),
                radius: 100.0,
            }],
        };

        let ron_str = ron::ser::to_string_pretty(&map, ron::ser::PrettyConfig::default()).unwrap();
        let parsed: MapData = ron::from_str(&ron_str).unwrap();

        assert_eq!(parsed.bounds.half_x, 500.0);
        assert_eq!(parsed.bounds.half_y, 400.0);
        assert_eq!(parsed.spawns.len(), 2);
        assert_eq!(parsed.spawns[0].team, 0);
        assert_eq!(parsed.asteroids.len(), 1);
        assert_eq!(parsed.asteroids[0].radius, 30.0);
        assert_eq!(parsed.control_points.len(), 1);
        assert_eq!(parsed.control_points[0].radius, 100.0);
    }

    #[test]
    fn map_data_default_is_empty() {
        let map = MapData::default();
        assert!(map.spawns.is_empty());
        assert!(map.asteroids.is_empty());
        assert!(map.control_points.is_empty());
        assert_eq!(map.bounds.half_x, 500.0);
        assert_eq!(map.bounds.half_y, 500.0);
    }

    #[test]
    fn save_and_load_file() {
        let map = MapData {
            bounds: BoundsDef {
                half_x: 300.0,
                half_y: 300.0,
            },
            spawns: vec![],
            asteroids: vec![AsteroidDef {
                position: (10.0, 20.0),
                radius: 15.0,
            }],
            control_points: vec![],
        };

        let dir = std::env::temp_dir().join("nebulous_test_maps");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("test_map.ron");

        save_map_data(&map, &path).unwrap();
        let loaded = load_map_data(&path).unwrap();

        assert_eq!(loaded.bounds.half_x, 300.0);
        assert_eq!(loaded.asteroids.len(), 1);
        assert_eq!(loaded.asteroids[0].position, (10.0, 20.0));

        // Cleanup
        std::fs::remove_file(&path).ok();
        std::fs::remove_dir(&dir).ok();
    }
}
```

- [ ] **Step 3: Run tests to verify they fail**

Run: `cargo test --lib map::data`
Expected: FAIL — structs don't exist yet

- [ ] **Step 4: Implement MapData structs and save/load functions**

Create `src/map/data.rs`:

```rust
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BoundsDef {
    pub half_x: f32,
    pub half_y: f32,
}

impl Default for BoundsDef {
    fn default() -> Self {
        Self {
            half_x: 500.0,
            half_y: 500.0,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SpawnPoint {
    pub team: u8,
    pub position: (f32, f32),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AsteroidDef {
    pub position: (f32, f32),
    pub radius: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ControlPointDef {
    pub position: (f32, f32),
    pub radius: f32,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct MapData {
    pub bounds: BoundsDef,
    pub spawns: Vec<SpawnPoint>,
    pub asteroids: Vec<AsteroidDef>,
    pub control_points: Vec<ControlPointDef>,
}

pub fn save_map_data(map: &MapData, path: &Path) -> Result<(), String> {
    let ron_str = ron::ser::to_string_pretty(map, ron::ser::PrettyConfig::default())
        .map_err(|e| format!("Failed to serialize map: {e}"))?;
    std::fs::write(path, ron_str).map_err(|e| format!("Failed to write file: {e}"))?;
    Ok(())
}

pub fn load_map_data(path: &Path) -> Result<MapData, String> {
    let contents = std::fs::read_to_string(path).map_err(|e| format!("Failed to read file: {e}"))?;
    ron::from_str(&contents).map_err(|e| format!("Failed to parse map: {e}"))
}
```

Add to `src/map/mod.rs` at the top: `pub mod data;`

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test --lib map::data`
Expected: PASS — all 3 tests

- [ ] **Step 6: Commit**

```bash
git add src/map/data.rs src/map/mod.rs Cargo.toml Cargo.lock
git commit -m "feat: add MapData structs and RON serialization"
```

---

## Task 2: Add Editor GameState Variant

**Files:**
- Modify: `src/game/mod.rs`

- [ ] **Step 1: Add Editor variant to GameState**

In `src/game/mod.rs`, add `Editor` after `GameOver` in the `GameState` enum:

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

- [ ] **Step 2: Update GameState tests if any reference variant count**

Check existing tests in `src/game/mod.rs` for any that might break.

- [ ] **Step 3: Run full test suite**

Run: `cargo test`
Expected: PASS — new variant is unused but shouldn't break anything

- [ ] **Step 4: Commit**

```bash
git add src/game/mod.rs
git commit -m "feat: add Editor variant to GameState"
```

---

## Task 3: Server --map Flag and MapData Loading

**Files:**
- Modify: `src/bin/server.rs`
- Modify: `src/net/server.rs`

- [ ] **Step 1: Add `--map` CLI arg and ServerMapPath resource to server binary**

In `src/bin/server.rs`, add to `Cli`:

```rust
/// Path to a map file (RON) in assets/maps/. If omitted, uses random generation.
#[arg(long)]
map: Option<String>,
```

Add a resource type in `src/net/server.rs`:

```rust
/// Optional path to a map file for server to load.
#[derive(Resource, Debug, Clone)]
pub struct ServerMapPath(pub Option<String>);
```

In `main()` of `src/bin/server.rs`, insert the resource:

```rust
.insert_resource(ServerMapPath(cli.map))
```

- [ ] **Step 2: Add `build_default_map_data` function for random generation fallback**

In `src/net/server.rs`, add a pure function that builds a `MapData` using the current random generation logic:

```rust
use crate::map::data::{MapData, BoundsDef, SpawnPoint, AsteroidDef, ControlPointDef};

fn build_default_map_data() -> MapData {
    use rand::Rng;
    let mut rng = rand::rng();
    let half_ext = MAP_HALF_EXTENT;
    let min_distance_from_edge = 50.0;
    let min_distance_from_center = 100.0;

    let mut asteroids = Vec::new();
    for _ in 0..12 {
        let radius = rng.random_range(15.0..40.0);
        let pos = loop {
            let candidate = bevy::math::Vec2::new(
                rng.random_range((-half_ext + min_distance_from_edge)..(half_ext - min_distance_from_edge)),
                rng.random_range((-half_ext + min_distance_from_edge)..(half_ext - min_distance_from_edge)),
            );
            if candidate.length() > min_distance_from_center
                && !is_in_asteroid_exclusion_zone(candidate)
            {
                break candidate;
            }
        };
        asteroids.push(AsteroidDef {
            position: (pos.x, pos.y),
            radius,
        });
    }

    MapData {
        bounds: BoundsDef {
            half_x: half_ext,
            half_y: half_ext,
        },
        spawns: vec![
            SpawnPoint { team: 0, position: (TEAM0_CORNER.x, TEAM0_CORNER.y) },
            SpawnPoint { team: 1, position: (TEAM1_CORNER.x, TEAM1_CORNER.y) },
        ],
        asteroids,
        control_points: vec![ControlPointDef {
            position: (0.0, 0.0),
            radius: crate::control_point::DEFAULT_ZONE_RADIUS,
        }],
    }
}
```

- [ ] **Step 3: Add `spawn_map_entities` function that spawns from MapData**

In `src/net/server.rs`, add a function that the server and editor both use:

```rust
/// Spawn asteroid and control point entities from a MapData definition.
/// Returns the spawn points for fleet placement.
pub fn spawn_map_entities(commands: &mut Commands, map_data: &MapData) -> Vec<SpawnPoint> {
    // Insert MapBounds
    commands.insert_resource(MapBounds {
        half_extents: bevy::math::Vec2::new(map_data.bounds.half_x, map_data.bounds.half_y),
    });

    // Spawn asteroids
    for def in &map_data.asteroids {
        commands.spawn((
            Asteroid,
            AsteroidSize { radius: def.radius },
            Transform::from_xyz(def.position.0, 0.0, def.position.1),
            Replicated,
        ));
    }

    // Spawn control points
    for def in &map_data.control_points {
        commands.spawn((
            ControlPoint,
            ControlPointState::Neutral,
            ControlPointRadius(def.radius),
            TeamScores::default(),
            Transform::from_xyz(def.position.0, 0.0, def.position.1),
            Replicated,
        ));
    }

    map_data.spawns.clone()
}
```

- [ ] **Step 4: Refactor `server_setup_game` to use MapData**

Replace the asteroid/control-point spawning section of `server_setup_game` with:

```rust
fn server_setup_game(
    mut commands: Commands,
    lobby: Res<crate::fleet::lobby::LobbyTracker>,
    client_teams: Res<ClientTeams>,
    map_path: Res<ServerMapPath>,
) {
    // Load or generate map data
    let map_data = if let Some(ref name) = map_path.0 {
        let path = std::path::PathBuf::from("assets/maps").join(name);
        match crate::map::data::load_map_data(&path) {
            Ok(data) => {
                info!("Server: loaded map from {}", path.display());
                data
            }
            Err(e) => {
                error!("Server: failed to load map '{}': {e}. Using default.", path.display());
                build_default_map_data()
            }
        }
    } else {
        build_default_map_data()
    };

    let spawns = spawn_map_entities(&mut commands, &map_data);

    // Determine spawn corners from map data (or fallback to defaults)
    let team0_corner = spawns.iter()
        .find(|s| s.team == 0)
        .map(|s| Vec2::new(s.position.0, s.position.1))
        .unwrap_or(TEAM0_CORNER);
    let team1_corner = spawns.iter()
        .find(|s| s.team == 1)
        .map(|s| Vec2::new(s.position.0, s.position.1))
        .unwrap_or(TEAM1_CORNER);

    let team_corners = [(Team(0), team0_corner), (Team(1), team1_corner)];

    // ... rest of fleet spawning remains unchanged, using team_corners ...
}
```

- [ ] **Step 5: Update `is_in_asteroid_exclusion_zone` to use dynamic spawn positions**

This function currently uses hardcoded TEAM0_CORNER/TEAM1_CORNER. Since `build_default_map_data` calls it during random generation (before spawn positions change), it stays the same for the fallback path. The loaded-map path doesn't need exclusion zones (asteroids are hand-placed). No change needed.

- [ ] **Step 6: Run full test suite**

Run: `cargo test`
Expected: PASS

- [ ] **Step 7: Commit**

```bash
git add src/bin/server.rs src/net/server.rs
git commit -m "feat: server --map flag loads MapData from RON files"
```

---

## Task 4: Client --editor Flag and Editor State Entry

**Files:**
- Modify: `src/bin/client.rs`
- Modify: `src/camera/mod.rs` (gate left-drag pan)

- [ ] **Step 1: Add `--editor` and `--map` CLI flags to client binary**

In `src/bin/client.rs`, add to `Cli`:

```rust
/// Launch the map editor instead of connecting to a server
#[arg(long)]
editor: bool,

/// Map file to load (for editor: load for editing; requires --editor)
#[arg(long)]
map: Option<String>,
```

- [ ] **Step 2: Add conditional startup logic**

In `main()` of `src/bin/client.rs`, branch on `cli.editor`:

```rust
if cli.editor {
    // Editor mode: skip networking, add editor plugin
    app.add_plugins(nebulous_shot_command::map::editor::MapEditorPlugin);
    if let Some(ref map_name) = cli.map {
        app.insert_resource(nebulous_shot_command::map::editor::EditorMapPath(map_name.clone()));
    }
    app.add_systems(Startup, set_editor_state);
} else {
    // Normal game mode: networking + all game plugins
    app.add_plugins((
        RepliconPlugins,
        RepliconRenetPlugins,
        SharedReplicationPlugin,
        FleetPlugin,
        InputPlugin,
        FleetUiPlugin,
        FleetStatusPlugin,
        RadarClientPlugin,
        FogClientPlugin,
        ShipVisualsPlugin,
    ))
    .add_plugins((
        ControlPointClientPlugin,
        ClientNetPlugin,
    ));
    app.insert_resource(ClientConnectAddress(cli.connect));
    app.init_resource::<LocalTeam>();

    if let Some(fleet_id) = cli.fleet {
        app.insert_resource(AutoFleet(preset_fleet(fleet_id)));
        app.add_systems(OnEnter(GameState::FleetComposition), auto_submit_fleet);
    }

    app.add_systems(Startup, set_connecting);
}
```

Add the editor startup system:

```rust
fn set_editor_state(mut next_state: ResMut<NextState<GameState>>) {
    next_state.set(GameState::Editor);
}
```

Note: `GamePlugin`, `CameraPlugin`, and `MeshPickingPlugin` are added in both paths (always needed).

**IMPORTANT:** `CameraPlugin` depends on `InputMode` resource (used by `camera_orbit`). In normal mode, `InputPlugin` initializes it. In editor mode, we must init it manually:

```rust
app.init_resource::<nebulous_shot_command::input::InputMode>();
```

- [ ] **Step 3: Gate left-drag camera pan to non-Editor states**

In `src/camera/mod.rs`, gate `camera_drag_pan` (left-click conflicts with editor selection) and `camera_zoom` (scroll conflicts with editor asteroid resize):

```rust
.add_systems(Update, (
    camera_pan,
    camera_zoom.run_if(not(in_state(GameState::Editor))),
    camera_orbit,
    camera_drag_pan.run_if(not(in_state(GameState::Editor))),
    deferred_center_camera,
));
```

The editor provides its own `editor_camera_zoom_or_resize` system that handles both zoom and asteroid resize.

- [ ] **Step 4: Verify client builds in editor mode**

Run: `cargo check --bin client`
Expected: PASS (editor module doesn't exist yet, but the import will fail — we'll create a stub)

- [ ] **Step 5: Create stub `src/map/editor.rs`**

```rust
use bevy::prelude::*;

/// Optional resource: path to load a map file in the editor.
#[derive(Resource, Debug, Clone)]
pub struct EditorMapPath(pub String);

pub struct MapEditorPlugin;

impl Plugin for MapEditorPlugin {
    fn build(&self, _app: &mut App) {
        // TODO: will be filled in subsequent tasks
    }
}
```

Add `pub mod editor;` to `src/map/mod.rs`.

- [ ] **Step 6: Run cargo check**

Run: `cargo check --bin client`
Expected: PASS

- [ ] **Step 7: Commit**

```bash
git add src/bin/client.rs src/camera/mod.rs src/map/editor.rs src/map/mod.rs
git commit -m "feat: client --editor flag with Editor GameState entry"
```

---

## Task 5: Editor Scene Setup (Ground Plane, Bounds Gizmos)

**Files:**
- Modify: `src/map/editor.rs`

- [ ] **Step 1: Implement editor scene spawn on OnEnter(Editor)**

In `src/map/editor.rs`, add scene setup:

```rust
use bevy::prelude::*;
use crate::game::GameState;
use crate::map::{GroundPlane, MapBounds};
use crate::map::data::{MapData, load_map_data};

#[derive(Resource, Debug, Clone)]
pub struct EditorMapPath(pub String);

/// The current file name (without path) for display. None = "Untitled".
#[derive(Resource, Debug, Default)]
pub struct EditorFileName(pub Option<String>);

/// The live map data being edited.
#[derive(Resource, Debug, Clone)]
pub struct EditorMapData(pub MapData);

impl Default for EditorMapData {
    fn default() -> Self {
        Self(MapData::default())
    }
}

pub struct MapEditorPlugin;

impl Plugin for MapEditorPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<EditorMapData>()
            .init_resource::<EditorFileName>()
            .add_systems(OnEnter(GameState::Editor), setup_editor_scene);
    }
}

fn setup_editor_scene(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    map_path: Option<Res<EditorMapPath>>,
    mut editor_data: ResMut<EditorMapData>,
    mut editor_file: ResMut<EditorFileName>,
) {
    // Load map if path provided
    if let Some(path_res) = map_path {
        let path = std::path::PathBuf::from("assets/maps").join(&path_res.0);
        match load_map_data(&path) {
            Ok(data) => {
                info!("Editor: loaded map from {}", path.display());
                editor_data.0 = data;
                editor_file.0 = Some(path_res.0.clone());
            }
            Err(e) => {
                error!("Editor: failed to load map: {e}. Starting empty.");
            }
        }
    }

    let bounds = &editor_data.0.bounds;

    // Insert MapBounds resource for camera/other systems
    commands.insert_resource(MapBounds {
        half_extents: Vec2::new(bounds.half_x, bounds.half_y),
    });

    // Spawn transparent ground plane for picking (same pattern as client_setup_scene)
    let plane_size = Vec2::new(bounds.half_x * 3.0, bounds.half_y * 3.0);
    commands.spawn((
        GroundPlane,
        Mesh3d(meshes.add(Plane3d::new(Vec3::Y, plane_size))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.02, 0.02, 0.05),
            perceptual_roughness: 1.0,
            ..default()
        })),
        Transform::from_xyz(0.0, 0.0, 0.0),
        Pickable::default(),
    ));
}
```

- [ ] **Step 2: Add bounds gizmo drawing system**

```rust
fn draw_editor_bounds_gizmos(
    mut gizmos: Gizmos,
    bounds: Res<MapBounds>,
) {
    let hx = bounds.half_extents.x;
    let hy = bounds.half_extents.y;
    let y = 0.5; // Slightly above ground

    let color = Color::srgba(0.3, 0.8, 1.0, 0.8);

    // Draw rectangle
    let corners = [
        Vec3::new(-hx, y, -hy),
        Vec3::new(hx, y, -hy),
        Vec3::new(hx, y, hy),
        Vec3::new(-hx, y, hy),
    ];

    for i in 0..4 {
        gizmos.line(corners[i], corners[(i + 1) % 4], color);
    }
}
```

Register in plugin:

```rust
.add_systems(Update, draw_editor_bounds_gizmos.run_if(in_state(GameState::Editor)))
```

- [ ] **Step 3: Run cargo check**

Run: `cargo check --bin client`
Expected: PASS

- [ ] **Step 4: Manual test — launch editor and see bounds**

Run: `cargo run --bin client -- --editor`
Expected: Window opens with dark ground plane and cyan boundary rectangle visible. Camera controls (WASD, scroll, right-drag orbit) work.

- [ ] **Step 5: Commit**

```bash
git add src/map/editor.rs
git commit -m "feat: editor scene setup with ground plane and bounds gizmos"
```

---

## Task 6: Editor State, Tools, and Entity Spawning

**Files:**
- Modify: `src/map/editor.rs`

- [ ] **Step 1: Add EditorState resource and EditorTool enum**

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EditorTool {
    #[default]
    Select,
    PlaceAsteroid,
    PlaceControlPoint,
    PlaceSpawn,
}

#[derive(Resource, Debug, Default)]
pub struct EditorState {
    pub tool: EditorTool,
    pub selected: Option<Entity>,
}
```

- [ ] **Step 2: Add marker components for editor-placed entities**

```rust
/// Marks an entity as an editor-placed asteroid (for query filtering).
#[derive(Component)]
pub struct EditorAsteroid;

/// Marks an entity as an editor-placed control point.
#[derive(Component)]
pub struct EditorControlPoint;

/// Marks an entity as an editor-placed spawn point. Stores the team.
#[derive(Component)]
pub struct EditorSpawn(pub u8);

/// Marks an entity as selected in the editor.
#[derive(Component)]
pub struct EditorSelected;
```

- [ ] **Step 3: Implement tool hotkey system**

```rust
fn handle_editor_hotkeys(
    keys: Res<ButtonInput<KeyCode>>,
    mut editor: ResMut<EditorState>,
) {
    if keys.just_pressed(KeyCode::Digit1) {
        editor.tool = EditorTool::Select;
    }
    if keys.just_pressed(KeyCode::Digit2) {
        editor.tool = EditorTool::PlaceAsteroid;
    }
    if keys.just_pressed(KeyCode::Digit3) {
        editor.tool = EditorTool::PlaceControlPoint;
    }
    if keys.just_pressed(KeyCode::Digit4) {
        editor.tool = EditorTool::PlaceSpawn;
    }
}
```

- [ ] **Step 4: Implement entity placement on ground click**

```rust
fn handle_editor_ground_click(
    click: On<Pointer<Click>>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    editor: Res<EditorState>,
    mut editor_data: ResMut<EditorMapData>,
    ground_query: Query<Entity, With<GroundPlane>>,
    spawn_query: Query<(Entity, &EditorSpawn)>,
) {
    if click.button != PointerButton::Primary {
        return;
    }

    let clicked_entity = click.event_target();
    if ground_query.get(clicked_entity).is_err() {
        return; // Not ground
    }

    let Some(hit_pos) = click.hit.position else {
        return;
    };
    let pos = Vec2::new(hit_pos.x, hit_pos.z);

    match editor.tool {
        EditorTool::PlaceAsteroid => {
            let radius = 25.0;
            editor_data.0.asteroids.push(crate::map::data::AsteroidDef {
                position: (pos.x, pos.y),
                radius,
            });
            spawn_editor_asteroid(&mut commands, &mut meshes, &mut materials, pos, radius);
        }
        EditorTool::PlaceControlPoint => {
            let radius = 100.0;
            editor_data.0.control_points.push(crate::map::data::ControlPointDef {
                position: (pos.x, pos.y),
                radius,
            });
            spawn_editor_control_point(&mut commands, &mut meshes, &mut materials, pos, radius);
        }
        EditorTool::PlaceSpawn => {
            // Determine team: count existing spawns
            let existing: Vec<(Entity, u8)> = spawn_query.iter()
                .map(|(e, s)| (e, s.0))
                .collect();
            let team = if existing.is_empty() {
                0
            } else if existing.len() == 1 {
                1 - existing[0].1 // Opposite team
            } else {
                // Replace oldest
                let oldest = existing[0].0;
                commands.entity(oldest).despawn();
                editor_data.0.spawns.retain(|s| s.team != existing[0].1);
                existing[0].1
            };
            editor_data.0.spawns.push(crate::map::data::SpawnPoint {
                team,
                position: (pos.x, pos.y),
            });
            spawn_editor_spawn_point(&mut commands, &mut meshes, &mut materials, pos, team);
        }
        EditorTool::Select => {} // Handled separately
    }
}
```

- [ ] **Step 5: Implement entity spawn helper functions**

```rust
fn spawn_editor_asteroid(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
    pos: Vec2,
    radius: f32,
) -> Entity {
    let material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.3, 0.25, 0.2),
        perceptual_roughness: 0.9,
        ..default()
    });
    commands
        .spawn((
            EditorAsteroid,
            Transform::from_xyz(pos.x, 0.0, pos.y),
            Pickable::default(),
        ))
        .with_child((
            Mesh3d(meshes.add(Sphere::new(radius))),
            MeshMaterial3d(material),
        ))
        .id()
}

fn spawn_editor_control_point(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
    pos: Vec2,
    radius: f32,
) -> Entity {
    let material = materials.add(StandardMaterial {
        base_color: Color::srgba(1.0, 1.0, 0.3, 0.3),
        alpha_mode: AlphaMode::Blend,
        ..default()
    });
    commands
        .spawn((
            EditorControlPoint,
            Transform::from_xyz(pos.x, 0.0, pos.y),
            Pickable::default(),
        ))
        .with_child((
            Mesh3d(meshes.add(Cylinder::new(radius, 1.0))),
            MeshMaterial3d(material),
        ))
        .id()
}

fn spawn_editor_spawn_point(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
    pos: Vec2,
    team: u8,
) -> Entity {
    let color = if team == 0 {
        Color::srgb(0.2, 0.4, 1.0)
    } else {
        Color::srgb(1.0, 0.3, 0.3)
    };
    let material = materials.add(StandardMaterial {
        base_color: color,
        emissive: bevy::color::LinearRgba::new(
            if team == 0 { 0.2 } else { 1.0 },
            if team == 0 { 0.4 } else { 0.3 },
            if team == 0 { 1.0 } else { 0.3 },
            1.0,
        ),
        ..default()
    });
    commands
        .spawn((
            EditorSpawn(team),
            Transform::from_xyz(pos.x, 0.0, pos.y),
            Pickable::default(),
        ))
        .with_child((
            Mesh3d(meshes.add(Cylinder::new(8.0, 2.0))),
            MeshMaterial3d(material),
        ))
        .id()
}
```

- [ ] **Step 6: Register systems and observer in plugin build**

```rust
impl Plugin for MapEditorPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<EditorMapData>()
            .init_resource::<EditorFileName>()
            .init_resource::<EditorState>()
            .add_systems(OnEnter(GameState::Editor), setup_editor_scene)
            .add_systems(
                Update,
                (
                    handle_editor_hotkeys,
                    draw_editor_bounds_gizmos,
                    draw_editor_entity_gizmos,
                )
                    .run_if(in_state(GameState::Editor)),
            );
    }
}
```

The ground click observer needs to be registered as a global observer in `setup_editor_scene`:

```rust
commands.add_observer(handle_editor_ground_click);
```

- [ ] **Step 7: Add gizmo drawing for control points and selection**

```rust
fn draw_editor_entity_gizmos(
    mut gizmos: Gizmos,
    cp_query: Query<(&Transform, &EditorControlPoint)>,
    selected: Query<&Transform, With<EditorSelected>>,
    // Removed unused: spawn_query
) {
    // Control point wireframe circles
    for (transform, _) in &cp_query {
        let pos = transform.translation;
        gizmos.circle(
            Isometry3d::new(pos + Vec3::Y * 0.5, Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
            100.0,
            Color::srgba(1.0, 1.0, 0.3, 0.6),
        );
    }

    // Selection highlight
    for transform in &selected {
        let pos = transform.translation;
        gizmos.circle(
            Isometry3d::new(pos + Vec3::Y * 0.5, Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
            15.0,
            Color::srgba(0.0, 1.0, 0.0, 0.8),
        );
    }
}
```

- [ ] **Step 8: Run cargo check**

Run: `cargo check --bin client`
Expected: PASS

- [ ] **Step 9: Manual test — place entities**

Run: `cargo run --bin client -- --editor`
Expected: Press 2, click ground → asteroid sphere appears. Press 3, click → control point wireframe circle. Press 4, click twice → two spawn markers (blue + red).

- [ ] **Step 10: Commit**

```bash
git add src/map/editor.rs
git commit -m "feat: editor entity placement (asteroid, control point, spawn)"
```

---

## Task 7: Editor Selection, Dragging, Resize, and Delete

**Files:**
- Modify: `src/map/editor.rs`

- [ ] **Step 1: Implement entity click observer for selection**

```rust
fn handle_editor_entity_click(
    click: On<Pointer<Click>>,
    mut commands: Commands,
    mut editor: ResMut<EditorState>,
    ground_query: Query<Entity, With<GroundPlane>>,
    asteroid_query: Query<Entity, With<EditorAsteroid>>,
    spawn_query: Query<Entity, With<EditorSpawn>>,
    selected_query: Query<Entity, With<EditorSelected>>,
) {
    if click.button != PointerButton::Primary {
        return;
    }
    if editor.tool != EditorTool::Select {
        return;
    }

    let clicked = click.event_target();

    // If clicked ground, deselect
    if ground_query.get(clicked).is_ok() {
        for e in &selected_query {
            commands.entity(e).remove::<EditorSelected>();
        }
        editor.selected = None;
        return;
    }

    // Check if clicked an editor entity (asteroid, control point, or spawn)
    let cp_query: Query<Entity, With<EditorControlPoint>> = todo!(); // included in fn params
    let entity = if asteroid_query.get(clicked).is_ok() {
        Some(clicked)
    } else if cp_query.get(clicked).is_ok() {
        Some(clicked)
    } else if spawn_query.get(clicked).is_ok() {
        Some(clicked)
    } else {
        None
    };

    if let Some(entity) = entity {
        // Deselect previous
        for e in &selected_query {
            commands.entity(e).remove::<EditorSelected>();
        }
        commands.entity(entity).insert(EditorSelected);
        editor.selected = Some(entity);
    }
}
```

Register as observer on each spawned entity (in spawn helpers, add `.observe(handle_editor_entity_click)`).

- [ ] **Step 2: Implement drag-to-move for selected entities**

Add a drag state resource and systems:

```rust
#[derive(Resource, Default)]
pub struct EditorDragState {
    pub dragging: bool,
    pub start_world: Vec2,
}

fn handle_editor_drag(
    mouse: Res<ButtonInput<MouseButton>>,
    camera_q: Query<(&Camera, &GlobalTransform), With<crate::camera::GameCamera>>,
    windows: Query<&Window>,
    mut editor: ResMut<EditorState>,
    mut drag: ResMut<EditorDragState>,
    mut transforms: Query<&mut Transform>,
    mut editor_data: ResMut<EditorMapData>,
    asteroid_query: Query<&EditorAsteroid>,
    cp_query: Query<&EditorControlPoint>,
    spawn_query: Query<&EditorSpawn>,
) {
    if editor.tool != EditorTool::Select {
        return;
    }
    let Some(selected) = editor.selected else {
        return;
    };

    let Ok((camera, cam_transform)) = camera_q.single() else {
        return;
    };
    let Ok(window) = windows.single() else {
        return;
    };

    // Raycast to ground plane (Y=0)
    let Some(cursor_pos) = window.cursor_position() else {
        return;
    };
    let Ok(ray) = camera.viewport_to_world(cam_transform, cursor_pos) else {
        return;
    };

    // Intersect with Y=0 plane
    if ray.direction.y.abs() < 0.001 {
        return;
    }
    let t = -ray.origin.y / ray.direction.y;
    if t < 0.0 {
        return;
    }
    let world_pos = ray.origin + ray.direction * t;
    let ground_pos = Vec2::new(world_pos.x, world_pos.z);

    if mouse.just_pressed(MouseButton::Left) && editor.tool == EditorTool::Select {
        drag.dragging = true;
        drag.start_world = ground_pos;
    }

    if drag.dragging && mouse.pressed(MouseButton::Left) {
        if let Ok(mut transform) = transforms.get_mut(selected) {
            transform.translation.x = ground_pos.x;
            transform.translation.z = ground_pos.y;

            // Update map data to match
            sync_entity_position_to_data(
                selected,
                ground_pos,
                &mut editor_data,
                &asteroid_query,
                &cp_query,
                &spawn_query,
            );
        }
    }

    if mouse.just_released(MouseButton::Left) {
        drag.dragging = false;
    }
}
```

The position sync uses the entity's current position to find its matching entry in `EditorMapData` (match by proximity, same pattern as delete). After moving, update the matching def's position:

```rust
// Find and update matching entry by old position (drag.start_world)
if asteroid_query.get(selected).is_ok() {
    if let Some(def) = editor_data.0.asteroids.iter_mut().find(|a| {
        (a.position.0 - drag.start_world.x).abs() < 0.1
            && (a.position.1 - drag.start_world.y).abs() < 0.1
    }) {
        def.position = (ground_pos.x, ground_pos.y);
    }
}
// Similar for control points and spawns
```

Update `drag.start_world` to the new position after each move frame to keep the match valid.

- [ ] **Step 3: Implement scroll-to-resize for asteroids**

**IMPORTANT:** Gate `camera_zoom` to not run in Editor state when an asteroid is selected. In `src/camera/mod.rs`, add a run condition or check `EditorState` resource. Simplest approach: gate `camera_zoom` with `.run_if(not(in_state(GameState::Editor)))` and add a separate `editor_camera_zoom` system that only zooms when no asteroid is selected.

Alternatively, in the editor, consume scroll events in the resize system and only forward to camera zoom when no asteroid is selected. The simplest approach: add an `EditorScrollConsumed` resource flag set by the resize system and checked by a custom editor zoom system.

**Recommended approach:** Gate the existing `camera_zoom` out of Editor state entirely. Add a dedicated `editor_camera_zoom` in the editor module that checks if an asteroid is selected — if so, resize; if not, zoom.

```rust
fn editor_camera_zoom_or_resize(
    mut scroll: EventReader<bevy::input::mouse::MouseWheel>,
    editor: Res<EditorState>,
    mut editor_data: ResMut<EditorMapData>,
    asteroid_query: Query<(&Transform, &Children), With<EditorAsteroid>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut mesh_query: Query<&mut Mesh3d>,
    // Camera zoom params:
    mut camera_query: Query<&mut Transform, (With<crate::camera::GameCamera>, Without<EditorAsteroid>)>,
    camera_settings: Res<crate::camera::CameraSettings>,
    mut look_at: ResMut<crate::camera::CameraLookAt>,
) {
    let mut scroll_delta = 0.0;
    for event in scroll.read() {
        scroll_delta += event.y;
    }
    if scroll_delta == 0.0 {
        return;
    }

    // If an asteroid is selected, resize it
    if let Some(selected) = editor.selected {
        if let Ok((transform, children)) = asteroid_query.get(selected) {
            let pos = (transform.translation.x, transform.translation.z);
            // Find matching asteroid in data and update radius
            if let Some(def) = editor_data.0.asteroids.iter_mut().find(|a| {
                (a.position.0 - pos.0).abs() < 0.1 && (a.position.1 - pos.1).abs() < 0.1
            }) {
                def.radius = (def.radius + scroll_delta * 2.0).clamp(10.0, 80.0);
                // Replace child mesh
                for &child in children.iter() {
                    if let Ok(mut mesh_handle) = mesh_query.get_mut(child) {
                        mesh_handle.0 = meshes.add(Sphere::new(def.radius));
                    }
                }
            }
            return; // Don't zoom camera
        }
    }

    // Otherwise, zoom camera (same logic as camera_zoom)
    // ... delegate to camera zoom logic
}
```

- [ ] **Step 4: Implement delete key**

```rust
fn handle_editor_delete(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    mut editor: ResMut<EditorState>,
    mut editor_data: ResMut<EditorMapData>,
    asteroid_query: Query<(Entity, &Transform), With<EditorAsteroid>>,
    cp_query: Query<(Entity, &Transform), With<EditorControlPoint>>,
    spawn_query: Query<(Entity, &EditorSpawn, &Transform)>,
) {
    if !keys.just_pressed(KeyCode::Delete) && !keys.just_pressed(KeyCode::Backspace) {
        return;
    }
    let Some(selected) = editor.selected else {
        return;
    };

    // Remove from map data and despawn entity
    if let Ok((_, transform)) = asteroid_query.get(selected) {
        let pos = (transform.translation.x, transform.translation.z);
        editor_data.0.asteroids.retain(|a| {
            (a.position.0 - pos.0).abs() > 0.1 || (a.position.1 - pos.1).abs() > 0.1
        });
    } else if let Ok((_, transform)) = cp_query.get(selected) {
        let pos = (transform.translation.x, transform.translation.z);
        editor_data.0.control_points.retain(|c| {
            (c.position.0 - pos.0).abs() > 0.1 || (c.position.1 - pos.1).abs() > 0.1
        });
    } else if let Ok((_, spawn, _)) = spawn_query.get(selected) {
        editor_data.0.spawns.retain(|s| s.team != spawn.0);
    }

    commands.entity(selected).despawn();
    editor.selected = None;
}
```

- [ ] **Step 5: Register all new systems**

Add to plugin build:

```rust
.init_resource::<EditorDragState>()
.add_systems(
    Update,
    (
        handle_editor_hotkeys,
        handle_editor_drag,
        handle_editor_scroll_resize,
        handle_editor_delete,
        draw_editor_bounds_gizmos,
        draw_editor_entity_gizmos,
    )
        .run_if(in_state(GameState::Editor)),
)
```

- [ ] **Step 6: Run cargo check**

Run: `cargo check --bin client`
Expected: PASS

- [ ] **Step 7: Manual test — select, drag, resize, delete**

Run: `cargo run --bin client -- --editor`
Expected: Press 2, place asteroids. Press 1, click asteroid → green highlight. Drag asteroid → moves. Scroll on asteroid → radius changes. Delete key → asteroid removed.

- [ ] **Step 8: Commit**

```bash
git add src/map/editor.rs
git commit -m "feat: editor selection, drag-to-move, scroll resize, delete"
```

---

## Task 8: Editor UI (Entity Palette + File Operations + Tool Indicator)

**Files:**
- Modify: `src/map/editor.rs`

- [ ] **Step 1: Add UI marker components**

```rust
#[derive(Component)]
struct EditorUiRoot;

#[derive(Component)]
struct ToolButton(EditorTool);

#[derive(Component)]
struct SaveButton;

#[derive(Component)]
struct LoadButton;

#[derive(Component)]
struct FileNameText;

#[derive(Component)]
struct EditorToolIndicator;

#[derive(Component)]
struct LoadPopupOverlay;

#[derive(Component)]
struct LoadFileOption(String);

#[derive(Component)]
struct SavePopupOverlay;

#[derive(Component)]
struct SaveNameInput;
```

- [ ] **Step 2: Spawn editor UI on OnEnter(Editor)**

Add a `spawn_editor_ui` system that creates:

**Left panel (entity palette):**
- Title "ENTITIES"
- Three buttons: "Asteroid" (EditorTool::PlaceAsteroid), "Control Point" (EditorTool::PlaceControlPoint), "Spawn Point" (EditorTool::PlaceSpawn)
- "Select" button (EditorTool::Select)
- Active tool highlighted

**Top bar:**
- File name text ("Untitled" or loaded file name)
- Save button
- Load button

**Bottom-left indicator:**
- Tool name text (same pattern as ModeIndicatorText in input module)

Use the same color constants and layout patterns as `fleet_builder.rs`.

```rust
fn spawn_editor_ui(mut commands: Commands) {
    commands
        .spawn((
            EditorUiRoot,
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                ..default()
            },
            Pickable::IGNORE,
            GlobalZIndex(5),
        ))
        .with_children(|root| {
            // Left panel
            root.spawn((
                Node {
                    width: Val::Px(200.0),
                    height: Val::Percent(100.0),
                    flex_direction: FlexDirection::Column,
                    padding: UiRect::all(Val::Px(10.0)),
                    row_gap: Val::Px(8.0),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.08, 0.08, 0.12, 0.9)),
            ))
            .with_children(|panel| {
                // Title
                panel.spawn((
                    Text::new("ENTITIES"),
                    TextFont { font_size: 20.0, ..default() },
                    TextColor(Color::WHITE),
                ));

                // Tool buttons
                for (label, tool) in [
                    ("Select (1)", EditorTool::Select),
                    ("Asteroid (2)", EditorTool::PlaceAsteroid),
                    ("Ctrl Point (3)", EditorTool::PlaceControlPoint),
                    ("Spawn (4)", EditorTool::PlaceSpawn),
                ] {
                    panel
                        .spawn((
                            ToolButton(tool),
                            Button,
                            Node {
                                width: Val::Percent(100.0),
                                padding: UiRect::all(Val::Px(10.0)),
                                justify_content: JustifyContent::Center,
                                ..default()
                            },
                            BackgroundColor(Color::srgb(0.2, 0.2, 0.35)),
                        ))
                        .with_child((
                            Text::new(label),
                            TextFont { font_size: 16.0, ..default() },
                            TextColor(Color::WHITE),
                        ));
                }

                // Spacer
                panel.spawn(Node { flex_grow: 1.0, ..default() });

                // File operations
                panel.spawn((
                    Text::new("FILE"),
                    TextFont { font_size: 20.0, ..default() },
                    TextColor(Color::WHITE),
                ));

                panel.spawn((
                    FileNameText,
                    Text::new("Untitled"),
                    TextFont { font_size: 14.0, ..default() },
                    TextColor(Color::srgb(0.6, 0.6, 0.6)),
                ));

                // Save button
                panel
                    .spawn((
                        SaveButton,
                        Button,
                        Node {
                            width: Val::Percent(100.0),
                            padding: UiRect::all(Val::Px(10.0)),
                            justify_content: JustifyContent::Center,
                            ..default()
                        },
                        BackgroundColor(Color::srgb(0.15, 0.55, 0.2)),
                    ))
                    .with_child((
                        Text::new("Save (Ctrl+S)"),
                        TextFont { font_size: 16.0, ..default() },
                        TextColor(Color::WHITE),
                    ));

                // Load button
                panel
                    .spawn((
                        LoadButton,
                        Button,
                        Node {
                            width: Val::Percent(100.0),
                            padding: UiRect::all(Val::Px(10.0)),
                            justify_content: JustifyContent::Center,
                            ..default()
                        },
                        BackgroundColor(Color::srgb(0.2, 0.2, 0.35)),
                    ))
                    .with_child((
                        Text::new("Load (Ctrl+O)"),
                        TextFont { font_size: 16.0, ..default() },
                        TextColor(Color::WHITE),
                    ));
            });

            // Bottom-left tool indicator
            root.spawn((
                EditorToolIndicator,
                Text::new("SELECT"),
                TextFont { font_size: 18.0, ..default() },
                TextColor(Color::srgba(0.3, 1.0, 0.3, 0.9)),
                Node {
                    position_type: PositionType::Absolute,
                    bottom: Val::Px(8.0),
                    left: Val::Px(220.0),
                    ..default()
                },
            ));
        });
}
```

- [ ] **Step 3: Add UI update systems**

```rust
fn update_tool_buttons(
    editor: Res<EditorState>,
    mut buttons: Query<(&ToolButton, &mut BackgroundColor)>,
) {
    if !editor.is_changed() {
        return;
    }
    for (tool_btn, mut bg) in &mut buttons {
        *bg = if tool_btn.0 == editor.tool {
            BackgroundColor(Color::srgb(0.25, 0.25, 0.45))
        } else {
            BackgroundColor(Color::srgb(0.2, 0.2, 0.35))
        };
    }
}

fn handle_tool_button_clicks(
    mut interaction_query: Query<(&Interaction, &ToolButton), Changed<Interaction>>,
    mut editor: ResMut<EditorState>,
) {
    for (interaction, tool_btn) in &mut interaction_query {
        if *interaction == Interaction::Pressed {
            editor.tool = tool_btn.0;
        }
    }
}

fn update_tool_indicator(
    editor: Res<EditorState>,
    mut query: Query<(&mut Text, &mut TextColor), With<EditorToolIndicator>>,
) {
    if !editor.is_changed() {
        return;
    }
    let Ok((mut text, mut color)) = query.single_mut() else {
        return;
    };

    let (label, c) = match editor.tool {
        EditorTool::Select => ("SELECT", Color::srgba(0.3, 1.0, 0.3, 0.9)),
        EditorTool::PlaceAsteroid => ("ASTEROID", Color::srgba(0.6, 0.5, 0.4, 0.9)),
        EditorTool::PlaceControlPoint => ("CONTROL POINT", Color::srgba(1.0, 1.0, 0.3, 0.9)),
        EditorTool::PlaceSpawn => ("SPAWN POINT", Color::srgba(0.5, 0.5, 1.0, 0.9)),
    };

    *text = Text::new(label);
    *color = TextColor(c);
}

fn update_file_name_text(
    file_name: Res<EditorFileName>,
    mut query: Query<&mut Text, With<FileNameText>>,
) {
    if !file_name.is_changed() {
        return;
    }
    let Ok(mut text) = query.single_mut() else {
        return;
    };
    let name = file_name.0.as_deref().unwrap_or("Untitled");
    *text = Text::new(name);
}
```

- [ ] **Step 4: Register UI systems**

Add `spawn_editor_ui` to `OnEnter(GameState::Editor)` alongside `setup_editor_scene`. Add update systems to the Update system set.

- [ ] **Step 5: Run cargo check**

Run: `cargo check --bin client`
Expected: PASS

- [ ] **Step 6: Manual test — UI panel and tool switching**

Run: `cargo run --bin client -- --editor`
Expected: Left panel with entity buttons and file operations. Clicking buttons switches tools. Tool indicator updates.

- [ ] **Step 7: Commit**

```bash
git add src/map/editor.rs
git commit -m "feat: editor UI panel with entity palette and tool indicator"
```

---

## Task 9: Save and Load Functionality

**Files:**
- Modify: `src/map/editor.rs`

- [ ] **Step 1: Implement save functionality**

```rust
fn handle_save(
    keys: Res<ButtonInput<KeyCode>>,
    mut interaction_query: Query<&Interaction, (Changed<Interaction>, With<SaveButton>)>,
    editor_data: Res<EditorMapData>,
    mut editor_file: ResMut<EditorFileName>,
) {
    let ctrl = keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::SuperLeft);
    let save_hotkey = ctrl && keys.just_pressed(KeyCode::KeyS);
    let button_clicked = interaction_query.iter().any(|i| *i == Interaction::Pressed);

    if !save_hotkey && !button_clicked {
        return;
    }

    // Ensure assets/maps/ directory exists
    std::fs::create_dir_all("assets/maps").ok();

    // Use existing file name or default
    let file_name = editor_file
        .0
        .clone()
        .unwrap_or_else(|| "untitled.ron".to_string());

    // Ensure .ron extension
    let file_name = if file_name.ends_with(".ron") {
        file_name
    } else {
        format!("{file_name}.ron")
    };

    let path = std::path::PathBuf::from("assets/maps").join(&file_name);
    match crate::map::data::save_map_data(&editor_data.0, &path) {
        Ok(()) => {
            info!("Editor: saved map to {}", path.display());
            editor_file.0 = Some(file_name);
        }
        Err(e) => {
            error!("Editor: failed to save: {e}");
        }
    }
}
```

- [ ] **Step 2: Implement load popup (list files in assets/maps/)**

```rust
fn handle_load_request(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    mut interaction_query: Query<&Interaction, (Changed<Interaction>, With<LoadButton>)>,
    existing_popup: Query<Entity, With<LoadPopupOverlay>>,
) {
    let ctrl = keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::SuperLeft);
    let load_hotkey = ctrl && keys.just_pressed(KeyCode::KeyO);
    let button_clicked = interaction_query.iter().any(|i| *i == Interaction::Pressed);

    if !load_hotkey && !button_clicked {
        return;
    }

    // Don't open if already open
    if !existing_popup.is_empty() {
        return;
    }

    // List .ron files in assets/maps/
    let mut files = Vec::new();
    if let Ok(entries) = std::fs::read_dir("assets/maps") {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("ron") {
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    files.push(name.to_string());
                }
            }
        }
    }
    files.sort();

    // Spawn popup overlay
    commands
        .spawn((
            LoadPopupOverlay,
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.7)),
            GlobalZIndex(10),
        ))
        .with_children(|overlay| {
            overlay
                .spawn((
                    Node {
                        width: Val::Px(400.0),
                        max_height: Val::Percent(80.0),
                        flex_direction: FlexDirection::Column,
                        padding: UiRect::all(Val::Px(20.0)),
                        row_gap: Val::Px(8.0),
                        overflow: Overflow::scroll_y(),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.15, 0.15, 0.22)),
                ))
                .with_children(|popup| {
                    popup.spawn((
                        Text::new("LOAD MAP"),
                        TextFont { font_size: 22.0, ..default() },
                        TextColor(Color::WHITE),
                    ));

                    if files.is_empty() {
                        popup.spawn((
                            Text::new("No maps found in assets/maps/"),
                            TextFont { font_size: 16.0, ..default() },
                            TextColor(Color::srgb(0.6, 0.6, 0.6)),
                        ));
                    }

                    for file_name in &files {
                        popup
                            .spawn((
                                LoadFileOption(file_name.clone()),
                                Button,
                                Node {
                                    width: Val::Percent(100.0),
                                    padding: UiRect::all(Val::Px(10.0)),
                                    ..default()
                                },
                                BackgroundColor(Color::srgb(0.2, 0.2, 0.35)),
                            ))
                            .with_child((
                                Text::new(file_name.as_str()),
                                TextFont { font_size: 16.0, ..default() },
                                TextColor(Color::WHITE),
                            ));
                    }

                    // Cancel button
                    popup
                        .spawn((
                            Button,
                            Node {
                                width: Val::Percent(100.0),
                                padding: UiRect::all(Val::Px(10.0)),
                                justify_content: JustifyContent::Center,
                                margin: UiRect::top(Val::Px(10.0)),
                                ..default()
                            },
                            BackgroundColor(Color::srgb(0.5, 0.2, 0.2)),
                        ))
                        .observe(|_click: On<Pointer<Click>>, mut commands: Commands, popup_q: Query<Entity, With<LoadPopupOverlay>>| {
                            for e in &popup_q {
                                commands.entity(e).despawn();
                            }
                        })
                        .with_child((
                            Text::new("Cancel"),
                            TextFont { font_size: 16.0, ..default() },
                            TextColor(Color::WHITE),
                        ));
                });
        });
}
```

- [ ] **Step 3: Handle load file selection**

```rust
fn handle_load_file_click(
    mut commands: Commands,
    mut interaction_query: Query<(&Interaction, &LoadFileOption), Changed<Interaction>>,
    popup_query: Query<Entity, With<LoadPopupOverlay>>,
    mut editor_data: ResMut<EditorMapData>,
    mut editor_file: ResMut<EditorFileName>,
    existing_entities: Query<Entity, Or<(With<EditorAsteroid>, With<EditorControlPoint>, With<EditorSpawn>)>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut bounds: ResMut<MapBounds>,
    mut editor: ResMut<EditorState>,
) {
    for (interaction, file_option) in &mut interaction_query {
        if *interaction != Interaction::Pressed {
            continue;
        }

        let path = std::path::PathBuf::from("assets/maps").join(&file_option.0);
        match crate::map::data::load_map_data(&path) {
            Ok(data) => {
                // Clear selection (prevent stale entity reference)
                editor.selected = None;

                // Despawn all existing editor entities
                for e in &existing_entities {
                    commands.entity(e).despawn();
                }

                // Update bounds
                bounds.half_extents = Vec2::new(data.bounds.half_x, data.bounds.half_y);

                // Spawn entities from loaded data
                for def in &data.asteroids {
                    let pos = Vec2::new(def.position.0, def.position.1);
                    spawn_editor_asteroid(&mut commands, &mut meshes, &mut materials, pos, def.radius);
                }
                for def in &data.control_points {
                    let pos = Vec2::new(def.position.0, def.position.1);
                    spawn_editor_control_point(&mut commands, pos, def.radius);
                }
                for def in &data.spawns {
                    let pos = Vec2::new(def.position.0, def.position.1);
                    spawn_editor_spawn_point(&mut commands, &mut meshes, &mut materials, pos, def.team);
                }

                editor_data.0 = data;
                editor_file.0 = Some(file_option.0.clone());
                info!("Editor: loaded {}", file_option.0);
            }
            Err(e) => {
                error!("Editor: failed to load {}: {e}", file_option.0);
            }
        }

        // Close popup
        for e in &popup_query {
            commands.entity(e).despawn();
        }
        break;
    }
}
```

- [ ] **Step 4: Handle Escape to close popup**

```rust
fn close_popup_on_escape(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    popup_query: Query<Entity, With<LoadPopupOverlay>>,
) {
    if keys.just_pressed(KeyCode::Escape) {
        for e in &popup_query {
            commands.entity(e).despawn();
        }
    }
}
```

- [ ] **Step 5: Register save/load systems**

Add to Update system set (gated on Editor state):

```rust
handle_save,
handle_load_request,
handle_load_file_click,
close_popup_on_escape,
update_file_name_text,
```

- [ ] **Step 6: Create assets/maps/ directory**

Run: `mkdir -p assets/maps`

- [ ] **Step 7: Run cargo check**

Run: `cargo check --bin client`
Expected: PASS

- [ ] **Step 8: Manual test — save and load**

Run: `cargo run --bin client -- --editor`
Expected: Place some entities. Ctrl+S → saves to `assets/maps/untitled.ron`. Click Load → popup shows "untitled.ron". Click it → entities reload. Verify RON file content is readable.

- [ ] **Step 9: Commit**

```bash
git add src/map/editor.rs assets/maps/
git commit -m "feat: editor save/load with file picker popup"
```

---

## Task 10: Load Existing Map Entities on Editor Startup

**Files:**
- Modify: `src/map/editor.rs`

- [ ] **Step 1: Spawn entities from loaded EditorMapData in setup_editor_scene**

At the end of `setup_editor_scene`, after loading the map data and spawning the ground plane, spawn visual entities from the loaded data:

```rust
// Spawn entities from map data
for def in &editor_data.0.asteroids {
    let pos = Vec2::new(def.position.0, def.position.1);
    spawn_editor_asteroid(&mut commands, &mut meshes, &mut materials, pos, def.radius);
}
for def in &editor_data.0.control_points {
    let pos = Vec2::new(def.position.0, def.position.1);
    spawn_editor_control_point(&mut commands, pos, def.radius);
}
for def in &editor_data.0.spawns {
    let pos = Vec2::new(def.position.0, def.position.1);
    spawn_editor_spawn_point(&mut commands, &mut meshes, &mut materials, pos, def.team);
}
```

- [ ] **Step 2: Manual test — load existing map on startup**

First, create a test map by running the editor and saving. Then:

Run: `cargo run --bin client -- --editor --map untitled.ron`
Expected: Editor opens with previously saved entities visible.

- [ ] **Step 3: Commit**

```bash
git add src/map/editor.rs
git commit -m "feat: spawn entities from map data on editor startup"
```

---

## Task 11: End-to-End Test — Create Map, Server Loads It

**Files:** No new files — integration test via manual steps.

- [ ] **Step 1: Create a test map in the editor**

Run: `cargo run --bin client -- --editor`
Place: 5 asteroids of varying sizes, 2 control points, 2 spawn points (one blue, one red). Save as `test_battle.ron`.

- [ ] **Step 2: Verify the RON file**

Run: `cat assets/maps/test_battle.ron`
Expected: Valid RON with correct positions and radii.

- [ ] **Step 3: Launch server with the map**

Run: `cargo run --bin server -- --map test_battle.ron`
Expected: Server logs "loaded map from assets/maps/test_battle.ron" and spawns the defined entities.

- [ ] **Step 4: Launch client and verify**

Run: `cargo run --bin client`
Expected: Asteroids appear at positions matching the editor placements. Control points at correct locations. Ships spawn near the defined spawn points.

- [ ] **Step 5: Run full test suite**

Run: `cargo test`
Expected: All tests pass (283+ existing + new MapData tests).

- [ ] **Step 6: Commit — update CLAUDE.md**

Update `CLAUDE.md` to document:
- `cargo run --bin client -- --editor` command
- `cargo run --bin client -- --editor --map name.ron` command
- `cargo run --bin server -- --map name.ron` command
- Phase 6 as COMPLETE in the roadmap section
- New module documentation for `src/map/data.rs` and `src/map/editor.rs`
- Updated test count

```bash
git add CLAUDE.md
git commit -m "docs: Phase 6 maps & editor complete"
```
