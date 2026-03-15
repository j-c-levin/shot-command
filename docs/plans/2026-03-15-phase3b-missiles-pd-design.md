# Phase 3b: Missiles & Point Defense — Design

Adds fire-and-forget missiles with ballistic arcs, tube-based VLS launchers with
player-controlled volley size, and two point defense systems (Laser PD and CIWS).
Missiles operate in 3D space — they arc over asteroids that block cannon fire.

---

## Missiles

### Fire-and-forget with terminal homing

Missiles are autonomous once launched. They fly a ballistic arc to a predicted
intercept point and activate a terminal seeker on arrival.

1. **At launch** — server computes a predicted intercept point. If the player
   clicked an enemy, lead calculation (same as cannons) predicts where the target
   will be. If the player clicked empty space, the intercept point is that position.
2. **In flight** — missile flies a ballistic arc toward the intercept point. It does
   NOT track the target during cruise. The launching ship's LOS is irrelevant — the
   missile is independent.
3. **Terminal phase** — when the missile reaches the dive zone near its intercept
   point, it checks a conical line of sight (~30° half-angle forward cone). If any
   valid target is within that cone, the missile adjusts course to home in. If no
   target is visible in the cone, the missile flies through empty space.
4. **Fuel limit** — missiles have a max range. When fuel is spent, the missile
   detonates and despawns. No infinite flight.

Terminal homing activates regardless of how the missile was targeted. A missile
fired at empty space will still home in on a target that happens to be in its
seeker cone during terminal phase.

### Ballistic arc flight profile

1. **Climb** — missile launches at ~45°, gaining altitude
2. **Cruise** — levels off at ~60m altitude, flies toward intercept point XZ
3. **Dive** — within dive distance of intercept point, pitches down toward target
4. **Terminal** — seeker cone activates, adjusts if target visible

Missiles fly in 3D space. They arc over asteroids that block flat cannon fire.
This is their primary tactical distinction from cannons.

### Missile speed and arrival time

Missiles are slower than cannon shells (80 m/s vs 120–150 m/s). Combined with the
arc path, arrival time is significantly longer than equivalent-range cannon fire.
This gives defenders time to react and PD time to engage.

### Missile HP

Missiles have health. PD systems deal damage to missiles. At 0 HP a missile is
destroyed mid-flight and despawned.

### Evasion through maneuver

Since missiles fly to a predicted intercept point and only course-correct during
terminal phase within a limited seeker cone, targets that maneuver after launch
can dodge. This rewards:
- Firing at committed/predictable targets
- Firing speculative salvos at suspected positions behind cover
- Bracketing a maneuvering target with multiple missiles aimed at different points

### Visibility

Enemy missiles follow the same LOS visibility rules as ships. A client only sees
enemy missiles that are within their team's line of sight. A salvo launched from
behind an asteroid field appears only when it enters LOS.

---

## VLS Launchers

### Tube model

Each VLS mount has N tubes and a total magazine of missiles. Tubes launch one
missile each. Once a tube fires, it reloads over a fixed duration before it can
fire again.

The player decides how many missiles to commit. The launcher determines how fast
they come out.

### Player interaction (M mode)

- Press **M** to enter missile fire mode (cursor indicates mode)
- **Left-click enemy** — queue one missile with lead-calculated intercept point
- **Left-click empty space** — queue one missile aimed at that XZ position
- Each click = one missile queued. Click rapidly for a big salvo. Can click
  different enemies to split the salvo.
- **M** again — exit mode. Queued missiles continue launching.
- **Esc** — exit mode AND cancel all queued-but-not-yet-loaded missiles.

### Launch behavior

- Available tubes fire immediately when missiles are queued
- If more missiles are queued than tubes available, the ship auto-launches as
  tubes finish reloading
- Exiting M mode does not stop in-progress launches

### Network command

`FireMissileCommand { ship: Entity, target_point: Vec2, target_entity: Option<Entity> }`

Client → server. Server validates ship ownership and VLS availability, queues
the missile for launch.

### Missile queue

