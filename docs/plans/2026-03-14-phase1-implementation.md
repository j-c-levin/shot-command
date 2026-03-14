# Phase 1: Core Simulation — Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace direct movement with physics-based simulation — velocity, acceleration, momentum, facing control, waypoint queuing, and three ship classes.

**Architecture:** Rewrite `ship/mod.rs` and `input/mod.rs` in place. Strip `combat/` entirely. Keep `fog/`, `camera/`, `map/`, `game/` (minus win condition). Pure math functions tested independently, systems tested at World level where needed.

**Tech Stack:** Rust, Bevy 0.18, nightly toolchain

**Design doc:** `docs/plans/2026-03-14-phase1-core-simulation-design.md`

---

## File Structure

### Modified files
- `src/game/mod.rs` — remove check_victory, spawn_victory_ui, Victory state
- `src/main.rs` — remove combat plugin, update setup_game to spawn multiple ships with classes
- `src/ship/mod.rs` — full rewrite: ShipClass, ShipProfile, Velocity, WaypointQueue, FacingTarget, FacingLocked, physics systems, new spawn_ship
- `src/input/mod.rs` — rewrite: facing lock/unlock, waypoint queuing, lock mode, visual indicators
- `src/fog/mod.rs` — minor: read vision_range from ShipProfile instead of ShipStats

### Deleted files
- `src/combat/mod.rs` — entire module removed

---

## Chunk 1: Strip Combat & Clean Game State

### Task 1: Remove combat module

**Files:**
- Delete: `src/combat/mod.rs`
- Modify: `src/main.rs`

- [ ] **Step 1: Delete combat module file**

Delete `src/combat/mod.rs` entirely.

- [ ] **Step 2: Remove combat from main.rs**

Remove `mod combat;` declaration and `combat::CombatPlugin` from plugin registration.
Remove `combat::FireRate::default()` from player ship setup in `setup_game`.

```rust
// In main.rs, remove these lines:
// mod combat;
// combat::CombatPlugin,
// .insert(combat::FireRate::default())
```

- [ ] **Step 3: Verify it compiles**

Run: `cargo check`
Expected: compiles with no errors (warnings OK)

- [ ] **Step 4: Commit**

```bash
git add -A && git commit -m "chore: strip combat module for Phase 1 rewrite"
```

### Task 2: Remove win condition from game module

**Files:**
- Modify: `src/game/mod.rs`

- [ ] **Step 1: Remove Victory state and related systems**

Remove `Victory` variant from `GameState` enum (keep Setup, Playing).
Remove `check_victory` system and `spawn_victory_ui` function.
Remove their registration from `GamePlugin::build`.
Keep: Team, Detected, EnemyVisibility, Health, GameState (Setup/Playing).

```rust
// GameState becomes:
#[derive(Clone, Copy, Default, Eq, PartialEq, Debug, Hash, States)]
pub enum GameState {
    #[default]
    Setup,
    Playing,
}

// GamePlugin::build becomes:
fn build(&self, app: &mut App) {
    app.init_state::<GameState>();
}
```

Remove the `check_victory` and `spawn_victory_ui` tests if any reference Victory.

- [ ] **Step 2: Verify it compiles**

Run: `cargo check`
Expected: compiles clean

- [ ] **Step 3: Run tests**

Run: `cargo test`
Expected: all remaining tests pass (combat tests gone, game tests for Team/EnemyVisibility/Health still pass)

- [ ] **Step 4: Commit**

```bash
git add src/game/mod.rs && git commit -m "chore: remove win condition for Phase 1"
```

---

## Chunk 2: Ship Data Model & Pure Physics Functions

### Task 3: Define ShipClass, ShipProfile, and new components

**Files:**
- Modify: `src/ship/mod.rs`

- [ ] **Step 1: Write tests for ShipProfile and thrust_multiplier**

Add these tests at the bottom of `src/ship/mod.rs` (replacing old tests):

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::PI;

    #[test]
    fn thrust_multiplier_facing_target_is_one() {
        // 0 degrees between facing and movement = full engine
        let m = thrust_multiplier(0.0, 0.2);
        assert!((m - 1.0).abs() < 0.001);
    }

    #[test]
    fn thrust_multiplier_facing_away_is_thruster_factor() {
        // PI radians (180°) = worst case, thrusters only
        let m = thrust_multiplier(PI, 0.2);
        assert!((m - 0.2).abs() < 0.001);
    }

    #[test]
    fn thrust_multiplier_perpendicular_is_midpoint() {
        // 90° = halfway between 1.0 and thruster_factor
        let m = thrust_multiplier(PI / 2.0, 0.2);
        let expected = 0.6; // lerp(0.2, 1.0, 0.5)
        assert!((m - expected).abs() < 0.01);
    }

    #[test]
    fn ship_profiles_are_ordered() {
        let b = ShipClass::Battleship.profile();
        let d = ShipClass::Destroyer.profile();
        let s = ShipClass::Scout.profile();
        assert!(b.acceleration < d.acceleration);
        assert!(d.acceleration < s.acceleration);
        assert!(b.top_speed < d.top_speed);
        assert!(d.top_speed < s.top_speed);
        assert!(b.turn_rate < d.turn_rate);
        assert!(d.turn_rate < s.turn_rate);
    }

    #[test]
    fn default_velocity_is_zero() {
        let v = Velocity::default();
        assert_eq!(v.linear, Vec2::ZERO);
        assert_eq!(v.angular, 0.0);
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test`
Expected: FAIL — `thrust_multiplier`, `ShipClass`, `Velocity` not defined

- [ ] **Step 3: Implement ShipClass, ShipProfile, Velocity, thrust_multiplier**

Replace the top of `src/ship/mod.rs` (above the systems) with:

```rust
use bevy::prelude::*;
use std::collections::VecDeque;

use crate::game::{EnemyVisibility, Health, Team};
use crate::map::{Asteroid, AsteroidSize, MapBounds};

pub struct ShipPlugin;

impl Plugin for ShipPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (
            update_facing_targets,
            turn_ships,
            apply_thrust,
            apply_velocity,
            check_waypoint_arrival,
            clamp_ships_to_bounds,
        ).chain());
    }
}

