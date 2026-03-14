# fog/

Fog of war via line-of-sight on a grid.

## Files

- `mod.rs` — VisibilityGrid resource (100x100 cells, Hidden/Explored/Visible), FogOverlay entity with dynamic texture, LOS raycasting from player ships (blocked by asteroids), Revealed component sync on enemy entities, ship visibility toggling, fog texture update
