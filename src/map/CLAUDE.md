# map/

Map setup, boundaries, and terrain.

## Files

- `mod.rs` — MapBounds resource (half_extents Vec2, contains/clamp/size helpers), Asteroid marker, AsteroidSize component (radius), GroundPlane marker (pickable, dark plane at Y=0), spawn systems for ground plane, 12 random asteroids (spheres, min distance from center/edge), and translucent boundary wall markers
