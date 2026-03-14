# Phase 1: Core Simulation — Design

Physics-based movement replacing direct movement. Ships feel like warships,
not video game units. Every maneuver takes time and commitment.

---

## Scope

### In
- Physics-based movement (velocity, acceleration, momentum, drift)
- Asymmetric thrust (forward strongest, lateral weaker, rear weakest)
- Angular velocity for turning (ramp up/down, not instant)
- Facing control with lock/unlock mechanic
- Waypoint queuing (shift+click)
- Three ship classes (battleship, destroyer, scout) with distinct profiles and meshes
- Multiple enemy ships scattered around map for testing
- Auto-brake on final waypoint arrival

### Out (stripped)
- Combat module (auto-targeting, projectiles, hit detection, damage)
- Win condition system
- FireRate component

### Kept as-is
- Fog/detection system (distance + LOS, enemy fade in/out)
- Camera controls
- Map/asteroids
- Game state machine (Setup/Playing, no Victory)
- EnemyVisibility, Detected, Health, Team components

---

## Ship Data Model

### ShipClass enum

Three variants: `Battleship`, `Destroyer`, `Scout`. Each maps to a ShipProfile.

### ShipProfile

```
ShipProfile {
    acceleration: f32,        // units/s² (max, main engine)
    thruster_factor: f32,     // 0.0–1.0, multiplier when facing opposite movement
    turn_rate: f32,           // max radians/s (cap)
    turn_acceleration: f32,   // radians/s² (angular acceleration)
    top_speed: f32,           // units/s (max, main engine direction)
    vision_range: f32,        // detection range (used by fog system)
}
```

### Thrust model (asymmetric)

Effective thrust depends on angle between ship facing and desired movement direction:
- 0° (facing target): `acceleration * 1.0`, top speed = `top_speed * 1.0`
- 90° (lateral): `acceleration * lerp(1.0, thruster_factor, 0.5)`
- 180° (facing away): `acceleration * thruster_factor`

Interpolation via cosine: `multiplier = lerp(thruster_factor, 1.0, (1.0 + cos(angle)) / 2.0)`

This applies to both acceleration and deceleration. A ship facing away from its
velocity vector brakes poorly (thrusters only). A ship facing into its velocity
brakes at full main engine power.

### Velocity component

```
Velocity { linear: Vec2, angular: f32 }
```

- `linear`: current movement velocity in XZ plane
- `angular`: current turn speed in radians/s (signed)

### Turning model

Angular velocity ramps up at `turn_acceleration`, capped at `turn_rate`.
Decelerates to stop precisely at target angle. Ships start turning slowly
and build up — choices have weight.

A battleship ordered to 180° sluggishly begins rotating, builds angular
velocity, then must decelerate rotation to stop at the right angle.

### WaypointQueue

```
WaypointQueue { waypoints: VecDeque<Vec2> }
```

Replaces MovementTarget. Ship moves toward front of queue, pops on arrival.
Final waypoint triggers auto-brake (ship decelerates to arrive at ~zero velocity).

### Facing components

```
FacingTarget { direction: Vec2 }  // unit vector, desired facing
FacingLocked                       // marker component
```

- Unlocked (default): FacingTarget auto-set toward current waypoint
- Locked: FacingTarget stays as player set it

---

## Physics Update Loop

Runs each frame, in order:

### Step 1: Determine desired facing
- Unlocked + has waypoints → FacingTarget = direction toward next waypoint
- Locked → FacingTarget unchanged
- Unlocked + no waypoints → no FacingTarget, angular velocity decelerates to zero

### Step 2: Turn toward facing target
- Calculate angle delta between current facing and FacingTarget
- Accelerate angular velocity at turn_acceleration, capped at turn_rate
- Decelerate angular velocity approaching target angle (no overshoot)
- Apply angular velocity to rotation

### Step 3: Calculate thrust
- Has waypoints → desired movement toward next waypoint
- No waypoints, final waypoint reached → auto-brake (decelerate to zero)
- No waypoints, never had any → no thrust, drift on current velocity
- Angle between facing and desired movement → effective_acceleration, effective_top_speed

### Step 4: Apply acceleration
- Speed < effective top speed → accelerate toward waypoint
- Speed > effective top speed → decelerate toward effective top speed
- No waypoints + braking → decelerate to zero

### Step 5: Apply velocity
- position += linear_velocity * delta_time

### Step 6: Waypoint arrival
- Within arrival threshold of front waypoint → pop it
- If that was the last waypoint → begin auto-brake sequence

---

## Input & Facing Lock

### Move orders (right-click ground)
- Clears waypoint queue, sets single waypoint
- Unlocked: auto-sets FacingTarget toward destination
- Locked: facing unchanged

### Waypoint queuing (shift+right-click ground)
- Appends to queue instead of clearing

### Facing lock (alt+right-click ground)
- Sets FacingTarget to direction from ship toward clicked point
- Adds FacingLocked marker
- Visual indicator: arrow/line from ship showing locked direction

### Facing unlock
- Alt+right-click on the ship itself → removes FacingLocked
- `L` key if locked → unlocks
- `L` key if unlocked → enters "lock mode", next right-click sets facing + lock, exits mode

### Visual indicators
- Locked facing: persistent arrow/line showing locked direction
- Lock mode active: HUD text or indicator
- Waypoint queue: small markers at each waypoint, connected by a line

---

## Ship Classes

### Profiles (relative values, tuned during implementation)

|              | Battleship | Destroyer | Scout |
|--------------|------------|-----------|-------|
| Acceleration | Slow       | Medium    | Fast  |
| Turn rate    | Slow       | Medium    | Fast  |
| Top speed    | Slow       | Medium    | Fast  |

### Meshes
- Battleship: elongated box (strong, imposing)
- Destroyer: cone (aggressive, dangerous)
- Scout: ellipsoid (nimble, soft)

### Sizes
- Battleship: largest
- Destroyer: medium
- Scout: smallest

---

## Spawning & Setup

### Scene composition
- 1 player battleship, 1 player destroyer, 1 player scout near one corner
- 4-6 enemy ships (mix of classes) scattered around the map
- Enemies get EnemyVisibility + Health + fade behavior (fog system unchanged)
- Asteroids unchanged

### What gets deleted
- `src/combat/mod.rs` — entire module
- Combat plugin registration in main.rs
- Win condition system in game/
- FireRate component

### What gets rewritten
- `src/ship/mod.rs` — ShipClass, ShipProfile, Velocity, WaypointQueue, FacingTarget,
  FacingLocked, physics update systems, spawn_ship with class parameter
- `src/input/mod.rs` — facing lock/unlock, waypoint queuing, lock mode,
  visual indicators (waypoint markers, facing arrow)

### What stays untouched
- `src/camera/` — no changes
- `src/map/` — no changes
- `src/fog/` — no changes (reads vision_range from ShipProfile now)