`MissileQueue` component on ship entities — list of pending launches with target
point and optional target entity. Each tick, for each VLS mount with an available
tube, pop from queue and spawn a missile entity. Queue synced via ShipSecrets
(team-private).

---

## Point Defense

Two PD systems, both fully automatic. PD engages any missile within range — the
player does not control PD targeting.

### Engagement zone: vertical cylinder

PD range is a vertical cylinder, not a sphere. The radius is measured in XZ, with
unlimited Y extent. Any missile passing through the cylinder at any altitude
(including cruise altitude) is a valid target. This means PD protects an area,
and ship positioning relative to PD ships matters.

### Laser PD (medium mount)

- Fires once per second
- Instant hit — no projectile entity, ray test against target missile
- Deals 10 damage per shot
- 150m cylinder radius
- Always hits — effectiveness is about whether one shot kills
- Prioritizes closest missile

### CIWS (small mount)

- Fires every 0.1s (10 rounds/sec)
- Spawns small projectile entities aimed at the target missile
- 2 damage per hit
- 100m cylinder radius
- Has spread/inaccuracy — not every round connects
- Prioritizes closest missile
- CIWS rounds only damage missiles — they cannot damage ships. Despawn on
  contact with missile or at max range.

### PD target selection

Each PD mount independently targets the closest missile within its cylinder.
Multiple PD mounts on the same ship can engage different missiles simultaneously.

---

## Weapon Table

| Weapon | Type | Mount | Tubes | Reload | Speed | HP | Damage | Range |
|--------|------|-------|-------|--------|-------|----|--------|-------|
| Heavy Cannon | Cannon | Large | — | 3s | 150 | — | 15×3 | 300m |
| Cannon | Cannon | Medium | — | 1s | 120 | — | 8 | 200m |
| Railgun | Cannon | Large | — | 7s | 300 | — | 50 | 1000m |
| Heavy VLS | Missile | Large | 8 | 3s | 80 | 15 | 30 | 500m |
| Light VLS | Missile | Medium | 4 | 2s | 80 | 10 | 20 | 400m |
| Laser PD | PD | Medium | — | 1s | instant | — | 10 | 150m |
| CIWS | PD | Small | — | 0.1s | 200 | — | 2 | 100m |

---

## Default Loadouts

| Class | Loadout |
|-------|---------|
| Battleship | HeavyCannon (L), HeavyVLS (L), LightVLS (M), LaserPD (M), CIWS (S), CIWS (S) |
| Destroyer | Railgun (L), Cannon (M), LaserPD (M), CIWS (S) |
| Scout | Cannon (M), CIWS (S) |

---

## Missile Entity Components

- `Missile` — marker component
- `MissileTarget { intercept_point: Vec3, target_entity: Option<Entity> }` — fixed destination and optional homing target
- `MissileVelocity(Vec3)` — 3D velocity (unlike flat XZ projectiles)
- `MissileHealth(u16)` — takes damage from PD
- `MissileDamage(u16)` — dealt on hit to ship
- `MissileOwner(Entity)` — ship that fired
- `MissileFuel(f32)` — remaining range in meters, decremented by distance traveled per tick

All replicated. Visibility filtered by LOS like ships.

### Collision

- Missile hits ship if distance < ship's collision_radius (same as projectiles)
- Missile skips owner ship (no self-hits)
- Friendly fire IS possible

### Missile flight phases (server tick)

1. Advance position by velocity × dt
2. Update velocity direction based on current flight phase (climb/cruise/dive)
3. During terminal phase: check seeker cone, adjust aim if target visible
4. Decrement fuel by distance traveled
5. Check collision against ships
6. Check fuel exhaustion → detonate and despawn

---

## Out of Scope for 3b

- Missile waypoint programming (two-phase player-directed flight)
- PD enable/disable toggle
- CIWS damaging ships
- Ammo limits (currently disabled globally)
- Missile visual trails / explosion effects beyond despawn
- Torpedo subclass (slower, higher damage)
- Fire control quality / lock vs track (Phase 4)
