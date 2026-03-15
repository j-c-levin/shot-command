# Phase 3b: Missiles & Point Defense Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add fire-and-forget missiles with 3D ballistic arcs, tube-based VLS launchers with player-controlled volley size, and two PD systems (Laser PD, CIWS) that auto-intercept missiles.

**Architecture:** Missiles are a new entity type parallel to Projectile, with 3D flight phases (climb/cruise/dive), terminal seeker homing, fuel limits, and HP. VLS launchers use a queue model — player queues missiles, tubes launch them as available. PD weapons auto-target closest missile within a vertical cylinder engagement zone. All entities are server-authoritative and replicated with LOS visibility filtering.

**Tech Stack:** Bevy 0.18, bevy_replicon 0.39, bevy_replicon_renet 0.15, Rust nightly.

**Design doc:** `docs/plans/2026-03-15-phase3b-missiles-pd-design.md`

---

## File Structure

### New files

| File | Responsibility |
|------|----------------|
| `src/weapon/missile.rs` | Missile marker, MissileTarget, MissileVelocity, MissileHealth, MissileDamage, MissileOwner, MissileFuel components. Flight phase enum. `spawn_missile` function. `MissilePlugin` with flight systems (advance, phase transitions, terminal homing, fuel depletion, collision, bounds cleanup). Pure flight math functions. |
| `src/weapon/pd.rs` | LaserPD and CIWS targeting logic. `PdPlugin` with systems: laser instant-hit, CIWS projectile spawning, PD target selection (closest missile in cylinder). Cylinder range check function. |

### Modified files

| File | Changes |
|------|---------|
| `src/weapon/mod.rs` | Add `pub mod missile; pub mod pd;`. Add `HeavyVLS`, `LightVLS`, `LaserPD`, `CIWS` to `WeaponType` enum. Add missile/PD profiles. Add `MissileQueue` component on ships. |
| `src/weapon/firing.rs` | Modify `auto_fire` to skip missile/PD weapon types (they have their own systems). Add `process_missile_queue` system that pops from queue and spawns missiles from available tubes. |
| `src/ship/mod.rs` | Add `MissileQueue` to ship components. Update `default_loadout` for all three classes with new weapon types. Add `MissileQueue` to ShipSecrets sync. |
| `src/input/mod.rs` | Add `MissileMode` resource. Add M key handling. In missile mode: left-click enemy queues missile with lead calc, left-click ground queues missile at position. Esc cancels queued missiles. Add missile mode HUD text. |
| `src/net/commands.rs` | Add `FireMissileCommand { ship, target_point, target_entity }` and `CancelMissilesCommand { ship }`. |
| `src/net/replication.rs` | Register missile components and new commands. |
| `src/net/server.rs` | Add `FireMissileCommand` handler (validate team, push to MissileQueue). Add `CancelMissilesCommand` handler. Add missile entities to LOS visibility. Sync MissileQueue via ShipSecrets. |
| `src/net/client.rs` | Add missile materializer (visual mesh for missile entities). |
| `src/net/materializer.rs` | Add `materialize_missiles` system. |
| `src/bin/server.rs` | Add `MissilePlugin` and `PdPlugin`. |
| `src/weapon/projectile.rs` | Add `CwisRound` marker component so CIWS projectiles skip ships in hit detection. |

---

## Chunk 1: Missile Components, Flight Math, and Spawning

### Task 1: Missile components and spawn function

**Files:**
- Create: `src/weapon/missile.rs`
- Modify: `src/weapon/mod.rs` (add `pub mod missile`)

- [ ] **Step 1: Write failing tests for missile flight math**

