# Nebulous Shot Command — MVP Design

## Vision

Space tactical game inspired by Nebulous: Fleet Command. Players maneuver ships through fog of war to locate and engage enemies. 3D rendered on a 2D plane with free camera.

## MVP Scope

- Single player vs stationary AI target
- One ship per side, identical stats
- Small bounded map (~1000x1000 units) with asteroid LOS blockers
- Click-to-move ship controls
- Fog of war via line-of-sight with obstacle blocking
- Win condition: locate the enemy ship through fog

## Architecture

Flat plugin architecture, one plugin per concern:

- **CameraPlugin** — free camera (pan, zoom, rotate), looking down at play plane
- **ShipPlugin** — ship entity, stats, movement toward target, arrival
- **MapPlugin** — map boundaries, asteroid spawning/placement
- **FogPlugin** — LOS calculation, visibility grid, fog overlay rendering
- **InputPlugin** — ship selection (left-click), move commands (right-click), camera controls
- **GameStatePlugin** — game states (Setup → Playing → Victory), win condition check

## Data Model

### Components
- Ship (marker), Team (id), ShipStats (speed, vision_range)
- MovementTarget (destination Vec2) — added on move order, removed on arrival
- Selected (marker) — on player-selected ships
- Asteroid (marker), AsteroidSize (radius)
- Revealed (marker) — on entities currently visible to player

### Resources
- MapBounds (half_extents Vec2)
- VisibilityGrid (2D grid of cell states: Hidden / Visible / Explored)

### States
- GameState: Setup, Playing, Victory(winner)

## System Ordering

1. Input phase — camera controls, ship selection, move commands
2. Simulation phase — ship movement, bounds clamping
3. Fog phase — LOS raycasting, entity visibility sync
4. Render phase — fog overlay update, ship show/hide, selection indicator
5. Win condition — check if enemy is Revealed

## Fog of War

Grid-based shadow casting. From each player ship, cast rays across the grid up to vision_range. Rays stop at asteroid cells. Marked cells get Visible status. Enemy entities in Visible cells get the Revealed marker.

## Future Expansion (not in MVP)

- Combat systems (weapons, damage, shields)
- Multiple ship types with different loadouts
- Fleet composition (3-5 ships per side)
- AI behavior (patrol, search, chase)
- Networked multiplayer
- Control points and domination mode
- Radar/sensor mechanics beyond basic LOS
- Waypoint queues for movement