#[derive(Component)]
pub struct Ship;

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShipClass {
    Battleship,
    Destroyer,
    Scout,
}

#[derive(Clone, Debug)]
pub struct ShipProfile {
    pub acceleration: f32,
    pub thruster_factor: f32,
    pub turn_rate: f32,
    pub turn_acceleration: f32,
    pub top_speed: f32,
    pub vision_range: f32,
    pub collision_radius: f32,
}

impl ShipClass {
    pub fn profile(&self) -> ShipProfile {
        match self {
            ShipClass::Battleship => ShipProfile {
                acceleration: 15.0,
                thruster_factor: 0.2,
                turn_rate: 0.8,
                turn_acceleration: 0.4,
                top_speed: 40.0,
                vision_range: 250.0,
                collision_radius: 12.0,
            },
            ShipClass::Destroyer => ShipProfile {
                acceleration: 30.0,
                thruster_factor: 0.3,
                turn_rate: 1.5,
                turn_acceleration: 1.0,
                top_speed: 80.0,
                vision_range: 200.0,
                collision_radius: 8.0,
            },
            ShipClass::Scout => ShipProfile {
                acceleration: 50.0,
                thruster_factor: 0.5,
                turn_rate: 3.0,
                turn_acceleration: 2.0,
                top_speed: 130.0,
                vision_range: 150.0,
                collision_radius: 5.0,
            },
        }
    }
}

#[derive(Component, Clone, Debug, Default)]
pub struct Velocity {
    pub linear: Vec2,
    pub angular: f32,
}

#[derive(Component, Clone, Debug)]
pub struct WaypointQueue {
    pub waypoints: VecDeque<Vec2>,
    /// True after final waypoint is popped — ship should auto-brake
    pub braking: bool,
}

impl Default for WaypointQueue {
    fn default() -> Self {
        Self {
            waypoints: VecDeque::new(),
            braking: false,
        }
    }
}

#[derive(Component, Clone, Debug)]
pub struct FacingTarget {
    pub direction: Vec2,
}

/// Marker: ship facing is player-locked, not auto-set by waypoints
#[derive(Component, Clone, Debug)]
pub struct FacingLocked;

#[derive(Component)]
pub struct Selected;

#[derive(Component)]
pub struct SelectionIndicator;

