# map/

Map boundaries, terrain, and editor.

## Files

- `mod.rs` — MapBounds resource (half_extents, contains/clamp/size), Asteroid/AsteroidSize components, GroundPlane marker (invisible, 3x bounds for click targeting)
- `data.rs` — MapData/BoundsDef/SpawnPoint/AsteroidDef/ControlPointDef structs (Serialize/Deserialize), RON save/load
- `editor.rs` — MapEditorPlugin (GameState::Editor): click-to-place/drag-to-move/scroll-resize/delete, entity palette UI, save/load popup, bounds gizmos

## Key behavior

- **Map files**: RON in `assets/maps/`. Server `--map name.ron` loads designed maps; without `--map`, random generation.
- **Editor**: dead-end state, no networking. EditorAsteroid/EditorControlPoint/EditorSpawn markers (distinct from game components). `editor_camera_zoom_or_resize` resizes asteroids or zooms camera depending on selection.
- **EditorMapData**: live MapData being edited; syncs on drag-release, delete, and placement.
