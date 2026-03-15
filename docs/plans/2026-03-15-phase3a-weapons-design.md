# Phase 3a: Mount Points & Cannons — Design

Adds mount points to ships, three cannon types, targeting system, projectile
simulation, damage, and ship destruction. This is the first sub-phase of
Phase 3 (Fleet & Loadouts). Ships can now shoot each other.

---

## Mount Point System

Each ship has a fixed set of mount slots defined by its class. Each mount has
a size (Large, Medium, Small) and a position offset relative to ship center
(for projectile spawn origin).

### Mounts per class

| Class | Large | Medium | Small | Total |
|-------|-------|--------|-------|-------|
| Battleship | 2 | 2 | 2 | 6 |
| Destroyer | 1 | 2 | 1 | 4 |
| Scout | 0 | 1 | 1 | 2 |

Small mounts are empty in 3a — reserved for point defense in 3b.

### Data model

A `Mounts` component on the ship entity holds a `Vec<Mount>` where each mount
has a `size: MountSize`, `offset: Vec2`, and `weapon: Option<WeaponState>`.
`WeaponState` tracks the weapon type, remaining ammo, and cooldown timer.

---

## Weapons

Three cannon types, differentiated by mount size, fire rate, and behavior.

### Cannon types

| Weapon | Mount Size | Fire Rate | Burst | Damage/Round | Firing Range | Arc | Ammo |
|--------|-----------|-----------|-------|-------------|-------------|-----|------|
| Heavy Cannon | Large | 1 salvo / 3s | 3 rounds | 15 | 300m | 360° turret | 60 |
| Cannon | Medium | 1 round / s | 1 round | 8 | 200m | 360° turret | 120 |
| Railgun | Large | 1 round / 7s | 1 round | 50 | 1000m | Forward ±10° | 10 |

Firing range is a gate — weapons won't fire beyond it. Projectiles fly until
they hit something or leave the map arena (map bounds despawn).

### Default loadouts (hardcoded, no selection UI)

| Class | Loadout |
|-------|---------|
| Battleship | 2× Heavy Cannon (large), rest empty |
| Destroyer | 1× Heavy Cannon (large), rest empty |
| Scout | 1× Cannon (medium), rest empty |

### Projectile speed

All projectile types share a base speed significantly faster than ships so
rounds feel like projectiles, not floating orbs. Exact value tuned in play.

---

## Projectiles

Each fired round is a server entity with position, velocity, damage, and owner
reference. Replicated to all clients so everyone sees rounds flying.

### Server tick behavior

1. Advance position by velocity × dt
2. Check collision against all ship collision radii (distance < collision_radius)
3. Skip the ship that fired the projectile (no self-hits)
4. Friendly fire IS possible (rounds hit whatever they intersect)
5. On hit: apply damage, despawn projectile
6. On leaving map bounds: despawn projectile

### Fire control (simplified, no lock/track)

Any visible enemy is a valid target. Server computes a lead vector — predicts
target position at time of arrival based on current velocity and distance.
Adds small random spread (±2° cannons, ±0.5° railgun). No fire control quality
system — that's Phase 4.

### Firing arc

Turret weapons (360°) fire at any target in range regardless of ship facing.
Railgun (Forward) fires only when angle between ship facing and target
direction is within ±10°. If target is outside the cone, weapon holds fire.

---

## Targeting

### Player interaction

1. Select ship (left-click, existing)
2. Press K to enter target selection mode
3. Left-click an enemy ship to designate target
4. Ship auto-fires all weapons that have the target in range and arc
5. Press K again on ship with existing target to clear it

### Input pattern consistency

Both L (facing lock) and K (target selection) now use hotkey + left-click:
- L + left-click ground → set facing direction, exit mode
- K + left-click enemy → designate target, exit mode
- K on ship with target → clear target (no click needed)
- Alt+right-click on ground still works as facing lock shortcut

### Target persistence

Target stays until: player clears it (K), target is destroyed, or target
leaves visibility. If target leaves LOS, targeting clears automatically.

### New commands

- `TargetCommand { ship: Entity, target: Entity }` — client → server
- `ClearTargetCommand { ship: Entity }` — client → server

### Replication

`TargetDesignation(Entity)` component lives on ShipSecrets (owning team only).
Enemy can't see who you're targeting. Client renders a targeting indicator on
the designated enemy (visible to your team).

---

## Damage & Destruction

### Health

`Health.hp` becomes `u16`. Ship HP by class:
- Battleship: 200
- Destroyer: 100
- Scout: 50

No armor, no directional damage — those are Phase 5.

### Collision detection

Each ship has `collision_radius` in ShipProfile (Battleship 12, Destroyer 8,
Scout 5). Projectile hits when distance from projectile to ship center is less
than collision_radius. Projectiles skip their owner ship.

### Ship destruction

When HP reaches 0:
1. Server inserts `Destroyed` marker on ship entity
2. Server despawns ship entity (and its ShipSecrets) after 1s delay
3. Client sees the `On<Remove, Ship>` observer fire — spawns a death visual
   (reuses ghost entity pattern with a brief flash effect)
4. In-flight projectiles from destroyed ship continue as independent entities

### Win condition

All enemy ships destroyed = game over. Server checks if no ships exist for
a team and transitions to a `GameOver` state. Client shows basic "You Win"
or "You Lose" text.

---

## Out of Scope for 3a

- Missiles (3b)
- Point defense (3b)
- Fleet composition / loadout screen (3c)
- Directional damage / armor (Phase 5)
- Repair (Phase 5)
- Fire control quality / lock vs track (Phase 4)
- Turret rotation animation (turrets snap to target)
- Ammo UI / HUD display
- Projectile visual trails