/// Thrust multiplier based on angle between facing and movement direction.
/// 0 radians (facing target) → 1.0
/// PI radians (facing away) → thruster_factor
/// Smooth cosine interpolation between.
pub fn thrust_multiplier(angle: f32, thruster_factor: f32) -> f32 {
    let t = (1.0 + angle.cos()) / 2.0; // 1.0 at 0°, 0.0 at 180°
    thruster_factor + t * (1.0 - thruster_factor)
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test`
Expected: new tests pass. Old ship tests removed (they tested old functions). Compile errors expected from missing system functions — that's fine, tests are module-scoped.

Note: The system functions (`update_facing_targets`, `turn_ships`, etc.) are referenced in the plugin but not yet implemented. Add stub functions so it compiles:

```rust
fn update_facing_targets() {}
fn turn_ships() {}
fn apply_thrust() {}
fn apply_velocity() {}
fn check_waypoint_arrival() {}
// clamp_ships_to_bounds already exists
```

- [ ] **Step 5: Commit**

```bash
git add src/ship/mod.rs && git commit -m "feat: add ShipClass, ShipProfile, Velocity, thrust_multiplier"
```

### Task 4: Pure physics helper functions

**Files:**
- Modify: `src/ship/mod.rs`

- [ ] **Step 1: Write tests for angle math and braking distance**

Add to tests module:

```rust
#[test]
fn angle_between_same_direction_is_zero() {
    let a = angle_between_directions(Vec2::X, Vec2::X);
    assert!(a.abs() < 0.001);
}

#[test]
fn angle_between_opposite_is_pi() {
    let a = angle_between_directions(Vec2::X, Vec2::NEG_X);
    assert!((a - PI).abs() < 0.001);
}

#[test]
fn angle_between_perpendicular_is_half_pi() {
    let a = angle_between_directions(Vec2::X, Vec2::Y);
    assert!((a - PI / 2.0).abs() < 0.001);
}

#[test]
fn braking_distance_at_zero_speed_is_zero() {
    assert_eq!(braking_distance(0.0, 10.0), 0.0);
}

#[test]
fn braking_distance_increases_with_speed() {
    let d1 = braking_distance(10.0, 5.0);
    let d2 = braking_distance(20.0, 5.0);
    assert!(d2 > d1);
}

#[test]
fn braking_distance_formula() {
    // v²/(2a) = 100/(2*5) = 10
    let d = braking_distance(10.0, 5.0);
    assert!((d - 10.0).abs() < 0.01);
}

#[test]
fn shortest_angle_positive() {
    let a = shortest_angle_delta(0.0, 1.0);
    assert!((a - 1.0).abs() < 0.001);
}

#[test]
fn shortest_angle_wraps_around() {
    // From 350° to 10° should be +20°, not -340°
    let from = 350.0_f32.to_radians();
    let to = 10.0_f32.to_radians();
    let delta = shortest_angle_delta(from, to);
    assert!((delta - 20.0_f32.to_radians()).abs() < 0.01);
}

#[test]
fn shortest_angle_negative() {
    // From 10° to 350° should be -20°
    let from = 10.0_f32.to_radians();
    let to = 350.0_f32.to_radians();
    let delta = shortest_angle_delta(from, to);
    assert!((delta - (-20.0_f32.to_radians())).abs() < 0.01);
}

#[test]
fn ship_xz_extracts_correctly() {
    let transform = Transform::from_xyz(10.0, 5.0, -20.0);
    assert_eq!(ship_xz_position(&transform), Vec2::new(10.0, -20.0));
}

#[test]
fn facing_direction_from_transform() {
    // Default transform faces -Z
    let t = Transform::default();
    let dir = ship_facing_direction(&t);
    assert!((dir - Vec2::new(0.0, -1.0)).length() < 0.01);
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test`
Expected: FAIL — functions not defined

- [ ] **Step 3: Implement pure helper functions**

Add to `src/ship/mod.rs` (after `thrust_multiplier`):

```rust
/// Unsigned angle (0..PI) between two unit vectors.
pub fn angle_between_directions(a: Vec2, b: Vec2) -> f32 {
    a.dot(b).clamp(-1.0, 1.0).acos()
}

/// Braking distance: v²/(2a). How far you travel while decelerating to zero.
pub fn braking_distance(speed: f32, deceleration: f32) -> f32 {
    if deceleration <= 0.0 || speed <= 0.0 {
        return 0.0;
    }
    (speed * speed) / (2.0 * deceleration)
}

/// Signed shortest angle from `from` to `to` in radians (-PI..PI).
pub fn shortest_angle_delta(from: f32, to: f32) -> f32 {
    let mut delta = (to - from) % std::f32::consts::TAU;
    if delta > std::f32::consts::PI {
        delta -= std::f32::consts::TAU;
    } else if delta < -std::f32::consts::PI {
        delta += std::f32::consts::TAU;
    }
    delta
}

/// Extract XZ position from transform as Vec2.
pub fn ship_xz_position(transform: &Transform) -> Vec2 {
    Vec2::new(transform.translation.x, transform.translation.z)
}

/// Get the ship's facing direction as a Vec2 in XZ plane.
/// Ships face -Z by default in Bevy, so forward = -Z.
pub fn ship_facing_direction(transform: &Transform) -> Vec2 {
    let forward = transform.forward();
    Vec2::new(forward.x, forward.z).normalize_or_zero()
}

/// Get the current heading angle (radians) of the ship in XZ plane.
/// 0 = +X, PI/2 = +Z (south), etc.
pub fn ship_heading(transform: &Transform) -> f32 {
    let dir = ship_facing_direction(transform);
    dir.y.atan2(dir.x)
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test`
Expected: all new tests pass

- [ ] **Step 5: Commit**

```bash
git add src/ship/mod.rs && git commit -m "feat: add pure physics helpers (angles, braking, facing)"
```

---

## Chunk 3: Physics Systems

### Task 5: Implement turning system

**Files:**
- Modify: `src/ship/mod.rs`

- [ ] **Step 1: Write test for angular deceleration distance**

```rust
#[test]
fn angular_braking_distance_formula() {
    // Same as linear: ω²/(2α)
    let d = braking_distance(2.0, 1.0); // reuse same formula
    assert!((d - 2.0).abs() < 0.01);
}
```

- [ ] **Step 2: Implement update_facing_targets system**

Replace the stub with:

```rust
fn update_facing_targets(
    mut commands: Commands,
    mut query: Query<
        (Entity, &Transform, &WaypointQueue, Option<&FacingLocked>),
        With<Ship>,
    >,
) {
    for (entity, transform, waypoints, locked) in &mut query {
        if locked.is_some() {
            // Locked ships keep their player-set FacingTarget
            continue;
        }

        if let Some(&next_wp) = waypoints.waypoints.front() {
            let pos = ship_xz_position(transform);
            let dir = (next_wp - pos).normalize_or_zero();
            if dir != Vec2::ZERO {
                commands.entity(entity).insert(FacingTarget { direction: dir });
            }
        } else {
            // No waypoints and unlocked — remove facing target so ship stops turning
            commands.entity(entity).remove::<FacingTarget>();
        }
    }
}
```

- [ ] **Step 3: Implement turn_ships system**

Replace the stub with:

```rust
fn turn_ships(
    time: Res<Time>,
    mut query: Query<
        (&mut Transform, &mut Velocity, &ShipClass, Option<&FacingTarget>),
        With<Ship>,
    >,
) {
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }

    for (mut transform, mut velocity, class, facing_target) in &mut query {
        let profile = class.profile();

        let Some(target) = facing_target else {
            // No target — decelerate angular velocity to zero
            if velocity.angular.abs() > 0.001 {
                let decel = profile.turn_acceleration * dt;
                if velocity.angular.abs() <= decel {
                    velocity.angular = 0.0;
                } else {
                    velocity.angular -= velocity.angular.signum() * decel;
                }
            } else {
                velocity.angular = 0.0;
            }

            // Still apply any remaining angular velocity
            if velocity.angular.abs() > 0.001 {
                let current_heading = ship_heading(&transform);
                let new_heading = current_heading + velocity.angular * dt;
                let dir = Vec2::new(new_heading.cos(), new_heading.sin());
                let look_target = Vec3::new(
                    transform.translation.x + dir.x,
                    transform.translation.y,
                    transform.translation.z + dir.y,
                );
                transform.look_at(look_target, Vec3::Y);
            }
            continue;
        };

        let target_heading = target.direction.y.atan2(target.direction.x);
        let current_heading = ship_heading(&transform);
        let delta = shortest_angle_delta(current_heading, target_heading);

        // If close enough to target, snap and zero angular velocity
        if delta.abs() < 0.01 && velocity.angular.abs() < 0.01 {
            velocity.angular = 0.0;
            let look_target = Vec3::new(
                transform.translation.x + target.direction.x,
                transform.translation.y,
                transform.translation.z + target.direction.y,
            );
            transform.look_at(look_target, Vec3::Y);
            continue;
        }

        // Calculate if we need to decelerate to stop at target angle
        let stop_distance = braking_distance(velocity.angular.abs(), profile.turn_acceleration);
        let should_brake = stop_distance >= delta.abs();

        if should_brake {
            // Decelerate
            let decel = profile.turn_acceleration * dt;
            if velocity.angular.abs() <= decel {
                velocity.angular = 0.0;
            } else {
                velocity.angular -= velocity.angular.signum() * decel;
            }
        } else {
            // Accelerate toward target
            let desired_sign = delta.signum();
            velocity.angular += desired_sign * profile.turn_acceleration * dt;
            // Clamp to max turn rate
            velocity.angular = velocity.angular.clamp(-profile.turn_rate, profile.turn_rate);
        }

        // Apply angular velocity
        let new_heading = current_heading + velocity.angular * dt;
        let dir = Vec2::new(new_heading.cos(), new_heading.sin());
        let look_target = Vec3::new(
            transform.translation.x + dir.x,
            transform.translation.y,
            transform.translation.z + dir.y,
        );
        transform.look_at(look_target, Vec3::Y);
    }
}
```

- [ ] **Step 4: Verify it compiles**

Run: `cargo check`

- [ ] **Step 5: Commit**

```bash
git add src/ship/mod.rs && git commit -m "feat: implement turning system with angular velocity"
```

### Task 6: Implement thrust and velocity systems

**Files:**
- Modify: `src/ship/mod.rs`

- [ ] **Step 1: Implement apply_thrust system**

Replace the stub with:

```rust
fn apply_thrust(
    time: Res<Time>,
    mut query: Query<
        (&Transform, &mut Velocity, &ShipClass, &WaypointQueue),
        With<Ship>,
    >,
    asteroid_query: Query<(&Transform, &AsteroidSize), With<Asteroid>>,
) {
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }

    for (transform, mut velocity, class, waypoints) in &mut query {
        let profile = class.profile();
        let facing = ship_facing_direction(transform);
        let speed = velocity.linear.length();

        // Auto-brake: no waypoints left and braking flag set
        if waypoints.braking && waypoints.waypoints.is_empty() {
            if speed > 0.1 {
                // Brake: apply deceleration opposing current velocity
                let vel_dir = velocity.linear.normalize_or_zero();
                let angle = angle_between_directions(facing, -vel_dir);
                let effective_decel = profile.acceleration * thrust_multiplier(angle, profile.thruster_factor);
                let decel_amount = effective_decel * dt;
                if speed <= decel_amount {
                    velocity.linear = Vec2::ZERO;
                } else {
                    velocity.linear -= vel_dir * decel_amount;
                }
            } else {
                velocity.linear = Vec2::ZERO;
            }
            continue;
        }

        // No waypoints, not braking — drift (no thrust applied)
        if waypoints.waypoints.is_empty() {
            continue;
        }

        // Has waypoints — thrust toward next waypoint
        let pos = ship_xz_position(transform);
        let next_wp = waypoints.waypoints[0];
        let to_target = next_wp - pos;
        let dist = to_target.length();

        if dist < 1.0 {
            continue;
        }

        let desired_dir = to_target / dist;
        let angle = angle_between_directions(facing, desired_dir);
        let multiplier = thrust_multiplier(angle, profile.thruster_factor);
        let effective_accel = profile.acceleration * multiplier;
        let effective_top_speed = profile.top_speed * multiplier;

        // Check if we're approaching the last waypoint and need to start braking
        let is_last = waypoints.waypoints.len() == 1;
        if is_last {
            let brake_dist = braking_distance(speed, effective_accel);
            if brake_dist >= dist {
                // Begin braking
                let vel_dir = velocity.linear.normalize_or_zero();
                let brake_angle = angle_between_directions(facing, -vel_dir);
                let brake_decel = profile.acceleration * thrust_multiplier(brake_angle, profile.thruster_factor);
                let decel_amount = brake_decel * dt;
                if speed <= decel_amount {
                    velocity.linear = Vec2::ZERO;
                } else {
                    velocity.linear -= vel_dir * decel_amount;
                }
                continue;
            }
        }

        // Accelerate toward waypoint
        if speed < effective_top_speed {
            velocity.linear += desired_dir * effective_accel * dt;
            // Clamp to effective top speed
            let new_speed = velocity.linear.length();
            if new_speed > effective_top_speed {
                velocity.linear = velocity.linear.normalize() * effective_top_speed;
            }
        } else if speed > effective_top_speed {
            // Over effective top speed (e.g. turned away from velocity) — decelerate
            let excess = speed - effective_top_speed;
            let decel = (effective_accel * dt).min(excess);
            velocity.linear = velocity.linear.normalize() * (speed - decel);
        }
    }
}
```

- [ ] **Step 2: Implement apply_velocity system**

Replace the stub with:

```rust
fn apply_velocity(
    time: Res<Time>,
    mut query: Query<(&mut Transform, &Velocity), With<Ship>>,
) {
    let dt = time.delta_secs();
    for (mut transform, velocity) in &mut query {
        transform.translation.x += velocity.linear.x * dt;
        transform.translation.z += velocity.linear.y * dt;
    }
}
```

- [ ] **Step 3: Implement check_waypoint_arrival system**

Replace the stub with:

```rust
const ARRIVAL_THRESHOLD: f32 = 10.0;

fn check_waypoint_arrival(
    mut query: Query<(&Transform, &mut WaypointQueue, &Velocity), With<Ship>>,
) {
    for (transform, mut waypoints, velocity) in &mut query {
        if let Some(&next_wp) = waypoints.waypoints.front() {
            let pos = ship_xz_position(transform);
            let dist = (next_wp - pos).length();

            if dist < ARRIVAL_THRESHOLD {
                waypoints.waypoints.pop_front();
                if waypoints.waypoints.is_empty() {
                    waypoints.braking = true;
                }
            }
        }
    }
}
```

- [ ] **Step 4: Update clamp_ships_to_bounds**

Keep existing but update to use new types:

```rust
fn clamp_ships_to_bounds(
    bounds: Res<MapBounds>,
    mut query: Query<(&mut Transform, &mut Velocity), With<Ship>>,
) {
    for (mut transform, mut velocity) in &mut query {
        let pos = ship_xz_position(&transform);
        let clamped = bounds.clamp(pos);
        if pos != clamped {
            transform.translation.x = clamped.x;
            transform.translation.z = clamped.y;
            // Kill velocity component that would push out of bounds
            if (pos.x - clamped.x).abs() > 0.01 {
                velocity.linear.x = 0.0;
            }
            if (pos.y - clamped.y).abs() > 0.01 {
                velocity.linear.y = 0.0;
            }
        }
    }
}
```

- [ ] **Step 5: Verify it compiles and tests pass**

Run: `cargo check && cargo test`

- [ ] **Step 6: Commit**

```bash
git add src/ship/mod.rs && git commit -m "feat: implement thrust, velocity, and waypoint arrival systems"
```

---

## Chunk 4: Fog System Update & Ship Spawning

### Task 7: Update fog system to use ShipClass

**Files:**
- Modify: `src/fog/mod.rs`

- [ ] **Step 1: Update fog to read vision_range from ShipClass**

Replace `ShipStats` import with `ShipClass`. In `detect_enemies`, change the query
from `&ShipStats` to `&ShipClass` and get `vision_range` via `class.profile().vision_range`.

```rust
// Change import:
use crate::ship::{Ship, ShipClass, ship_xz_position};

// In detect_enemies, change query:
player_ships: Query<(&Transform, &ShipClass, &Team), With<Ship>>,

// And usage:
let profile = class.profile();
if is_in_los(player_pos, enemy_pos, profile.vision_range, &asteroids) {
```

- [ ] **Step 2: Verify it compiles and fog tests pass**

Run: `cargo check && cargo test`
Expected: fog tests still pass (they test pure functions, not systems)

- [ ] **Step 3: Commit**

```bash
git add src/fog/mod.rs && git commit -m "refactor: fog reads vision_range from ShipClass profile"
```

### Task 8: Rewrite spawn_ship with ShipClass and new meshes

**Files:**
- Modify: `src/ship/mod.rs`

- [ ] **Step 1: Rewrite spawn_ship**

Replace existing `spawn_ship` function:

```rust
pub fn spawn_ship(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
    position: Vec2,
    team: Team,
    color: Color,
    class: ShipClass,
) -> Entity {
    let profile = class.profile();

    let ship_mesh = match class {
        ShipClass::Battleship => meshes.add(Cuboid::new(12.0, 8.0, 28.0)),
        ShipClass::Destroyer => meshes.add(Cone { radius: 8.0, height: 20.0 }),
        ShipClass::Scout => meshes.add(Sphere::new(5.0).mesh().uv(16, 16)),
    };

    // Child mesh rotation: align forward (-Z) with mesh's natural orientation
    let mesh_rotation = match class {
        ShipClass::Battleship => Quat::IDENTITY, // Cuboid: long axis is Z, already aligned
        ShipClass::Destroyer => Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2), // Cone tip +Y → -Z
        ShipClass::Scout => Quat::IDENTITY, // Sphere: no orientation needed
    };

    let is_enemy = team != Team::PLAYER;
    let ship_material = materials.add(StandardMaterial {
        base_color: if is_enemy { color.with_alpha(0.0) } else { color },
        emissive: color.into(),
        alpha_mode: if is_enemy { AlphaMode::Blend } else { AlphaMode::Opaque },
        ..default()
    });

    let initial_visibility = if is_enemy {
        Visibility::Hidden
    } else {
        Visibility::Visible
    };

    let mut entity_commands = commands.spawn((
        Ship,
        team,
        class,
        Velocity::default(),
        WaypointQueue::default(),
        Transform::from_xyz(position.x, 5.0, position.y),
        initial_visibility,
    ));

    entity_commands.with_child((
        Mesh3d(ship_mesh),
        MeshMaterial3d(ship_material),
        Transform::from_rotation(mesh_rotation),
    ));

    if is_enemy {
        entity_commands.insert((EnemyVisibility::default(), Health { hp: 3 }));
    }

    entity_commands.id()
}
```

- [ ] **Step 2: Verify it compiles**

Run: `cargo check`
Expected: errors in main.rs (spawn_ship signature changed) — that's expected, fixed next task

- [ ] **Step 3: Commit**

```bash
git add src/ship/mod.rs && git commit -m "feat: spawn_ship with ShipClass, distinct meshes per class"
```

### Task 9: Update main.rs setup to spawn fleet

**Files:**
- Modify: `src/main.rs`

- [ ] **Step 1: Rewrite setup_game**

Update imports and rewrite `setup_game` to spawn three player ships and scattered enemies:

```rust
use ship::{spawn_ship, ShipClass};

fn setup_game(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut next_state: ResMut<NextState<GameState>>,
) {
    commands.add_observer(on_ground_clicked);

    // Player fleet — near bottom-left corner
    let player_color = Color::srgb(0.2, 0.6, 1.0);

    let battleship = spawn_ship(
        &mut commands, &mut meshes, &mut materials,
        Vec2::new(-300.0, -300.0), Team::PLAYER, player_color, ShipClass::Battleship,
    );
    commands.entity(battleship).observe(on_ship_clicked);

    let destroyer = spawn_ship(
        &mut commands, &mut meshes, &mut materials,
        Vec2::new(-330.0, -260.0), Team::PLAYER, player_color, ShipClass::Destroyer,
    );
    commands.entity(destroyer).observe(on_ship_clicked);

    let scout = spawn_ship(
        &mut commands, &mut meshes, &mut materials,
        Vec2::new(-280.0, -330.0), Team::PLAYER, player_color, ShipClass::Scout,
    );
    commands.entity(scout).observe(on_ship_clicked);

    // Enemy ships scattered around the map
    let enemy_color = Color::srgb(1.0, 0.2, 0.2);
    let enemy_positions = [
        (Vec2::new(300.0, 300.0), ShipClass::Battleship),
        (Vec2::new(-200.0, 350.0), ShipClass::Destroyer),
        (Vec2::new(350.0, -100.0), ShipClass::Destroyer),
        (Vec2::new(0.0, 300.0), ShipClass::Scout),
        (Vec2::new(250.0, -300.0), ShipClass::Scout),
    ];

    for (pos, class) in enemy_positions {
        let enemy = spawn_ship(
            &mut commands, &mut meshes, &mut materials,
            pos, Team::ENEMY, enemy_color, class,
        );
        commands.entity(enemy).observe(on_ship_clicked);
    }

    next_state.set(GameState::Playing);
    info!("Game setup complete — entering Playing state");
}
```

- [ ] **Step 2: Remove old combat import**

Ensure `mod combat;` line is removed and no combat references remain.

- [ ] **Step 3: Verify it compiles and runs**

Run: `cargo check && cargo test`

- [ ] **Step 4: Commit**

```bash
git add src/main.rs && git commit -m "feat: spawn player fleet + scattered enemies with ship classes"
```

---

## Chunk 5: Input Rework

### Task 10: Rewrite input for waypoints and facing

**Files:**
- Modify: `src/input/mod.rs`

- [ ] **Step 1: Add LockMode resource and update InputPlugin**

```rust
use bevy::prelude::*;

use crate::game::Team;
use crate::map::GroundPlane;
use crate::ship::{
    FacingLocked, FacingTarget, Selected, SelectionIndicator, Ship, WaypointQueue,
};

pub struct InputPlugin;

/// Resource: when true, next right-click sets facing lock direction
#[derive(Resource, Default)]
pub struct LockMode(pub bool);

impl Plugin for InputPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<LockMode>()
            .add_systems(Startup, setup_selection_indicator)
            .add_systems(
                Update,
                (update_selection_indicator, handle_keyboard),
            );
    }
}
```

- [ ] **Step 2: Rewrite on_ground_clicked for waypoints and facing**

```rust
pub fn on_ground_clicked(
    click: On<Pointer<Click>>,
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    mut lock_mode: ResMut<LockMode>,
    ground_query: Query<Entity, With<GroundPlane>>,
    selected_query: Query<(Entity, &Transform), With<Selected>>,
) {
    let clicked_entity = click.event_target();
    if ground_query.get(clicked_entity).is_err() {
        return;
    }
    let Some(hit_pos) = click.hit.position else {
        return;
    };
    let destination = Vec2::new(hit_pos.x, hit_pos.z);

    // Alt+right-click: set facing direction and lock
    if click.button == PointerButton::Secondary
        && keys.pressed(KeyCode::AltLeft)
    {
        for (entity, transform) in &selected_query {
            let pos = Vec2::new(transform.translation.x, transform.translation.z);
            let dir = (destination - pos).normalize_or_zero();
            if dir != Vec2::ZERO {
                commands.entity(entity).insert(FacingTarget { direction: dir });
                commands.entity(entity).insert(FacingLocked);
            }
        }
        lock_mode.0 = false; // Exit lock mode if active
        return;
    }

    if click.button != PointerButton::Secondary {
        return;
    }

    // Lock mode: next right-click sets facing
    if lock_mode.0 {
        for (entity, transform) in &selected_query {
            let pos = Vec2::new(transform.translation.x, transform.translation.z);
            let dir = (destination - pos).normalize_or_zero();
            if dir != Vec2::ZERO {
                commands.entity(entity).insert(FacingTarget { direction: dir });
                commands.entity(entity).insert(FacingLocked);
            }
        }
        lock_mode.0 = false;
        return;
    }

    // Shift+right-click: append waypoint
    let shift = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);

    for (entity, _transform) in &selected_query {
        if shift {
            // Append to existing queue
            commands.queue(move |world: &mut World| {
                if let Some(mut wq) = world.get_mut::<WaypointQueue>(entity) {
                    wq.waypoints.push_back(destination);
                    wq.braking = false;
                }
            });
        } else {
            // Clear queue, set single waypoint
            let mut queue = WaypointQueue::default();
            queue.waypoints.push_back(destination);
            commands.entity(entity).insert(queue);
        }
    }
}
```

- [ ] **Step 3: Update on_ship_clicked to handle alt-click unlock**

```rust
pub fn on_ship_clicked(
    click: On<Pointer<Click>>,
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    ship_query: Query<(Entity, &Team), With<Ship>>,
    selected_query: Query<Entity, With<Selected>>,
) {
    let clicked_entity = click.event_target();
    let Ok((entity, team)) = ship_query.get(clicked_entity) else {
        return;
    };

    // Alt+right-click on own ship: unlock facing
    if click.button == PointerButton::Secondary
        && keys.pressed(KeyCode::AltLeft)
        && *team == Team::PLAYER
    {
        commands.entity(entity).remove::<FacingLocked>();
        return;
    }

    if click.button != PointerButton::Primary {
        return;
    }

    if *team != Team::PLAYER {
        return;
    }

    // Deselect previous
    for prev in &selected_query {
        commands.entity(prev).remove::<Selected>();
    }

    commands.entity(entity).insert(Selected);
}
```

- [ ] **Step 4: Rewrite handle_keyboard for L key and Escape**

```rust
fn handle_keyboard(
    keys: Res<ButtonInput<KeyCode>>,
    mut commands: Commands,
    mut lock_mode: ResMut<LockMode>,
    selected_query: Query<Entity, (With<Selected>, With<Ship>)>,
) {
    if keys.just_pressed(KeyCode::Escape) {
        for entity in &selected_query {
            commands.entity(entity).remove::<Selected>();
        }
        lock_mode.0 = false;
    }

    if keys.just_pressed(KeyCode::KeyL) {
        // If any selected ship is locked, unlock all
        // Otherwise toggle lock mode for next click
        let any_locked = selected_query.iter().next().is_some();
        // Check if any selected ship has FacingLocked
        // (simplified: toggle lock mode)
        lock_mode.0 = !lock_mode.0;
    }
}
```

Note: The L key handling needs access to `FacingLocked` query. Revised:

```rust
fn handle_keyboard(
    keys: Res<ButtonInput<KeyCode>>,
    mut commands: Commands,
    mut lock_mode: ResMut<LockMode>,
    selected_query: Query<Entity, With<Selected>>,
    locked_query: Query<Entity, (With<Selected>, With<FacingLocked>)>,
) {
    if keys.just_pressed(KeyCode::Escape) {
        for entity in &selected_query {
            commands.entity(entity).remove::<Selected>();
        }
        lock_mode.0 = false;
    }

    if keys.just_pressed(KeyCode::KeyL) {
        if locked_query.iter().next().is_some() {
            // Some selected ships are locked — unlock them
            for entity in &locked_query {
                commands.entity(entity).remove::<FacingLocked>();
            }
            lock_mode.0 = false;
        } else {
            // No selected ships locked — toggle lock mode
            lock_mode.0 = !lock_mode.0;
        }
    }
}
```

- [ ] **Step 5: Keep setup_selection_indicator and update_selection_indicator unchanged**

These functions stay as they are — they work with `Selected` and `SelectionIndicator` which haven't changed.

- [ ] **Step 6: Verify it compiles**

Run: `cargo check`

- [ ] **Step 7: Commit**

```bash
git add src/input/mod.rs && git commit -m "feat: input rework — waypoint queuing, facing lock/unlock, lock mode"
```

---

## Chunk 6: Visual Indicators

### Task 11: Waypoint markers and facing arrow

**Files:**
- Modify: `src/input/mod.rs` or `src/ship/mod.rs`

- [ ] **Step 1: Add waypoint marker components**

In `src/ship/mod.rs`, add:

```rust
/// Marker for waypoint indicator entities. Stores owning ship.
#[derive(Component)]
pub struct WaypointMarker {
    pub owner: Entity,
}