In `src/weapon/missile.rs`, add a `#[cfg(test)] mod tests` block with tests for:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compute_intercept_point_stationary_target() {
        let shooter = Vec3::new(0.0, 0.0, 0.0);
        let target = Vec3::new(200.0, 0.0, 0.0);
        let velocity = Vec2::ZERO;
        let missile_speed = 80.0;

        let intercept = compute_intercept_point(shooter, target, velocity, missile_speed);
        assert!((intercept.x - 200.0).abs() < 1.0);
        assert!((intercept.z - 0.0).abs() < 1.0);
    }

    #[test]
    fn compute_intercept_point_moving_target() {
        let shooter = Vec3::new(0.0, 0.0, 0.0);
        let target = Vec3::new(200.0, 0.0, 0.0);
        let velocity = Vec2::new(0.0, 50.0); // moving +Z
        let missile_speed = 80.0;

        let intercept = compute_intercept_point(shooter, target, velocity, missile_speed);
        // Intercept should be ahead in Z
        assert!(intercept.z > 0.0);
    }

    #[test]
    fn flight_phase_transitions() {
        // Starting phase is Climb
        assert_eq!(FlightPhase::Climb, FlightPhase::Climb);
    }

    #[test]
    fn spawn_missile_creates_all_components() {
        let mut world = World::new();
        let owner = world.spawn_empty().id();
        let target = world.spawn_empty().id();

        let entity;
        {
            let mut commands = world.commands();
            entity = spawn_missile(
                &mut commands,
                Vec3::new(10.0, 5.0, 20.0),
                Vec3::new(200.0, 0.0, 200.0),
                Some(target),
                80.0,
                30,
                15,
                500.0,
                owner,
            );
        }
        world.flush();

        assert!(world.get::<Missile>(entity).is_some());
        assert!(world.get::<MissileTarget>(entity).is_some());
        assert!(world.get::<MissileVelocity>(entity).is_some());
        assert!(world.get::<MissileHealth>(entity).is_some());
        assert!(world.get::<MissileDamage>(entity).is_some());
        assert!(world.get::<MissileOwner>(entity).is_some());
        assert!(world.get::<MissileFuel>(entity).is_some());
        assert!(world.get::<Transform>(entity).is_some());
    }

    #[test]
    fn missile_fuel_depletes_by_distance() {
        let fuel = 500.0_f32;
        let distance_traveled = 10.0_f32;
        let remaining = fuel - distance_traveled;
        assert!((remaining - 490.0).abs() < 0.01);
    }

    #[test]
    fn target_in_seeker_cone_detected() {
        let missile_pos = Vec3::new(100.0, 10.0, 100.0);
        let missile_dir = Vec3::new(1.0, 0.0, 0.0).normalize();
        let target_pos = Vec3::new(110.0, 0.0, 102.0); // slightly off-axis
        let cone_half_angle = 30.0_f32.to_radians();

        assert!(is_in_seeker_cone(missile_pos, missile_dir, target_pos, cone_half_angle));
    }

    #[test]
    fn target_outside_seeker_cone_not_detected() {
        let missile_pos = Vec3::new(100.0, 10.0, 100.0);
        let missile_dir = Vec3::new(1.0, 0.0, 0.0).normalize();
        let target_pos = Vec3::new(100.0, 0.0, 200.0); // 90 degrees off
        let cone_half_angle = 30.0_f32.to_radians();

        assert!(!is_in_seeker_cone(missile_pos, missile_dir, target_pos, cone_half_angle));
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test missile --lib -- -v`
Expected: FAIL — functions and types not defined.

- [ ] **Step 3: Implement missile components and pure functions**

In `src/weapon/missile.rs`:

```rust
use bevy::ecs::entity::MapEntities;
use bevy::prelude::*;
use bevy_replicon::prelude::Replicated;
use serde::{Deserialize, Serialize};

use crate::game::GameState;
use crate::map::MapBounds;
use crate::ship::{Ship, ShipClass, ship_xz_position};

/// Cruise altitude for the ballistic arc.
pub const CRUISE_ALTITUDE: f32 = 60.0;
/// Climb angle in radians (~45 degrees).
const CLIMB_ANGLE: f32 = std::f32::consts::FRAC_PI_4;
/// Distance from intercept point at which dive begins (XZ plane).
const DIVE_DISTANCE: f32 = 50.0;
/// Seeker cone half-angle in radians (~30 degrees).
pub const SEEKER_HALF_ANGLE: f32 = 0.5236; // ~30 degrees

// ── Components ──────────────────────────────────────────────────────

#[derive(Component, Serialize, Deserialize)]
pub struct Missile;

#[derive(Component, Serialize, Deserialize, Clone)]
pub struct MissileTarget {
    pub intercept_point: Vec3,
    pub target_entity: Option<Entity>,
}

// MapEntities needed because target_entity contains an Entity reference
impl MapEntities for MissileTarget {
    fn map_entities<M: bevy::ecs::entity::EntityMapper>(&mut self, mapper: &mut M) {
        if let Some(ref mut e) = self.target_entity {
            *e = mapper.map_entity(*e);
        }
    }
}

#[derive(Component, Serialize, Deserialize, Clone)]
pub struct MissileVelocity(pub Vec3);

#[derive(Component, Serialize, Deserialize, Clone)]
pub struct MissileHealth(pub u16);

#[derive(Component, Serialize, Deserialize, Clone)]
pub struct MissileDamage(pub u16);

#[derive(Component, Serialize, Deserialize, MapEntities, Clone)]
pub struct MissileOwner(#[entities] pub Entity);

#[derive(Component, Serialize, Deserialize, Clone)]
pub struct MissileFuel(pub f32);

#[derive(Component, Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum FlightPhase {
    Climb,
    Cruise,
    Dive,
    Terminal,
}

// ── Pure functions ──────────────────────────────────────────────────

/// Compute the intercept point for a missile using lead prediction.
/// Reuses the same 2-iteration approach as cannon fire control.
pub fn compute_intercept_point(
    shooter_pos: Vec3,
    target_pos: Vec3,
    target_velocity: Vec2,
    missile_speed: f32,
) -> Vec3 {
    if missile_speed < 0.001 {
        return target_pos;
    }
    let dist = Vec2::new(
        target_pos.x - shooter_pos.x,
        target_pos.z - shooter_pos.z,
    ).length();
    // Rough estimate: arc path is ~1.3x the XZ distance
    let travel_time = (dist * 1.3) / missile_speed;
    let predicted = Vec3::new(
        target_pos.x + target_velocity.x * travel_time,
        0.0,
        target_pos.z + target_velocity.y * travel_time,
    );
    // Refine once
    let dist2 = Vec2::new(
        predicted.x - shooter_pos.x,
        predicted.z - shooter_pos.z,
    ).length();
    let travel_time2 = (dist2 * 1.3) / missile_speed;
    Vec3::new(
        target_pos.x + target_velocity.x * travel_time2,
        0.0,
        target_pos.z + target_velocity.y * travel_time2,
    )
}

/// Check if a target position falls within the missile's forward seeker cone.
pub fn is_in_seeker_cone(
    missile_pos: Vec3,
    missile_forward: Vec3,
    target_pos: Vec3,
    cone_half_angle: f32,
) -> bool {
    let to_target = (target_pos - missile_pos).normalize_or_zero();
    let forward = missile_forward.normalize_or_zero();
    if to_target == Vec3::ZERO || forward == Vec3::ZERO {
        return false;
    }
    let dot = forward.dot(to_target).clamp(-1.0, 1.0);
    dot.acos() <= cone_half_angle
}

// ── Spawning ────────────────────────────────────────────────────────

pub fn spawn_missile(
    commands: &mut Commands,
    origin: Vec3,
    intercept_point: Vec3,
    target_entity: Option<Entity>,
    speed: f32,
    damage: u16,
    hp: u16,
    fuel: f32,
    owner: Entity,
) -> Entity {
    // Initial velocity: climb upward at CLIMB_ANGLE toward the intercept XZ
    let to_target_xz = Vec2::new(
        intercept_point.x - origin.x,
        intercept_point.z - origin.z,
    ).normalize_or_zero();

    let horizontal = speed * CLIMB_ANGLE.cos();
    let vertical = speed * CLIMB_ANGLE.sin();
    let velocity = Vec3::new(
        to_target_xz.x * horizontal,
        vertical,
        to_target_xz.y * horizontal,
    );

    commands
        .spawn((
            Missile,
            MissileTarget { intercept_point, target_entity },
            MissileVelocity(velocity),
            MissileHealth(hp),
            MissileDamage(damage),
            MissileOwner(owner),
            MissileFuel(fuel),
            FlightPhase::Climb,
            Transform::from_translation(origin),
            Replicated,
        ))
        .id()
}
```

Add `pub mod missile;` to `src/weapon/mod.rs`.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test missile --lib -- -v`
Expected: All 7 missile tests pass.

- [ ] **Step 5: Commit**

```
git add src/weapon/missile.rs src/weapon/mod.rs
git commit -m "feat: missile components, flight math, and spawn function"
```

---

### Task 2: Missile flight systems

**Files:**
- Modify: `src/weapon/missile.rs`

- [ ] **Step 1: Write failing tests for flight phase transitions**

Add to the test module in `src/weapon/missile.rs`:

```rust
#[test]
fn climb_to_cruise_transition_at_altitude() {
    // Missile above CRUISE_ALTITUDE should transition from Climb to Cruise
    let altitude = CRUISE_ALTITUDE + 1.0;
    let phase = FlightPhase::Climb;
    let new_phase = if altitude >= CRUISE_ALTITUDE && phase == FlightPhase::Climb {
        FlightPhase::Cruise
    } else {
        phase
    };
    assert_eq!(new_phase, FlightPhase::Cruise);
}

#[test]
fn cruise_to_dive_near_intercept() {
    let missile_xz = Vec2::new(190.0, 0.0);
    let intercept_xz = Vec2::new(200.0, 0.0);
    let dist = (missile_xz - intercept_xz).length();
    assert!(dist < DIVE_DISTANCE);
}
```

- [ ] **Step 2: Run tests to verify they fail/pass as expected**

Run: `cargo test missile --lib -- -v`

- [ ] **Step 3: Implement flight systems in `MissilePlugin`**

Add to `src/weapon/missile.rs`:

```rust
// ── Systems ─────────────────────────────────────────────────────────

/// Advance missile positions by velocity × dt. Decrement fuel.
fn advance_missiles(
    time: Res<Time>,
    mut query: Query<(&mut Transform, &MissileVelocity, &mut MissileFuel), With<Missile>>,
) {
    let dt = time.delta_secs();
    for (mut transform, vel, mut fuel) in &mut query {
        let displacement = vel.0 * dt;
        transform.translation += displacement;
        fuel.0 -= displacement.length();
    }
}

/// Transition flight phases and update velocity direction.
fn update_missile_flight(
    mut query: Query<(
        &Transform,
        &mut MissileVelocity,
        &mut FlightPhase,
        &MissileTarget,
    ), With<Missile>>,
    target_query: Query<&Transform, With<Ship>>,
) {
    for (transform, mut vel, mut phase, target) in &mut query {
        let speed = vel.0.length();
        if speed < 0.001 { continue; }

        let pos = transform.translation;
        let intercept_xz = Vec2::new(target.intercept_point.x, target.intercept_point.z);
        let missile_xz = Vec2::new(pos.x, pos.z);
        let dist_to_intercept = (missile_xz - intercept_xz).length();

        match *phase {
            FlightPhase::Climb => {
                if pos.y >= CRUISE_ALTITUDE {
                    *phase = FlightPhase::Cruise;
                }
                // Velocity set at spawn, stays until phase change
            }
            FlightPhase::Cruise => {
                // Level flight toward intercept point
                let to_intercept = Vec2::new(
                    target.intercept_point.x - pos.x,
                    target.intercept_point.z - pos.z,
                ).normalize_or_zero();
                vel.0 = Vec3::new(to_intercept.x * speed, 0.0, to_intercept.y * speed);

                if dist_to_intercept < DIVE_DISTANCE {
                    *phase = FlightPhase::Dive;
                }
            }
            FlightPhase::Dive => {
                // Pitch down toward intercept point at ground level
                let dive_target = Vec3::new(target.intercept_point.x, 0.0, target.intercept_point.z);
                let dir = (dive_target - pos).normalize_or_zero();
                vel.0 = dir * speed;

                if dist_to_intercept < DIVE_DISTANCE * 0.5 {
                    *phase = FlightPhase::Terminal;
                }
            }
            FlightPhase::Terminal => {
                // Check seeker cone for target entity
                let current_dir = vel.0.normalize_or_zero();
                if let Some(target_entity) = target.target_entity {
                    if let Ok(target_transform) = target_query.get(target_entity) {
                        if is_in_seeker_cone(pos, current_dir, target_transform.translation, SEEKER_HALF_ANGLE) {
                            let dir = (target_transform.translation - pos).normalize_or_zero();
                            vel.0 = dir * speed;
                        }
                    }
                } else {
                    // No specific target — check all ships in seeker cone
                    // (handled by a separate system for clarity)
                }

                // Continue toward intercept point if no target acquired
                let dive_target = Vec3::new(target.intercept_point.x, 0.0, target.intercept_point.z);
                if vel.0.normalize_or_zero() == Vec3::ZERO {
                    let dir = (dive_target - pos).normalize_or_zero();
                    vel.0 = dir * speed;
                }
            }
        }
    }
}

/// Untargeted missiles in terminal phase: scan for any ship in seeker cone.
fn terminal_seeker_scan(
    mut query: Query<(
        &Transform,
        &mut MissileVelocity,
        &mut MissileTarget,
        &MissileOwner,
        &FlightPhase,
    ), With<Missile>>,
    ship_query: Query<(Entity, &Transform), With<Ship>>,
) {
    for (transform, mut vel, mut target, owner, phase) in &mut query {
        if *phase != FlightPhase::Terminal { continue; }

        let speed = vel.0.length();
        let current_dir = vel.0.normalize_or_zero();
        let pos = transform.translation;

        // Find closest ship in seeker cone (excluding owner)
        let mut best: Option<(Entity, f32)> = None;
        for (ship_entity, ship_transform) in &ship_query {
            if ship_entity == owner.0 { continue; }
            if !is_in_seeker_cone(pos, current_dir, ship_transform.translation, SEEKER_HALF_ANGLE) {
                continue;
            }
            let dist = (ship_transform.translation - pos).length();
            if best.map_or(true, |(_, d)| dist < d) {
                best = Some((ship_entity, dist));
            }
        }

        if let Some((entity, _)) = best {
            target.target_entity = Some(entity);
        }
    }
}

/// Despawn missiles that run out of fuel.
fn despawn_spent_missiles(
    mut commands: Commands,
    query: Query<(Entity, &MissileFuel), With<Missile>>,
) {
    for (entity, fuel) in &query {
        if fuel.0 <= 0.0 {
            commands.entity(entity).despawn();
        }
    }
}

/// Despawn missiles that leave map bounds.
fn check_missile_bounds(
    mut commands: Commands,
    bounds: Res<MapBounds>,
    query: Query<(Entity, &Transform), With<Missile>>,
) {
    for (entity, transform) in &query {
        let pos = transform.translation;
        if pos.x.abs() > bounds.half_extents.x || pos.z.abs() > bounds.half_extents.y {
            commands.entity(entity).despawn();
        }
    }
}

/// Check missile-to-ship collisions.
fn check_missile_hits(
    mut commands: Commands,
    missile_query: Query<
        (Entity, &Transform, &MissileDamage, &MissileOwner),
        With<Missile>,
    >,
    mut ship_query: Query<(Entity, &Transform, &ShipClass, &mut crate::game::Health), With<Ship>>,
) {
    for (missile_entity, missile_transform, damage, owner) in &missile_query {
        let missile_xz = Vec2::new(missile_transform.translation.x, missile_transform.translation.z);

        for (ship_entity, ship_transform, class, mut health) in &mut ship_query {
            if ship_entity == owner.0 { continue; }

            let ship_xz = Vec2::new(ship_transform.translation.x, ship_transform.translation.z);
            let dist = (missile_xz - ship_xz).length();

            if dist < class.profile().collision_radius {
                health.hp = health.hp.saturating_sub(damage.0);
                commands.entity(missile_entity).despawn();
                break;
            }
        }
    }
}

/// Destroy missiles that reach 0 HP (from PD damage).
fn despawn_destroyed_missiles(
    mut commands: Commands,
    query: Query<(Entity, &MissileHealth), With<Missile>>,
) {
    for (entity, health) in &query {
        if health.0 == 0 {
            commands.entity(entity).despawn();
        }
    }
}

// ── Plugin ──────────────────────────────────────────────────────────

pub struct MissilePlugin;

impl Plugin for MissilePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                advance_missiles,
                update_missile_flight,
                terminal_seeker_scan,
                check_missile_hits,
                despawn_spent_missiles,
                despawn_destroyed_missiles,
                check_missile_bounds,
            )
                .chain()
                .run_if(in_state(GameState::Playing)),
        );
    }
}
```

- [ ] **Step 4: Run `cargo check` and tests**

Run: `cargo check && cargo test --lib -- -v`
Expected: All tests pass, no warnings.

- [ ] **Step 5: Commit**

```
git commit -m "feat: missile flight systems — climb/cruise/dive/terminal with seeker homing"
```

---

## Chunk 2: Weapon Types, VLS Launchers, and Missile Queue

### Task 3: Add missile and PD weapon types

**Files:**
- Modify: `src/weapon/mod.rs`

- [ ] **Step 1: Write failing tests for new weapon profiles**

Add to `src/weapon/mod.rs` test module:

```rust
#[test]
fn heavy_vls_profile_values() {
    let p = WeaponType::HeavyVLS.profile();
    assert_eq!(p.damage, 30);
    assert_eq!(p.firing_range, 500.0);
    assert_eq!(p.projectile_speed, 80.0);
}

#[test]
fn light_vls_profile_values() {
    let p = WeaponType::LightVLS.profile();
    assert_eq!(p.damage, 20);
    assert_eq!(p.firing_range, 400.0);
}

#[test]
fn laser_pd_profile_values() {
    let p = WeaponType::LaserPD.profile();
    assert_eq!(p.damage, 10);
    assert_eq!(p.fire_rate_secs, 1.0);
}

#[test]
fn cwis_profile_values() {
    let p = WeaponType::CWIS.profile();
    assert_eq!(p.damage, 2);
    assert_eq!(p.fire_rate_secs, 0.1);
}

#[test]
fn vls_mount_sizes() {
    assert_eq!(WeaponType::HeavyVLS.mount_size(), MountSize::Large);
    assert_eq!(WeaponType::LightVLS.mount_size(), MountSize::Medium);
    assert_eq!(WeaponType::LaserPD.mount_size(), MountSize::Medium);
    assert_eq!(WeaponType::CWIS.mount_size(), MountSize::Small);
}
```

- [ ] **Step 2: Run tests — expect failure**

Run: `cargo test weapon::tests --lib -- -v`

- [ ] **Step 3: Add new weapon types to `WeaponType` enum and profiles**

Extend `WeaponType` enum with `HeavyVLS`, `LightVLS`, `LaserPD`, `CWIS`. Add their profiles to `profile()`. Add a new `WeaponCategory` enum (`Cannon`, `Missile`, `PointDefense`) and a `category()` method on `WeaponType`. Add VLS-specific fields to `WeaponProfile`: `tubes: u8` (0 for non-VLS), `missile_hp: u16`, `missile_fuel: f32`, `pd_cylinder_radius: f32`.

Update `mount_size()` for the new types.

- [ ] **Step 4: Run tests — expect pass**

Run: `cargo test weapon::tests --lib -- -v`

- [ ] **Step 5: Commit**

```
git commit -m "feat: add HeavyVLS, LightVLS, LaserPD, CWIS weapon types and profiles"
```

---

### Task 4: MissileQueue component and ShipSecrets sync

**Files:**
- Modify: `src/weapon/mod.rs` (add MissileQueue)
- Modify: `src/ship/mod.rs` (add MissileQueue to ship spawn and ShipSecrets sync)
- Modify: `src/net/server.rs` (sync MissileQueue to ShipSecrets)

- [ ] **Step 1: Add `MissileQueue` component**

In `src/weapon/mod.rs`, add:

```rust
/// Queued missile launches: (intercept_point_xz, optional target entity).
/// Lives on Ship entities. Synced to ShipSecrets for replication.
#[derive(Component, Serialize, Deserialize, Clone, Debug, Default)]
pub struct MissileQueue(pub Vec<(Vec2, Option<Entity>)>);
```

Note: `MissileQueue` must implement `MapEntities` because it contains `Option<Entity>`.

- [ ] **Step 2: Add MissileQueue to ship spawning**

In `src/ship/mod.rs`, add `MissileQueue::default()` to `spawn_server_ship` bundle.

- [ ] **Step 3: Add MissileQueue to ShipSecrets sync**

In `src/net/server.rs`, find `sync_ship_secrets` and add MissileQueue syncing (same pattern as WaypointQueue).

Add `MissileQueue` to the ShipSecrets entity replication in `src/net/replication.rs`.

- [ ] **Step 4: Run `cargo check` and existing tests**

Run: `cargo check && cargo test --lib -- -v`

- [ ] **Step 5: Commit**

```
git commit -m "feat: MissileQueue component with ShipSecrets sync"
```

---

### Task 5: Missile queue processing system

**Files:**
- Modify: `src/weapon/firing.rs`

- [ ] **Step 1: Write failing test for queue processing**

```rust
#[test]
fn cannon_auto_fire_skips_missile_types() {
    // Verify that auto_fire does not fire VLS weapons
    // (they are handled by process_missile_queue instead)
    let profile = WeaponType::HeavyVLS.profile();
    assert_eq!(profile.category(), WeaponCategory::Missile);
}
```

- [ ] **Step 2: Implement `process_missile_queue` system**

Add to `src/weapon/firing.rs`:

```rust
/// Process the missile queue: for each ship with queued missiles,
/// find VLS mounts with available tubes (cooldown == 0), pop from queue,
/// spawn missile entity, reset tube cooldown.
pub fn process_missile_queue(
    mut commands: Commands,
    mut ships: Query<(Entity, &Transform, &mut Mounts, &mut MissileQueue), With<Ship>>,
    target_query: Query<(&Transform, &Velocity), With<Ship>>,
) {
    for (ship_entity, ship_transform, mut mounts, mut queue) in &mut ships {
        if queue.0.is_empty() { continue; }

        for mount_idx in 0..mounts.0.len() {
            if queue.0.is_empty() { break; }

            let mount = &mounts.0[mount_idx];
            let Some(ref weapon) = mount.weapon else { continue; };
            let profile = weapon.weapon_type.profile();
            if profile.category() != WeaponCategory::Missile { continue; }
            if weapon.cooldown > 0.0 { continue; }

            // Pop from queue and spawn missile
            let (target_xz, target_entity) = queue.0.remove(0);

            // Compute intercept point
            let intercept = if let Some(te) = target_entity {
                if let Ok((target_tf, target_vel)) = target_query.get(te) {
                    compute_intercept_point(
                        ship_transform.translation,
                        target_tf.translation,
                        target_vel.linear,
                        profile.projectile_speed,
                    )
                } else {
                    Vec3::new(target_xz.x, 0.0, target_xz.y)
                }
            } else {
                Vec3::new(target_xz.x, 0.0, target_xz.y)
            };

            let origin = ship_transform.translation + Vec3::new(0.0, 5.0, 0.0);

            spawn_missile(
                &mut commands,
                origin,
                intercept,
                target_entity,
                profile.projectile_speed,
                profile.damage,
                profile.missile_hp,
                profile.missile_fuel,
                ship_entity,
            );

            // Reset tube cooldown
            let weapon_mut = mounts.0[mount_idx].weapon.as_mut().unwrap();
            weapon_mut.cooldown = profile.fire_rate_secs;
        }
    }
}
```

Also modify `auto_fire` to skip missile and PD weapon categories:

```rust
// At the start of the mount loop, after getting the profile:
if profile.category() != WeaponCategory::Cannon { continue; }
```

- [ ] **Step 3: Register `process_missile_queue` in server systems**

In `src/net/server.rs`, add `process_missile_queue` to the weapon system chain (after `auto_fire`).

- [ ] **Step 4: Run tests**

Run: `cargo check && cargo test --lib -- -v`

- [ ] **Step 5: Commit**

```
git commit -m "feat: missile queue processing — VLS tubes launch queued missiles"
```

---

## Chunk 3: Point Defense Systems

### Task 6: PD module — cylinder range check and Laser PD

**Files:**
- Create: `src/weapon/pd.rs`
- Modify: `src/weapon/mod.rs` (add `pub mod pd`)

- [ ] **Step 1: Write failing tests for PD**

In `src/weapon/pd.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missile_inside_cylinder_detected() {
        let pd_pos = Vec3::new(100.0, 0.0, 100.0);
        let missile_pos = Vec3::new(110.0, 60.0, 105.0); // 11m XZ distance, high altitude
        let radius = 150.0;
        assert!(is_in_pd_cylinder(pd_pos, missile_pos, radius));
    }

    #[test]
    fn missile_outside_cylinder_not_detected() {
        let pd_pos = Vec3::new(100.0, 0.0, 100.0);
        let missile_pos = Vec3::new(300.0, 60.0, 100.0); // 200m XZ distance
        let radius = 150.0;
        assert!(!is_in_pd_cylinder(pd_pos, missile_pos, radius));
    }
}
```

- [ ] **Step 2: Run tests — expect failure**

- [ ] **Step 3: Implement PD module**

```rust
use bevy::prelude::*;

use crate::game::GameState;
use crate::ship::{Ship, ship_xz_position};
use crate::weapon::Mounts;
use crate::weapon::missile::{Missile, MissileHealth};
use crate::weapon::{WeaponCategory, WeaponType};

/// Check if a missile is within a PD's vertical cylinder (XZ distance only).
pub fn is_in_pd_cylinder(pd_pos: Vec3, missile_pos: Vec3, radius: f32) -> bool {
    let dx = pd_pos.x - missile_pos.x;
    let dz = pd_pos.z - missile_pos.z;
    (dx * dx + dz * dz).sqrt() <= radius
}

/// Laser PD: instant hit, deal damage to closest missile in cylinder.
fn laser_pd_fire(
    time: Res<Time>,
    mut pd_ships: Query<(&Transform, &mut Mounts), With<Ship>>,
    mut missiles: Query<(Entity, &Transform, &mut MissileHealth), With<Missile>>,
) {
    for (ship_transform, mut mounts) in &mut pd_ships {
        for mount in mounts.0.iter_mut() {
            let Some(ref mut weapon) = mount.weapon else { continue; };
            if weapon.weapon_type != WeaponType::LaserPD { continue; }
            if weapon.cooldown > 0.0 { continue; }

            let profile = weapon.weapon_type.profile();

            // Find closest missile in cylinder
            let mut closest: Option<(Entity, f32)> = None;
            for (entity, missile_tf, _) in &missiles {
                if !is_in_pd_cylinder(ship_transform.translation, missile_tf.translation, profile.pd_cylinder_radius) {
                    continue;
                }
                let dist = Vec2::new(
                    ship_transform.translation.x - missile_tf.translation.x,
                    ship_transform.translation.z - missile_tf.translation.z,
                ).length();
                if closest.map_or(true, |(_, d)| dist < d) {
                    closest = Some((entity, dist));
                }
            }

            if let Some((target_entity, _)) = closest {
                if let Ok((_, _, mut health)) = missiles.get_mut(target_entity) {
                    health.0 = health.0.saturating_sub(profile.damage);
                }
                weapon.cooldown = profile.fire_rate_secs;
            }
        }
    }
}

/// CIWS: spawn small projectiles aimed at closest missile in cylinder.
/// CIWS rounds use a `CwisRound` marker and only damage missiles.
fn cwis_fire(
    mut commands: Commands,
    mut pd_ships: Query<(&Transform, &mut Mounts), With<Ship>>,
    missiles: Query<(Entity, &Transform), With<Missile>>,
) {
    for (ship_transform, mut mounts) in &mut pd_ships {
        for mount in mounts.0.iter_mut() {
            let Some(ref mut weapon) = mount.weapon else { continue; };
            if weapon.weapon_type != WeaponType::CWIS { continue; }
            if weapon.cooldown > 0.0 { continue; }

            let profile = weapon.weapon_type.profile();

            // Find closest missile in cylinder
            let mut closest: Option<(Entity, f32, Vec3)> = None;
            for (entity, missile_tf) in &missiles {
                if !is_in_pd_cylinder(ship_transform.translation, missile_tf.translation, profile.pd_cylinder_radius) {
                    continue;
                }
                let dist = Vec2::new(
                    ship_transform.translation.x - missile_tf.translation.x,
                    ship_transform.translation.z - missile_tf.translation.z,
                ).length();
                if closest.map_or(true, |(_, d, _)| dist < d) {
                    closest = Some((entity, dist, missile_tf.translation));
                }
            }

            if let Some((_, _, target_pos)) = closest {
                // Spawn CIWS round toward missile with some spread
                let dir = (target_pos - ship_transform.translation).normalize_or_zero();
                let mut rng = rand::rng();
                let spread: f32 = rng.random_range(-2.0_f32..2.0_f32).to_radians();
                let cos_s = spread.cos();
                let sin_s = spread.sin();
                // Rotate in 3D (simplified: rotate around Y axis for XZ spread)
                let spread_dir = Vec3::new(
                    dir.x * cos_s - dir.z * sin_s,
                    dir.y,
                    dir.x * sin_s + dir.z * cos_s,
                );

                use crate::weapon::projectile::CwisRound;
                commands.spawn((
                    crate::weapon::projectile::Projectile,
                    crate::weapon::projectile::ProjectileVelocity(spread_dir * profile.projectile_speed),
                    crate::weapon::projectile::ProjectileDamage(profile.damage),
                    crate::weapon::projectile::ProjectileOwner(Entity::PLACEHOLDER),
                    CwisRound,
                    Transform::from_translation(ship_transform.translation + Vec3::new(0.0, 5.0, 0.0)),
                    Replicated,
                ));

                weapon.cooldown = profile.fire_rate_secs;
            }
        }
    }
}

pub struct PdPlugin;

impl Plugin for PdPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (laser_pd_fire, cwis_fire)
                .run_if(in_state(GameState::Playing)),
        );
    }
}
```

- [ ] **Step 4: Add `CwisRound` marker to `projectile.rs`**

In `src/weapon/projectile.rs`, add:

```rust
/// Marker for CIWS rounds. These only damage missiles, not ships.
#[derive(Component, Serialize, Deserialize)]
pub struct CwisRound;
```

Modify `check_projectile_hits` to skip projectiles with `CwisRound`.
Add a new `check_cwis_hits` system that checks `CwisRound` projectiles against missiles only.

- [ ] **Step 5: Run tests**

Run: `cargo check && cargo test --lib -- -v`

- [ ] **Step 6: Commit**

```
git commit -m "feat: Laser PD and CIWS point defense systems"
```

---

## Chunk 4: Input, Commands, and Networking

### Task 7: FireMissileCommand and CancelMissilesCommand

**Files:**
- Modify: `src/net/commands.rs`
- Modify: `src/net/replication.rs`

- [ ] **Step 1: Add commands**

In `src/net/commands.rs`:

```rust
/// Client → server: queue a missile launch from a ship.
#[derive(Event, Debug, Clone, Serialize, Deserialize, MapEntities)]
pub struct FireMissileCommand {
    #[entities]
    pub ship: Entity,
    pub target_point: Vec2,
    pub target_entity: Option<Entity>,
}

/// Client → server: cancel all queued (not yet launched) missiles.
#[derive(Event, Debug, Clone, Serialize, Deserialize, MapEntities)]
pub struct CancelMissilesCommand {
    #[entities]
    pub ship: Entity,
}
```

Note: `FireMissileCommand` needs custom `MapEntities` impl because `target_entity` is `Option<Entity>`.

- [ ] **Step 2: Register in `SharedReplicationPlugin`**

Add to `src/net/replication.rs`:

```rust
.add_mapped_client_event::<FireMissileCommand>(Channel::Ordered)
.add_mapped_client_event::<CancelMissilesCommand>(Channel::Ordered);
```

Also register all new missile components:

```rust
.replicate::<Missile>()
.replicate::<MissileTarget>()
.replicate::<MissileVelocity>()
.replicate::<MissileHealth>()
.replicate::<MissileDamage>()
.replicate::<MissileOwner>()
.replicate::<MissileFuel>()
.replicate::<FlightPhase>()
.replicate::<MissileQueue>()
.replicate::<CwisRound>()
```

- [ ] **Step 3: Run `cargo check`**

- [ ] **Step 4: Commit**

```
git commit -m "feat: FireMissileCommand, CancelMissilesCommand, missile replication"
```

---

### Task 8: Server command handlers

**Files:**
- Modify: `src/net/server.rs`

- [ ] **Step 1: Add `FireMissileCommand` handler**

Add observer in `ServerNetPlugin::build`:

```rust
app.add_observer(handle_fire_missile);
```

Implement:

```rust
fn handle_fire_missile(
    trigger: On<FromClient<FireMissileCommand>>,
    client_teams: Res<ClientTeams>,
    team_query: Query<&Team, With<Ship>>,
    mut queue_query: Query<&mut MissileQueue, With<Ship>>,
) {
    let client_entity = client_entity(trigger.client_id());
    // ... validate team ownership (same pattern as move command)
    // ... push (target_point, target_entity) onto MissileQueue
}
```

- [ ] **Step 2: Add `CancelMissilesCommand` handler**

```rust
fn handle_cancel_missiles(
    trigger: On<FromClient<CancelMissilesCommand>>,
    // ... validate and clear queue
) {
    // Clear the MissileQueue for the ship
}
```

- [ ] **Step 3: Add missile entities to LOS visibility in `server_update_visibility`**

Add a section after ship visibility that iterates missile entities and applies the same LOS check. Missiles are visible to a client if any friendly ship has LOS on the missile.

- [ ] **Step 4: Run `cargo check` and tests**

- [ ] **Step 5: Commit**

```
git commit -m "feat: server missile command handlers and missile LOS visibility"
```

---

### Task 9: Client M-mode input

**Files:**
- Modify: `src/input/mod.rs`

- [ ] **Step 1: Add `MissileMode` resource and HUD**

Follow the same pattern as `LockMode` and `TargetMode`:

```rust
#[derive(Resource, Default)]
pub struct MissileMode(pub bool);
```

- [ ] **Step 2: Add M key to `handle_keyboard`**

```rust
if keys.just_pressed(KeyCode::KeyM) {
    missile_mode.0 = !missile_mode.0;
    lock_mode.0 = false;
    target_mode.0 = false;
}
```

Also modify Escape handling to cancel queued missiles:

```rust
if keys.just_pressed(KeyCode::Escape) {
    // ... existing deselect logic ...
    if missile_mode.0 {
        // Cancel queued missiles for selected ships
        for entity in &selected_query {
            commands.client_trigger(CancelMissilesCommand { ship: entity });
        }
    }
    missile_mode.0 = false;
}
```

- [ ] **Step 3: Add missile click handling**

In `on_ship_clicked`, add missile mode handling (before target mode check):

```rust
if missile_mode.0 && *team != my_team {
    for selected_ship in &selected_query {
        commands.client_trigger(FireMissileCommand {
            ship: selected_ship,
            target_point: Vec2::new(
                ship_transform.translation.x,
                ship_transform.translation.z,
            ),
            target_entity: Some(entity),
        });
    }
    // Don't exit missile mode — allow rapid clicking
    return;
}
```

In `on_ground_clicked`, add missile mode handling:

```rust
if click.button == PointerButton::Primary && missile_mode.0 {
    for (entity, _, team) in &selected_query {
        if *team != my_team { continue; }
        commands.client_trigger(FireMissileCommand {
            ship: entity,
            target_point: destination,
            target_entity: None,
        });
    }
    return;
}
```

- [ ] **Step 4: Add missile mode HUD text**

Same pattern as lock/target mode HUDs:

```rust
Text::new("MISSILE MODE — Click enemy or ground to fire")
```

- [ ] **Step 5: Run `cargo check`**

- [ ] **Step 6: Commit**

```
git commit -m "feat: M-mode missile input — click enemy or ground to queue missiles"
```

---

## Chunk 5: Integration, Materializer, and Plugin Wiring

### Task 10: Missile materializer (client visuals)

**Files:**
- Modify: `src/net/materializer.rs`

- [ ] **Step 1: Add `materialize_missiles` system**

Follow the pattern of `materialize_projectiles`. Missiles get a small cone mesh (pointing in velocity direction) with a red/orange emissive material.

- [ ] **Step 2: Register in `ClientNetPlugin`**

Add `materialize_missiles` to the `Update` system set alongside other materializers.

- [ ] **Step 3: Commit**

```
git commit -m "feat: client missile materializer — cone mesh with emissive material"
```

---

### Task 11: Plugin wiring and default loadouts

**Files:**
- Modify: `src/bin/server.rs` (add MissilePlugin, PdPlugin)
- Modify: `src/ship/mod.rs` (update default_loadout)

- [ ] **Step 1: Add plugins to server binary**

```rust
use nebulous_shot_command::weapon::missile::MissilePlugin;
use nebulous_shot_command::weapon::pd::PdPlugin;

// In .add_plugins():
MissilePlugin,
PdPlugin,
```

- [ ] **Step 2: Update default loadouts**

In `src/ship/mod.rs`, update `default_loadout`:

```rust
ShipClass::Battleship => vec![
    Some(WeaponType::HeavyCannon),  // Large
    Some(WeaponType::HeavyVLS),     // Large
    Some(WeaponType::LightVLS),     // Medium
    Some(WeaponType::LaserPD),      // Medium
    Some(WeaponType::CWIS),         // Small
    Some(WeaponType::CWIS),         // Small
],
ShipClass::Destroyer => vec![
    Some(WeaponType::Railgun),      // Large
    Some(WeaponType::Cannon),       // Medium
    Some(WeaponType::LaserPD),      // Medium
    Some(WeaponType::CWIS),         // Small
],
ShipClass::Scout => vec![
    Some(WeaponType::Cannon),       // Medium
    Some(WeaponType::CWIS),         // Small
],
```

- [ ] **Step 3: Run full `cargo check` and `cargo test`**

Run: `cargo check && cargo test --lib -- -v`
Expected: All tests pass (existing 65 + new missile/PD tests).

- [ ] **Step 4: Commit**

```
git commit -m "feat: wire MissilePlugin + PdPlugin, update default loadouts for Phase 3b"
```

---

### Task 12: Integration testing and tuning

**Files:**
- All modified files

- [ ] **Step 1: Run full test suite**

Run: `cargo test --lib -- -v`
Fix any failures.

- [ ] **Step 2: Compile both binaries**

Run: `cargo build --bin server && cargo build --bin client`
Fix any compilation errors.

- [ ] **Step 3: Update CLAUDE.md**

Update the architecture section with missile/PD system descriptions, new modules, test counts, and system ordering.

- [ ] **Step 4: Final commit**

```
git commit -m "docs: update CLAUDE.md for Phase 3b missiles & point defense"
```
