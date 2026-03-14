# fog/

Fog of war via line-of-sight on a grid.

## Files

- `mod.rs` — VisibilityGrid resource (100x100 cells with Hidden/Explored/Visible states), FogOverlay entity with dynamic RGBA texture (Pickable::IGNORE), LOS raycasting from player ships blocked by asteroids, Revealed component sync on enemy entities, ship visibility toggling based on Revealed, fog texture update each frame

## System chain (runs during Playing state)

1. `update_fog_grid` — clear Visible→Explored, raycast from player ships to mark cells Visible
2. `sync_entity_visibility` — add/remove Revealed on enemy ships based on grid cell state
3. `update_ship_rendering` — toggle Visibility::Visible/Hidden on enemies based on Revealed
4. `update_fog_overlay` — rewrite fog texture alpha per cell (Hidden=200, Explored=140, Visible=0)

## Key function

`ray_blocked_by_asteroid(start, end, asteroids)` — circle-line intersection test, returns true if any asteroid blocks the ray path