/// Marker for facing direction indicator. Stores owning ship.
#[derive(Component)]
pub struct FacingIndicator {
    pub owner: Entity,
}
```

- [ ] **Step 2: Add visual indicator systems to ShipPlugin**

Add new systems to `ShipPlugin::build`:

```rust
app.add_systems(Update, (
    update_waypoint_markers,
    update_facing_indicators,
).after(check_waypoint_arrival));
```

- [ ] **Step 3: Implement waypoint marker system**

```rust
fn update_waypoint_markers(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    ship_query: Query<(Entity, &WaypointQueue), With<Ship>>,
    marker_query: Query<(Entity, &WaypointMarker)>,
) {
    // Despawn all existing markers
    for (entity, _) in &marker_query {
        commands.entity(entity).despawn();
    }

    // Spawn new markers for each ship's waypoints
    for (ship_entity, waypoints) in &ship_query {
        for wp in &waypoints.waypoints {
            commands.spawn((
                WaypointMarker { owner: ship_entity },
                Mesh3d(meshes.add(Sphere::new(2.0))),
                MeshMaterial3d(materials.add(StandardMaterial {
                    base_color: Color::srgba(0.2, 0.8, 1.0, 0.4),
                    alpha_mode: AlphaMode::Blend,
                    unlit: true,
                    ..default()
                })),
                Transform::from_xyz(wp.x, 1.0, wp.y),
            ));
        }
    }
}
```

- [ ] **Step 4: Implement facing indicator system**

```rust
fn update_facing_indicators(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    ship_query: Query<(Entity, &Transform, Option<&FacingLocked>, Option<&FacingTarget>), With<Ship>>,
    indicator_query: Query<(Entity, &FacingIndicator)>,
) {
    // Despawn all existing facing indicators
    for (entity, _) in &indicator_query {
        commands.entity(entity).despawn();
    }

    // Spawn facing indicator for locked ships only
    for (ship_entity, transform, locked, facing) in &ship_query {
        if locked.is_none() {
            continue;
        }
        let Some(target) = facing else {
            continue;
        };

        let pos = transform.translation;
        let arrow_len = 30.0;
        let end = Vec3::new(
            pos.x + target.direction.x * arrow_len,
            1.0,
            pos.z + target.direction.y * arrow_len,
        );

        commands.spawn((
            FacingIndicator { owner: ship_entity },
            Mesh3d(meshes.add(Capsule3d::new(0.5, arrow_len))),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color: Color::srgba(1.0, 0.8, 0.2, 0.6),
                alpha_mode: AlphaMode::Blend,
                unlit: true,
                ..default()
            })),
            Transform::from_translation(Vec3::new(
                pos.x + target.direction.x * arrow_len / 2.0,
                1.0,
                pos.z + target.direction.y * arrow_len / 2.0,
            ))
            .looking_at(end, Vec3::Y),
        ));
    }
}
```

Note: This implementation despawns and respawns every frame which is wasteful. For Phase 1 this is acceptable — optimize later if it becomes a performance issue.

- [ ] **Step 5: Verify it compiles**

Run: `cargo check`

- [ ] **Step 6: Commit**

```bash
git add src/ship/mod.rs && git commit -m "feat: waypoint markers and facing direction indicators"
```

---

## Chunk 7: Integration & Polish

### Task 12: Remove old ShipStats references

**Files:**
- Modify: any files still referencing `ShipStats` or `MovementTarget`

- [ ] **Step 1: Search for remaining old type references**

Run: `grep -r "ShipStats\|MovementTarget" src/`

Fix any remaining references. The fog system was already updated in Task 7.

- [ ] **Step 2: Verify clean compile and all tests pass**

Run: `cargo check && cargo test`

- [ ] **Step 3: Run the game**

Run: `cargo run`

Verify:
- Three player ships visible (different sizes/shapes)
- Ships respond to right-click move orders
- Ships accelerate and decelerate (not instant movement)
- Ships turn gradually (angular velocity ramp)
- Shift+click queues waypoints
- Alt+right-click locks facing direction
- Alt+right-click on ship unlocks
- L key toggles lock mode
- Ships auto-brake at final waypoint
- Enemy ships scattered, fade in when you approach
- Different ship classes feel distinct (scout zippy, battleship sluggish)

- [ ] **Step 4: Commit**

```bash
git add -A && git commit -m "feat: Phase 1 core simulation complete — physics movement, facing, waypoints, ship classes"
```

### Task 13: Update CLAUDE.md and module docs

**Files:**
- Modify: `CLAUDE.md`
- Modify: `src/ship/CLAUDE.md`
- Modify: `src/input/CLAUDE.md`
- Delete: `src/combat/CLAUDE.md`

- [ ] **Step 1: Update root CLAUDE.md**

Update the architecture section to reflect:
- Ship module now has ShipClass, ShipProfile, Velocity, WaypointQueue, FacingTarget/FacingLocked
- Physics update loop ordering
- Input module has facing lock/unlock, waypoint queuing, lock mode
- Combat module removed
- Test count updated

- [ ] **Step 2: Update module CLAUDE.md files**

Update `src/ship/CLAUDE.md` and `src/input/CLAUDE.md` to reflect new types and systems.
Delete `src/combat/CLAUDE.md`.

- [ ] **Step 3: Commit**

```bash
git add -A && git commit -m "docs: update CLAUDE.md for Phase 1 changes"
```
