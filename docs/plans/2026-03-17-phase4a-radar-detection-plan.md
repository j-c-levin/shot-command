# Phase 4a: Radar & Detection Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the uniform 400m vision system with a layered radar detection model — mountable radar equipment, SNR-based detection with signature/track thresholds, RWR bearing lines, and RadarContact entities for beyond-visual-range awareness.

**Architecture:** Radar is a new `WeaponCategory::Sensor` in the existing mount system. A new `src/radar/` module owns the SNR calculation, RadarContact entity lifecycle, and RWR logic. The server runs radar detection each frame alongside the existing LOS system, creating/updating/despawning `RadarContact` entities per team. Clients render contacts as gizmos (pulsing circles for signatures, diamond markers for tracks). PD systems are extended to engage radar-tracked missiles.

**Key visibility decisions:**
- `RadarActive` is **server-only** (not replicated) — enemy clients must NOT know if a ship's radar is on. Client reads radar status from ShipSecrets.
- `RwrBearings` lives on **ShipSecrets entities** (team-private), not Ship entities.
- `ContactSourceShip` is **replicated with MapEntities** from the start — clients need it to target tracks.
- RadarContact entities use a dedicated `RadarBit` filter. Both `LosBit` and `RadarBit` must be explicitly set for contacts.

**Tech Stack:** Bevy 0.18, bevy_replicon 0.39 (entity replication + visibility filtering), existing mount/weapon system.

---

## File Structure

| File | Responsibility |
|------|---------------|
| `src/radar/mod.rs` (create) | Components (RadarContact, ContactLevel, ContactTeam, ContactId, ContactSourceShip, RadarActive), RCS constants, SNR pure functions (`compute_snr`, `compute_aspect_factor`), ContactTracker resource, RadarPlugin (server) / RadarClientPlugin (client) |
| `src/radar/contacts.rs` (create) | Server system: `update_radar_contacts` — creates/updates/despawns RadarContact entities per team based on SNR across all team radars. Also tracks missiles/projectiles. `cleanup_stale_contacts` for destroyed ships. |
| `src/radar/rwr.rs` (create) | `RwrBearings` component (on ShipSecrets), `is_in_rwr_range` pure function, server system: `update_rwr_bearings` |
| `src/radar/visuals.rs` (create) | Client gizmo systems: `draw_radar_signature_gizmos` (pulsing circles), `draw_radar_track_gizmos` (diamond markers), `draw_radar_status_gizmos` (blue/grey icon, reads ShipSecrets), `draw_rwr_gizmos` (bearing lines), `draw_tracked_missile_gizmos` (distinctive marker on radar-tracked missiles) |
| `src/weapon/mod.rs` (modify) | Add `SearchRadar`/`NavRadar` to WeaponType enum, `WeaponCategory::Sensor`, profiles, mount sizes |
| `src/ship/mod.rs` (modify) | Add `rcs: f32` to ShipProfile, add RCS values per class |
| `src/fleet/mod.rs` (modify) | Add radar costs to `weapon_cost()` |
| `src/net/commands.rs` (modify) | Add `RadarToggleCommand` |
| `src/net/replication.rs` (modify) | Register RadarContact components, RwrBearings, RadarToggleCommand |
| `src/net/server.rs` (modify) | Add `RadarBit` resource, radar contact visibility filtering, `handle_radar_toggle` observer, sync RadarActive to ShipSecrets, extend missile visibility for radar-tracked |
| `src/input/mod.rs` (modify) | Add R key for radar toggle (uses `selected_query`) |
| `src/weapon/pd.rs` (modify) | Extend PD to engage radar-tracked missiles beyond visual range |
| `src/ui/fleet_builder.rs` (modify) | Add SearchRadar/NavRadar to ALL_WEAPONS constant |
| `src/lib.rs` (modify) | Add `pub mod radar;`, register plugins |

---

## Task 1: Add RCS to ShipProfile

**Files:**
- Modify: `src/ship/mod.rs:63-73` (ShipProfile struct), `src/ship/mod.rs:180-213` (profiles)

- [ ] **Step 1: Write failing tests for RCS values**

Add to the existing `#[cfg(test)]` block in `src/ship/mod.rs`:

```rust
#[test]
fn battleship_rcs_largest() {
    let bb = ShipClass::Battleship.profile();
    let dd = ShipClass::Destroyer.profile();
    let sc = ShipClass::Scout.profile();
    assert!(bb.rcs > dd.rcs);
    assert!(dd.rcs > sc.rcs);
}

#[test]
fn rcs_values_positive() {
    for class in [ShipClass::Battleship, ShipClass::Destroyer, ShipClass::Scout] {
        assert!(class.profile().rcs > 0.0);
        assert!(class.profile().rcs <= 1.0);
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test rcs -- --nocapture`
Expected: FAIL — `rcs` field doesn't exist on ShipProfile

- [ ] **Step 3: Add `rcs` field to ShipProfile and set values**

In `src/ship/mod.rs`, add `pub rcs: f32` to `ShipProfile` struct (after `collision_radius`).

Set values in each `ShipClass::profile()` match arm:
- Battleship: `rcs: 1.0`
- Destroyer: `rcs: 0.5`
- Scout: `rcs: 0.25`

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test rcs -- --nocapture`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/ship/mod.rs
git commit -m "feat: add RCS (radar cross-section) to ShipProfile"
```

---

## Task 2: Add Radar WeaponTypes

**Files:**
- Modify: `src/weapon/mod.rs:42-186` (WeaponType enum, category, profile, mount_size)
- Modify: `src/fleet/mod.rs:32-42` (weapon_cost)
- Modify: `src/ui/fleet_builder.rs` (ALL_WEAPONS constant)

- [ ] **Step 1: Write failing tests for radar weapon properties**

Add to `#[cfg(test)]` in `src/weapon/mod.rs`:

```rust
#[test]
fn search_radar_is_sensor() {
    assert_eq!(WeaponType::SearchRadar.category(), WeaponCategory::Sensor);
}

#[test]
fn nav_radar_is_sensor() {
    assert_eq!(WeaponType::NavRadar.category(), WeaponCategory::Sensor);
}

#[test]
fn search_radar_mount_size_medium() {
    assert_eq!(WeaponType::SearchRadar.mount_size(), MountSize::Medium);
}

#[test]
fn nav_radar_mount_size_small() {
    assert_eq!(WeaponType::NavRadar.mount_size(), MountSize::Small);
}

#[test]
fn search_radar_range_800() {
    let profile = WeaponType::SearchRadar.profile();
    assert_eq!(profile.firing_range, 800.0);
}

#[test]
fn nav_radar_range_500() {
    let profile = WeaponType::NavRadar.profile();
    assert_eq!(profile.firing_range, 500.0);
}
```

Add to `#[cfg(test)]` in `src/fleet/mod.rs`:

```rust
#[test]
fn search_radar_cost() {
    assert_eq!(weapon_cost(WeaponType::SearchRadar), 35);
}

#[test]
fn nav_radar_cost() {
    assert_eq!(weapon_cost(WeaponType::NavRadar), 20);
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test radar -- --nocapture`
Expected: FAIL — `SearchRadar` and `NavRadar` don't exist

- [ ] **Step 3: Add radar variants to WeaponType**

In `src/weapon/mod.rs`:

Add `Sensor` to `WeaponCategory` enum.

Add `SearchRadar` and `NavRadar` to `WeaponType` enum.

Add match arms to `category()`:
```rust
WeaponType::SearchRadar | WeaponType::NavRadar => WeaponCategory::Sensor,
```

Add match arms to `profile()` — radars reuse WeaponProfile but only `firing_range` matters (the rest zeroed):
```rust
WeaponType::SearchRadar => WeaponProfile {
    fire_rate_secs: 0.0,
    burst_count: 0,
    damage: 0,
    firing_range: 800.0,
    projectile_speed: 0.0,
    spread_degrees: 0.0,
    arc: FiringArc::Turret,
    tubes: 0,
    missile_fuel: 0.0,
    pd_cylinder_radius: 0.0,
},
WeaponType::NavRadar => WeaponProfile {
    fire_rate_secs: 0.0,
    burst_count: 0,
    damage: 0,
    firing_range: 500.0,
    projectile_speed: 0.0,
    spread_degrees: 0.0,
    arc: FiringArc::Turret,
    tubes: 0,
    missile_fuel: 0.0,
    pd_cylinder_radius: 0.0,
},
```

Add match arms to `mount_size()`:
```rust
WeaponType::SearchRadar => MountSize::Medium,
WeaponType::NavRadar => MountSize::Small,
```

In `src/fleet/mod.rs`, add to `weapon_cost()`:
```rust
WeaponType::SearchRadar => 35,
WeaponType::NavRadar => 20,
```

- [ ] **Step 4: Verify existing code handles the new category gracefully**

Check `src/weapon/firing.rs` — the `auto_fire` system uses `if weapon.weapon_type.category() != WeaponCategory::Cannon` comparisons (not exhaustive matches). Sensors will be skipped by these `!=` checks, which is correct. Verify no other code assumes only three categories exist.

Check `src/weapon/pd.rs` — PD systems filter by `WeaponCategory::PointDefense`. Sensors are excluded. Correct.

**Update `ALL_WEAPONS` in `src/ui/fleet_builder.rs`** — this is a hardcoded array (~line 37) listing all weapon variants for the fleet builder picker. Add `WeaponType::SearchRadar` and `WeaponType::NavRadar` to the array.

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test radar -- --nocapture`
Expected: PASS

Run: `cargo test` (full suite)
Expected: All 143+ tests pass

- [ ] **Step 6: Commit**

```bash
git add src/weapon/mod.rs src/fleet/mod.rs src/ui/fleet_builder.rs
git commit -m "feat: add SearchRadar and NavRadar weapon types with Sensor category"
```

---

## Task 3: SNR Pure Functions

**Files:**
- Create: `src/radar/mod.rs`

- [ ] **Step 1: Create `src/radar/mod.rs` with module declarations and write failing SNR tests**

```rust
pub mod contacts;
pub mod rwr;
pub mod visuals;

use bevy::ecs::entity::MapEntities;
use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::game::{GameState, Team};

/// Signature threshold — low SNR, fuzzy detection.
pub const SIGNATURE_THRESHOLD: f32 = 0.1;
/// Track threshold — high SNR, precise lock.
pub const TRACK_THRESHOLD: f32 = 0.4;
/// Positional fuzz radius for signature contacts (meters).
pub const SIGNATURE_FUZZ_RADIUS: f32 = 75.0;
/// Small RCS for missiles (detectable by radar).
pub const MISSILE_RCS: f32 = 0.05;
/// Small RCS for projectiles (detectable by radar, smaller than missiles).
pub const PROJECTILE_RCS: f32 = 0.02;

/// Compute aspect factor based on angle between radar bearing and target facing.
/// Broadside (90°) = 1.0, nose-on (0°/180°) = 0.25.
pub fn compute_aspect_factor(radar_bearing: Vec2, target_facing: Vec2) -> f32 {
    todo!()
}

/// Compute SNR (signal-to-noise ratio) for a radar detecting a target.
/// Returns a value where higher = stronger signal.
pub fn compute_snr(
    radar_range: f32,
    distance: f32,
    target_rcs: f32,
    aspect_factor: f32,
) -> f32 {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Aspect factor tests ──

    #[test]
    fn aspect_broadside_is_max() {
        // Radar looking along +X, target facing +Z (perpendicular = broadside)
        let factor = compute_aspect_factor(Vec2::X, Vec2::Y);
        assert!((factor - 1.0).abs() < 0.01);
    }

    #[test]
    fn aspect_nose_on_is_min() {
        // Radar looking along +X, target facing -X (nose toward radar)
        let factor = compute_aspect_factor(Vec2::X, Vec2::NEG_X);
        assert!((factor - 0.25).abs() < 0.01);
    }

    #[test]
    fn aspect_tail_on_is_min() {
        // Radar looking along +X, target facing +X (tail toward radar)
        let factor = compute_aspect_factor(Vec2::X, Vec2::X);
        assert!((factor - 0.25).abs() < 0.01);
    }

    #[test]
    fn aspect_factor_range() {
        // All possible angles should produce values in [0.25, 1.0]
        for angle_deg in (0..360).step_by(10) {
            let angle = (angle_deg as f32).to_radians();
            let target_facing = Vec2::new(angle.cos(), angle.sin());
            let factor = compute_aspect_factor(Vec2::X, target_facing);
            assert!(factor >= 0.24, "factor {factor} below min at {angle_deg}°");
            assert!(factor <= 1.01, "factor {factor} above max at {angle_deg}°");
        }
    }

    // ── SNR tests ──

    #[test]
    fn snr_at_zero_distance_is_high() {
        let snr = compute_snr(800.0, 1.0, 1.0, 1.0);
        assert!(snr > TRACK_THRESHOLD);
    }

    #[test]
    fn snr_decreases_with_distance() {
        let near = compute_snr(800.0, 200.0, 1.0, 1.0);
        let far = compute_snr(800.0, 600.0, 1.0, 1.0);
        assert!(near > far);
    }

    #[test]
    fn snr_increases_with_rcs() {
        let small = compute_snr(800.0, 400.0, 0.25, 1.0);
        let large = compute_snr(800.0, 400.0, 1.0, 1.0);
        assert!(large > small);
    }

    #[test]
    fn snr_increases_with_aspect() {
        let nose = compute_snr(800.0, 400.0, 1.0, 0.25);
        let broadside = compute_snr(800.0, 400.0, 1.0, 1.0);
        assert!(broadside > nose);
    }

    #[test]
    fn battleship_broadside_tracked_at_800() {
        let snr = compute_snr(800.0, 800.0, 1.0, 1.0);
        assert!(snr >= TRACK_THRESHOLD, "BB broadside at max range should be tracked, got {snr}");
    }

    #[test]
    fn scout_nose_on_not_tracked_at_800() {
        let snr = compute_snr(800.0, 800.0, 0.25, 0.25);
        assert!(snr < TRACK_THRESHOLD, "Scout nose-on at max range should not be tracked, got {snr}");
    }

    #[test]
    fn scout_nose_on_signature_at_500() {
        let snr = compute_snr(800.0, 500.0, 0.25, 0.25);
        assert!(snr >= SIGNATURE_THRESHOLD, "Scout nose-on at 500m should be signature, got {snr}");
    }

    #[test]
    fn nav_radar_shorter_range() {
        let search = compute_snr(800.0, 400.0, 0.5, 1.0);
        let nav = compute_snr(500.0, 400.0, 0.5, 1.0);
        assert!(search > nav);
    }

    // ── ContactTracker tests ──

    #[test]
    fn contact_tracker_allocates_sequential_ids() {
        let mut tracker = ContactTracker::default();
        let id1 = tracker.allocate_id(0);
        let id2 = tracker.allocate_id(0);
        assert_eq!(id1.0, 1);
        assert_eq!(id2.0, 2);
    }

    #[test]
    fn contact_tracker_ids_per_team() {
        let mut tracker = ContactTracker::default();
        let team0 = tracker.allocate_id(0);
        let team1 = tracker.allocate_id(1);
        assert_eq!(team0.0, 1); // Each team starts at 1
        assert_eq!(team1.0, 1);
    }
}
```

- [ ] **Step 2: Add `pub mod radar;` to `src/lib.rs`**

- [ ] **Step 3: Run tests to verify they fail**

Run: `cargo test radar -- --nocapture`
Expected: FAIL — `todo!()` panics

- [ ] **Step 4: Implement `compute_aspect_factor` and `compute_snr`**

```rust
pub fn compute_aspect_factor(radar_bearing: Vec2, target_facing: Vec2) -> f32 {
    // The angle between the target's facing and the radar bearing.
    // We want broadside (perpendicular) = 1.0, nose/tail (parallel) = 0.25.
    // Use the cross product magnitude (sin of angle) to get perpendicularity.
    let cross = radar_bearing.x * target_facing.y - radar_bearing.y * target_facing.x;
    let sin_angle = cross.abs().clamp(0.0, 1.0);
    0.25 + 0.75 * sin_angle
}

pub fn compute_snr(
    radar_range: f32,
    distance: f32,
    target_rcs: f32,
    aspect_factor: f32,
) -> f32 {
    if distance <= 0.0 {
        return f32::MAX;
    }
    (radar_range * radar_range / (distance * distance)) * target_rcs * aspect_factor
}
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test radar -- --nocapture`
Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add src/radar/mod.rs src/lib.rs
git commit -m "feat: SNR pure functions for radar detection (aspect factor, signal calculation)"
```

---

## Task 4: RadarContact Components and Replication

**Files:**
- Modify: `src/radar/mod.rs` (add components, ContactTracker resource)
- Create: `src/radar/contacts.rs` (stub)
- Create: `src/radar/rwr.rs` (stub)
- Create: `src/radar/visuals.rs` (stub)
- Modify: `src/net/replication.rs` (register components and command)
- Modify: `src/net/commands.rs` (add RadarToggleCommand)

- [ ] **Step 1: Define components in `src/radar/mod.rs`**

Add below the pure functions (above `#[cfg(test)]`):

```rust
/// Marker for a ship with its radar currently active. SERVER-ONLY — not replicated.
/// Radar state reaches the client via ShipSecrets (RadarActiveSecret component).
#[derive(Component, Clone, Debug)]
pub struct RadarActive;

/// Replicated component on ShipSecrets to tell the owning team if radar is on.
/// Only visible to the owning team (ShipSecrets are team-private).
#[derive(Component, Serialize, Deserialize, Clone, Debug, Default)]
pub struct RadarActiveSecret(pub bool);

/// The level of radar detection achieved on a contact.
#[derive(Component, Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum ContactLevel {
    Signature,
    Track,
}

/// A radar contact entity — replicated to the detecting team.
/// Standalone entity (not a child of the detected ship).
#[derive(Component, Serialize, Deserialize, Clone, Debug)]
pub struct RadarContact;

/// Links a RadarContact back to the actual ship it represents.
/// Replicated with MapEntities so clients can resolve contacts to ship entities
/// for targeting. Note: the target entity may not exist on the client if the ship
/// is outside visual LOS — client code must handle None gracefully.
#[derive(Component, Serialize, Deserialize, Clone, Debug, MapEntities)]
pub struct ContactSourceShip(#[entities] pub Entity);

/// The team that owns this radar contact (i.e., the detecting team).
#[derive(Component, Serialize, Deserialize, Clone, Debug)]
pub struct ContactTeam(pub Team);

/// Stable contact ID for consistent numbering across frames.
#[derive(Component, Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ContactId(pub u8);

/// Whether this contact represents a ship, missile, or projectile.
#[derive(Component, Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum ContactKind {
    Ship,
    Missile,
    Projectile,
}

/// Server resource tracking active radar contacts per team.
/// Maps (detecting_team_id, source_entity) → contact_entity.
#[derive(Resource, Default, Debug)]
pub struct ContactTracker {
    pub contacts: HashMap<(u8, Entity), Entity>,
    pub next_id: HashMap<u8, u8>,
}

impl ContactTracker {
    pub fn allocate_id(&mut self, team_id: u8) -> ContactId {
        let id = self.next_id.entry(team_id).or_insert(1);
        let contact_id = ContactId(*id);
        *id = id.wrapping_add(1).max(1);
        contact_id
    }
}
```

- [ ] **Step 2: Create stub files for submodules**

Create `src/radar/contacts.rs`:
```rust
//! Server system: creates/updates/despawns RadarContact entities based on SNR.
```

Create `src/radar/rwr.rs`:
```rust
//! RWR (Radar Warning Receiver) bearing detection.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// RWR bearing lines for a ship — directions toward enemy radar sources.
/// Lives on ShipSecrets entities (team-private). Only ships with radar hardware get this.
#[derive(Component, Serialize, Deserialize, Clone, Debug, Default)]
pub struct RwrBearings(pub Vec<Vec2>);

/// Returns true if target_pos is within radar_range of radar_pos.
pub fn is_in_rwr_range(radar_pos: Vec2, radar_range: f32, target_pos: Vec2) -> bool {
    radar_pos.distance(target_pos) <= radar_range
}
```

Create `src/radar/visuals.rs`:
```rust
//! Client gizmo rendering for radar contacts and radar status indicators.
```

- [ ] **Step 3: Add RadarToggleCommand to `src/net/commands.rs`**

```rust
#[derive(Event, Debug, Clone, Serialize, Deserialize, MapEntities)]
pub struct RadarToggleCommand {
    #[entities]
    pub ship: Entity,
}
```

- [ ] **Step 4: Register in `src/net/replication.rs`**

Add imports for all new components and command.

Add to replicated components section (after ShipSecrets group):
```rust
// Radar components (on ShipSecrets)
app.replicate::<RadarActiveSecret>()
    .replicate::<RwrBearings>();

// Radar contact entity components
app.replicate::<RadarContact>()
    .replicate::<ContactLevel>()
    .replicate::<ContactTeam>()
    .replicate::<ContactId>()
    .replicate::<ContactSourceShip>()
    .replicate::<ContactKind>();
```

Add to client→server triggers:
```rust
.add_mapped_client_event::<RadarToggleCommand>(Channel::Ordered)
```

- [ ] **Step 5: Verify compilation**

Run: `cargo check`
Expected: Compiles (stub modules, no systems yet)

- [ ] **Step 6: Commit**

```bash
git add src/radar/ src/net/commands.rs src/net/replication.rs
git commit -m "feat: RadarContact components, RadarToggleCommand, replication registration"
```

---

## Task 5: Radar Toggle Command (Server + Input)

**Files:**
- Modify: `src/net/server.rs` (handle_radar_toggle observer, sync radar state to ShipSecrets)
- Modify: `src/input/mod.rs` (R key handler)

- [ ] **Step 1: Add handle_radar_toggle observer in `src/net/server.rs`**

Add imports:
```rust
use crate::radar::RadarActive;
use crate::net::commands::RadarToggleCommand;
use crate::weapon::WeaponCategory;
```

Add observer registration in `ServerNetPlugin::build`:
```rust
app.add_observer(handle_radar_toggle);
```

Implement:
```rust
fn handle_radar_toggle(
    trigger: On<FromClient<RadarToggleCommand>>,
    mut commands: Commands,
    client_teams: Res<ClientTeams>,
    team_query: Query<&Team, With<Ship>>,
    radar_query: Query<Option<&RadarActive>>,
    mounts_query: Query<&Mounts>,
) {
    let from = trigger.event();
    let cmd = &from.message;

    let Some(_) = validate_ownership(
        from.client_id,
        cmd.ship,
        &client_teams,
        &team_query,
        "RadarToggleCommand",
    ) else {
        return;
    };

    // Check ship has a radar equipped
    let Ok(mounts) = mounts_query.get(cmd.ship) else {
        return;
    };
    let has_radar = mounts.0.iter().any(|m| {
        m.weapon
            .as_ref()
            .is_some_and(|w| w.weapon_type.category() == WeaponCategory::Sensor)
    });
    if !has_radar {
        info!("RadarToggleCommand rejected: ship {:?} has no radar", cmd.ship);
        return;
    }

    // Toggle RadarActive marker (server-only component)
    if let Ok(Some(_)) = radar_query.get(cmd.ship) {
        commands.entity(cmd.ship).remove::<RadarActive>();
        info!("Radar OFF for ship {:?}", cmd.ship);
    } else {
        commands.entity(cmd.ship).insert(RadarActive);
        info!("Radar ON for ship {:?}", cmd.ship);
    }
}
```

- [ ] **Step 2: Sync RadarActive to ShipSecrets**

In the existing `sync_ship_secrets` system in `src/net/server.rs`, add sync for `RadarActiveSecret`:

For each ship that has a ShipSecrets entity, set `RadarActiveSecret(ship.has::<RadarActive>())` on the secrets entity. This follows the same pattern as WaypointQueue/FacingTarget sync.

Add `RadarActiveSecret` to the ShipSecrets entity spawn in `spawn_server_ship` (default `RadarActiveSecret(false)`).

- [ ] **Step 3: Add R key handler in `src/input/mod.rs`**

Add import: `use crate::net::commands::RadarToggleCommand;`

In the `handle_keyboard` system, add R key handler. The R key does NOT enter a mode — it immediately toggles radar for all selected ships, following the same pattern as the S key (stop command):

```rust
if keyboard.just_pressed(KeyCode::KeyR) {
    for (entity, _) in &selected_query {
        commands.client_trigger(RadarToggleCommand { ship: entity });
    }
}
```

Use the existing `selected_query: Query<(Entity, &Transform), With<Selected>>` parameter, NOT the general ship query.

- [ ] **Step 4: Verify compilation and run tests**

Run: `cargo check && cargo test`
Expected: Compiles, all tests pass

- [ ] **Step 5: Commit**

```bash
git add src/net/server.rs src/input/mod.rs src/ship/mod.rs
git commit -m "feat: R key radar toggle with server-authoritative validation, synced via ShipSecrets"
```

---

## Task 6: Server Radar Detection System

**Files:**
- Modify: `src/radar/contacts.rs` (implement update_radar_contacts)
- Modify: `src/radar/mod.rs` (RadarPlugin)

- [ ] **Step 1: Write integration-style tests for detection scenarios**

Add to `#[cfg(test)]` in `src/radar/mod.rs`:

```rust
#[test]
fn battleship_detected_at_600m_broadside() {
    let aspect = compute_aspect_factor(Vec2::X, Vec2::Y);
    let snr = compute_snr(800.0, 600.0, 1.0, aspect);
    assert!(snr >= TRACK_THRESHOLD, "BB broadside at 600m should be tracked, got {snr}");
}

#[test]
fn scout_not_detected_at_700m_nose_on() {
    let aspect = compute_aspect_factor(Vec2::X, Vec2::NEG_X);
    let snr = compute_snr(800.0, 700.0, 0.25, aspect);
    assert!(snr < TRACK_THRESHOLD, "Scout nose-on at 700m should not be tracked, got {snr}");
}

#[test]
fn nav_radar_tracks_destroyer_at_300m() {
    let aspect = compute_aspect_factor(Vec2::X, Vec2::Y);
    let snr = compute_snr(500.0, 300.0, 0.5, aspect);
    assert!(snr >= TRACK_THRESHOLD, "DD broadside at 300m with nav radar should be tracked, got {snr}");
}

#[test]
fn missile_detected_by_radar_at_close_range() {
    // Missile (RCS 0.05) at 200m from 800m radar, broadside
    let snr = compute_snr(800.0, 200.0, MISSILE_RCS, 1.0);
    assert!(snr >= SIGNATURE_THRESHOLD, "Missile at 200m should be at least signature, got {snr}");
}
```

- [ ] **Step 2: Run tests — should pass (pure function tests)**

Run: `cargo test radar -- --nocapture`
Expected: PASS

- [ ] **Step 3: Implement `update_radar_contacts` in `src/radar/contacts.rs`**

This system:
1. Iterates all ships with `RadarActive`, gets their best radar range from mounts
2. For each active radar, checks all enemy ships AND enemy missiles/projectiles
3. Computes SNR for each radar-target pair (aspect factor only for ships, 1.0 for missiles/projectiles)
4. Takes the best SNR across all team radars for each target (team-shared tracks)
5. Creates/updates/despawns `RadarContact` entities via `ContactTracker`
6. Signature contacts get fuzzed positions; track contacts get precise positions
7. Asteroids block radar LOS (use `crate::fog::ray_blocked_by_asteroid`)

The `ray_blocked_by_asteroid` function in `src/fog/mod.rs` is already `pub`.

Key query parameters:
```rust
pub fn update_radar_contacts(
    mut commands: Commands,
    mut tracker: ResMut<ContactTracker>,
    radar_ships: Query<(&Transform, &Team, &Mounts), (With<Ship>, With<RadarActive>)>,
    all_ships: Query<(Entity, &Transform, &ShipClass, &Team), With<Ship>>,
    missile_query: Query<(Entity, &Transform, &MissileOwner), With<Missile>>,
    projectile_query: Query<(Entity, &Transform, &ProjectileOwner), With<Projectile>>,
    existing_contacts: Query<Entity, With<RadarContact>>,
    asteroid_query: Query<(&Transform, &AsteroidSize), With<Asteroid>>,
)
```

For missiles and projectiles: use `MISSILE_RCS` / `PROJECTILE_RCS` constants, aspect factor = 1.0 (small objects, no meaningful aspect angle), and `ContactKind::Missile` / `ContactKind::Projectile`.

Determine the missile/projectile's team from its owner entity (look up the owner's Team component via the `MissileOwner.0` / `ProjectileOwner.0` entity). Skip friendly missiles/projectiles.

**Important:** All spawned RadarContact entities MUST include the `bevy_replicon::prelude::Replicated` component alongside their other components, otherwise they will not replicate to clients.

- [ ] **Step 4: Add `cleanup_stale_contacts` system**

When a source ship is destroyed/despawned, its contacts become stale. Add a cleanup system:

```rust
pub fn cleanup_stale_contacts(
    mut commands: Commands,
    mut tracker: ResMut<ContactTracker>,
    entities: &Entities,
) {
    let stale: Vec<(u8, Entity)> = tracker.contacts.keys()
        .filter(|(_, source)| !entities.contains(*source))
        .cloned()
        .collect();

    for key in stale {
        if let Some(contact_entity) = tracker.contacts.remove(&key) {
            if let Some(entity_commands) = commands.get_entity(contact_entity) {
                entity_commands.despawn();
            }
        }
    }
}
```

- [ ] **Step 5: Add RadarPlugin to `src/radar/mod.rs`**

```rust
pub struct RadarPlugin;

impl Plugin for RadarPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ContactTracker>();
        app.add_systems(
            Update,
            (
                contacts::update_radar_contacts,
                contacts::cleanup_stale_contacts,
            )
                .chain()
                .run_if(in_state(GameState::Playing)),
        );
    }
}
```

- [ ] **Step 6: Register RadarPlugin on the server**

Add `RadarPlugin` to the server's plugin list (in `src/bin/server.rs` or via `ServerNetPlugin`).

- [ ] **Step 7: Verify compilation and run tests**

Run: `cargo check && cargo test`
Expected: Compiles, all tests pass

- [ ] **Step 8: Commit**

```bash
git add src/radar/ src/lib.rs src/bin/server.rs
git commit -m "feat: server radar detection system — SNR-based contacts for ships, missiles, projectiles"
```

---

## Task 7: Radar Contact Visibility Filtering

**Files:**
- Modify: `src/net/server.rs` (RadarBit resource, radar contact visibility)

- [ ] **Step 1: Add RadarBit resource to `src/net/server.rs`**

Follow the same pattern as LosBit:

```rust
#[derive(Resource, Deref)]
pub struct RadarBit(FilterBit);

impl FromWorld for RadarBit {
    fn from_world(world: &mut World) -> Self {
        let bit = world.resource_scope(|world, mut filter_registry: Mut<FilterRegistry>| {
            world.resource_scope(|world, mut registry: Mut<ReplicationRegistry>| {
                filter_registry.register_scope::<Entity>(world, &mut registry)
            })
        });
        Self(bit)
    }
}
```

Init in `ServerNetPlugin::build`:
```rust
app.init_resource::<RadarBit>();
```

- [ ] **Step 2: Add radar contact visibility to `server_update_visibility`**

Add `radar_bit: Res<RadarBit>` and `contact_query: Query<(Entity, &ContactTeam), With<RadarContact>>` as parameters to `server_update_visibility`.

After the existing ship/secrets/missile visibility blocks, add:

```rust
// RadarContact visibility: only visible to the detecting team.
// Must set BOTH los_bit and radar_bit — replicon uses OR across filter bits,
// and unset bits default to invisible for scoped entities.
for (contact_entity, contact_team) in &contact_query {
    let visible = contact_team.0 == *client_team;
    client_visibility.set(contact_entity, **los_bit, visible);
    client_visibility.set(contact_entity, **radar_bit, visible);
}
```

- [ ] **Step 3: Extend missile visibility to include radar-tracked**

In the missile visibility section of `server_update_visibility`, change the visibility check for enemy missiles from just visual LOS to `visual_los OR radar_tracked`:

```rust
let visible = if is_friendly {
    true
} else {
    let missile_pos = Vec2::new(
        missile_transform.translation.x,
        missile_transform.translation.z,
    );
    // Visual LOS check (existing)
    let visual_los = all_ships.iter().any(
        |&(_, friendly_pos, friendly_range, ship_team)| {
            ship_team == *client_team
                && is_in_los(friendly_pos, missile_pos, friendly_range, &asteroids)
        },
    );
    // Radar tracking check (new): any friendly radar can see this missile
    let radar_tracked = radar_ships.iter().any(
        |(radar_transform, radar_team, radar_mounts)| {
            if radar_team.0 != client_team.0 { return false; }
            let radar_pos = ship_xz_position(radar_transform);
            let radar_range = radar_mounts.0.iter()
                .filter_map(|m| m.weapon.as_ref())
                .filter(|w| w.weapon_type.category() == WeaponCategory::Sensor)
                .map(|w| w.weapon_type.profile().firing_range)
                .fold(0.0_f32, f32::max);
            radar_pos.distance(missile_pos) <= radar_range
        },
    );
    visual_los || radar_tracked
};
```

This requires adding the `radar_ships` query (same as used in contacts system) as a parameter to `server_update_visibility`.

- [ ] **Step 4: Verify compilation**

Run: `cargo check`
Expected: Compiles

- [ ] **Step 5: Commit**

```bash
git add src/net/server.rs
git commit -m "feat: radar contact visibility filtering and radar-tracked missile visibility"
```

---

## Task 8: RWR System

**Files:**
- Modify: `src/radar/rwr.rs` (server system)
- Modify: `src/radar/mod.rs` (register in RadarPlugin, add tests)

- [ ] **Step 1: Write tests for RWR detection**

Add to `#[cfg(test)]` in `src/radar/mod.rs`:

```rust
#[test]
fn rwr_detects_radar_within_range() {
    let radar_pos = Vec2::ZERO;
    let radar_range = 800.0;
    let target_pos = Vec2::new(600.0, 0.0);
    assert!(rwr::is_in_rwr_range(radar_pos, radar_range, target_pos));
}

#[test]
fn rwr_no_detection_outside_range() {
    let radar_pos = Vec2::ZERO;
    let radar_range = 800.0;
    let target_pos = Vec2::new(900.0, 0.0);
    assert!(!rwr::is_in_rwr_range(radar_pos, radar_range, target_pos));
}

#[test]
fn rwr_exact_boundary() {
    let radar_pos = Vec2::ZERO;
    let radar_range = 800.0;
    let target_pos = Vec2::new(800.0, 0.0);
    assert!(rwr::is_in_rwr_range(radar_pos, radar_range, target_pos));
}
```

- [ ] **Step 2: Run tests to verify they pass**

Run: `cargo test rwr -- --nocapture`
Expected: PASS (pure function already implemented in Task 4)

- [ ] **Step 3: Implement `update_rwr_bearings` server system in `src/radar/rwr.rs`**

```rust
use crate::game::Team;
use crate::radar::RadarActive;
use crate::ship::{Ship, ShipSecrets, ShipSecretsOwner, ship_xz_position};
use crate::weapon::{Mounts, WeaponCategory};

/// Server system: for each ship with active radar, check which enemy ships
/// (that have radar hardware = RWR capability) are within range.
/// Updates RwrBearings on the target's ShipSecrets entity.
pub fn update_rwr_bearings(
    radar_ships: Query<(&Transform, &Team, &Mounts), (With<Ship>, With<RadarActive>)>,
    all_ships: Query<(Entity, &Transform, &Team, &Mounts), With<Ship>>,
    mut secrets_query: Query<(&ShipSecretsOwner, &mut RwrBearings), With<ShipSecrets>>,
) {
    // Clear all bearings
    for (_, mut bearings) in &mut secrets_query {
        bearings.0.clear();
    }

    // For each active radar, find enemy ships with RWR (= have radar hardware) in range
    for (radar_transform, radar_team, radar_mounts) in &radar_ships {
        let radar_range = radar_mounts.0.iter()
            .filter_map(|m| m.weapon.as_ref())
            .filter(|w| w.weapon_type.category() == WeaponCategory::Sensor)
            .map(|w| w.weapon_type.profile().firing_range)
            .fold(0.0_f32, f32::max);

        let radar_pos = ship_xz_position(radar_transform);

        for (target_entity, target_transform, target_team, target_mounts) in &all_ships {
            if target_team.0 == radar_team.0 {
                continue; // Skip friendlies
            }

            // Target must have radar hardware for RWR to work
            let has_radar_hw = target_mounts.0.iter().any(|m| {
                m.weapon.as_ref().is_some_and(|w| w.weapon_type.category() == WeaponCategory::Sensor)
            });
            if !has_radar_hw {
                continue;
            }

            let target_pos = ship_xz_position(target_transform);
            if !is_in_rwr_range(radar_pos, radar_range, target_pos) {
                continue;
            }

            // Add bearing to the target's ShipSecrets
            let bearing = (radar_pos - target_pos).normalize_or_zero();
            for (owner, mut bearings) in &mut secrets_query {
                if owner.0 == target_entity {
                    bearings.0.push(bearing);
                }
            }
        }
    }
}
```

- [ ] **Step 4: Add RwrBearings to ShipSecrets entity spawn**

In `src/ship/mod.rs` `spawn_server_ship`: when spawning the ShipSecrets entity, add `RwrBearings::default()` if the ship has radar hardware (check mounts). Always safe to add it — if the ship has no radar, the bearings list stays empty.

- [ ] **Step 5: Register RwrBearings replication and system**

`RwrBearings` replication was already registered in Task 4.

Add `update_rwr_bearings` to `RadarPlugin::build`:
```rust
app.add_systems(
    Update,
    (
        contacts::update_radar_contacts,
        contacts::cleanup_stale_contacts,
        rwr::update_rwr_bearings,
    )
        .chain()
        .run_if(in_state(GameState::Playing)),
);
```

- [ ] **Step 6: Verify compilation and run tests**

Run: `cargo check && cargo test`
Expected: Compiles, all tests pass

- [ ] **Step 7: Commit**

```bash
git add src/radar/ src/ship/mod.rs
git commit -m "feat: RWR bearing detection — enemy radar emissions produce directional warnings via ShipSecrets"
```

---

## Task 9: Client Radar Visuals (Gizmos)

**Files:**
- Modify: `src/radar/visuals.rs` (gizmo rendering systems)
- Modify: `src/radar/mod.rs` (register client plugin)

- [ ] **Step 1: Implement radar status gizmo (blue/grey on own ships)**

In `src/radar/visuals.rs`:

```rust
use bevy::prelude::*;

use crate::game::Team;
use crate::net::LocalTeam;
use crate::radar::{RadarActiveSecret, RadarContact, ContactLevel, ContactTeam, ContactId, ContactKind};
use crate::radar::rwr::RwrBearings;
use crate::ship::{Ship, ShipSecrets, ShipSecretsOwner, ship_xz_position};
use crate::weapon::{Mounts, WeaponCategory};

/// Draw a small circle above own ships indicating radar status.
/// Reads from ShipSecrets (RadarActiveSecret) — team-private, no info leak.
pub fn draw_radar_status_gizmos(
    mut gizmos: Gizmos,
    local_team: Res<LocalTeam>,
    ships: Query<(Entity, &Transform, &Team, &Mounts), With<Ship>>,
    secrets: Query<(&ShipSecretsOwner, &RadarActiveSecret), With<ShipSecrets>>,
) {
    let Some(my_team) = local_team.0 else { return; };
    for (ship_entity, transform, team, mounts) in &ships {
        if *team != my_team {
            continue;
        }

        let has_radar = mounts.0.iter().any(|m| {
            m.weapon.as_ref().is_some_and(|w| w.weapon_type.category() == WeaponCategory::Sensor)
        });
        if !has_radar {
            continue;
        }

        // Find radar active state from ShipSecrets
        let is_active = secrets.iter()
            .find(|(owner, _)| owner.0 == ship_entity)
            .map(|(_, active)| active.0)
            .unwrap_or(false);

        let pos = transform.translation + Vec3::Y * 15.0;
        let color = if is_active {
            Color::srgb(0.2, 0.5, 1.0) // Blue
        } else {
            Color::srgb(0.4, 0.4, 0.4) // Grey
        };

        gizmos.circle(
            Isometry3d::new(pos, Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
            3.0,
            color,
        );
    }
}
```

- [ ] **Step 2: Implement radar signature gizmo (pulsing circle)**

```rust
/// Draw pulsing circles for radar signature contacts.
pub fn draw_radar_signature_gizmos(
    mut gizmos: Gizmos,
    local_team: Res<LocalTeam>,
    time: Res<Time>,
    contacts: Query<(&Transform, &ContactLevel, &ContactTeam, &ContactKind), With<RadarContact>>,
) {
    let Some(my_team) = local_team.0 else { return; };
    for (transform, level, contact_team, kind) in &contacts {
        if *level != ContactLevel::Signature || contact_team.0 != my_team {
            continue;
        }
        if *kind != ContactKind::Ship {
            continue; // Only show signature gizmos for ships
        }

        let pos = transform.translation;
        let pulse = 0.7 + 0.3 * (time.elapsed_secs() * 2.0).sin();
        let radius = 20.0 * pulse;
        let color = Color::srgba(1.0, 0.5, 0.0, 0.4 * pulse);

        gizmos.circle(
            Isometry3d::new(pos, Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
            radius,
            color,
        );
    }
}
```

- [ ] **Step 3: Implement radar track gizmo (diamond marker)**

```rust
/// Draw diamond markers for radar track contacts (ships only).
pub fn draw_radar_track_gizmos(
    mut gizmos: Gizmos,
    local_team: Res<LocalTeam>,
    contacts: Query<(&Transform, &ContactLevel, &ContactTeam, &ContactKind), With<RadarContact>>,
) {
    let Some(my_team) = local_team.0 else { return; };
    for (transform, level, contact_team, kind) in &contacts {
        if *level != ContactLevel::Track || contact_team.0 != my_team {
            continue;
        }
        if *kind != ContactKind::Ship {
            continue;
        }

        let pos = transform.translation;
        let color = Color::srgb(1.0, 0.2, 0.2); // Red

        // Diamond shape using 4 lines
        let size = 5.0;
        let top = pos + Vec3::Z * size;
        let bottom = pos - Vec3::Z * size;
        let left = pos - Vec3::X * size;
        let right = pos + Vec3::X * size;
        gizmos.line(top, right, color);
        gizmos.line(right, bottom, color);
        gizmos.line(bottom, left, color);
        gizmos.line(left, top, color);
    }
}
```

- [ ] **Step 4: Implement tracked missile gizmo**

```rust
/// Draw distinctive markers for radar-tracked missiles.
pub fn draw_tracked_missile_gizmos(
    mut gizmos: Gizmos,
    local_team: Res<LocalTeam>,
    contacts: Query<(&Transform, &ContactLevel, &ContactTeam, &ContactKind), With<RadarContact>>,
) {
    let Some(my_team) = local_team.0 else { return; };
    for (transform, level, contact_team, kind) in &contacts {
        if contact_team.0 != my_team {
            continue;
        }
        if *kind != ContactKind::Missile {
            continue;
        }
        // Only show tracked missiles (signature missiles too faint to display)
        if *level != ContactLevel::Track {
            continue;
        }

        let pos = transform.translation;
        let color = Color::srgb(1.0, 0.4, 0.0); // Orange

        // Small X marker
        let size = 2.5;
        gizmos.line(pos + Vec3::new(-size, 0.0, -size), pos + Vec3::new(size, 0.0, size), color);
        gizmos.line(pos + Vec3::new(-size, 0.0, size), pos + Vec3::new(size, 0.0, -size), color);
    }
}
```

- [ ] **Step 5: Implement RWR bearing gizmo**

```rust
/// Draw RWR bearing lines from own ships (reads from ShipSecrets).
pub fn draw_rwr_gizmos(
    mut gizmos: Gizmos,
    local_team: Res<LocalTeam>,
    ships: Query<(Entity, &Transform, &Team), With<Ship>>,
    secrets: Query<(&ShipSecretsOwner, &RwrBearings), With<ShipSecrets>>,
) {
    let Some(my_team) = local_team.0 else { return; };
    for (ship_entity, transform, team) in &ships {
        if *team != my_team {
            continue;
        }

        // Find RWR bearings from this ship's secrets
        let Some((_, bearings)) = secrets.iter().find(|(owner, _)| owner.0 == ship_entity) else {
            continue;
        };

        let ship_pos = transform.translation;
        let color = Color::srgb(1.0, 1.0, 0.0); // Yellow

        for bearing in &bearings.0 {
            let end = ship_pos + Vec3::new(bearing.x, 0.0, bearing.y) * 100.0;
            gizmos.line(ship_pos, end, color);
        }
    }
}
```

- [ ] **Step 6: Register RadarClientPlugin**

Add to `src/radar/mod.rs`:

```rust
pub struct RadarClientPlugin;

impl Plugin for RadarClientPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                visuals::draw_radar_status_gizmos,
                visuals::draw_radar_signature_gizmos,
                visuals::draw_radar_track_gizmos,
                visuals::draw_tracked_missile_gizmos,
                visuals::draw_rwr_gizmos,
            )
                .run_if(in_state(GameState::Playing)),
        );
    }
}
```

Add `RadarClientPlugin` to the client binary's plugin list in `src/bin/client.rs`.

- [ ] **Step 7: Verify compilation**

Run: `cargo check`
Expected: Compiles

- [ ] **Step 8: Commit**

```bash
git add src/radar/ src/bin/client.rs
git commit -m "feat: client radar gizmos — status, signature pulse, track diamond, tracked missiles, RWR bearings"
```

---

## Task 10: PD Radar Integration

**Files:**
- Modify: `src/weapon/pd.rs` (extend PD to engage radar-tracked missiles)

- [ ] **Step 1: Understand current PD system structure**

Read `src/weapon/pd.rs`. The `laser_pd_fire` and `cwis_fire` systems iterate missiles and check if they're within PD range. Currently on the server, the server can see ALL entities — the issue is not query visibility but rather the game logic check.

The change: PD systems should check whether the defending ship has radar AND the missile is within radar range, in addition to the existing visual LOS check. Since the server sees all entities already, the logic is:

```
can_engage = missile_in_pd_range AND (missile_in_visual_los OR (ship_has_radar_active AND missile_in_radar_range))
```

- [ ] **Step 2: Add radar-awareness to PD systems**

In both `laser_pd_fire` and `cwis_fire`, add `RadarActive` and `Mounts` to the ship query. When checking if a ship can engage a missile:

```rust
// Existing: check if missile is in visual LOS (400m)
let in_visual_los = ship_pos.distance(missile_pos) <= ship_class.profile().vision_range;

// New: check if missile is radar-tracked
let radar_tracked = if ship_query.get_component::<RadarActive>(ship_entity).is_ok() {
    let radar_range = mounts.0.iter()
        .filter_map(|m| m.weapon.as_ref())
        .filter(|w| w.weapon_type.category() == WeaponCategory::Sensor)
        .map(|w| w.weapon_type.profile().firing_range)
        .fold(0.0_f32, f32::max);
    ship_pos.distance(missile_pos) <= radar_range
} else {
    false
};

if !in_visual_los && !radar_tracked {
    continue; // Can't see the missile, can't engage
}
```

- [ ] **Step 3: Verify compilation and run tests**

Run: `cargo check && cargo test`
Expected: Compiles, all tests pass

- [ ] **Step 4: Commit**

```bash
git add src/weapon/pd.rs
git commit -m "feat: PD engages radar-tracked missiles beyond visual range"
```

---

## Task 11: Integration with K/M Mode Targeting

**Files:**
- Modify: `src/input/mod.rs` (K/M mode reads radar tracks for targeting)

- [ ] **Step 1: Extend enemy numbering to include radar tracks**

The existing `EnemyNumbers` resource assigns 1-9 to visible enemy ships in K/M mode. Extend it to also include radar track contacts:

1. Query for `RadarContact` entities with `ContactLevel::Track`, `ContactKind::Ship`, and `ContactTeam` matching local team
2. For each track, read `ContactSourceShip` to get the underlying entity
3. If the source entity is already numbered (visible in LOS), skip it (avoid double-numbering)
4. Otherwise, assign the next available number

The `ContactSourceShip` entity may not exist on the client (outside visual LOS). In that case, `bevy_replicon` maps it to `Entity::PLACEHOLDER` or the entity simply doesn't exist. The client should use the `RadarContact` entity itself for display but the `ContactSourceShip` entity for targeting commands.

If `ContactSourceShip.0` resolves to a valid entity on the client, use it for `TargetCommand`. If it doesn't (entity not present), still display the numbered track but skip targeting — the player can see the contact but can't lock onto a ghost entity. (Alternatively, add a `TargetContactCommand` that references the contact entity and the server resolves it — this is cleaner.)

**Recommended approach:** Add a `TargetByContactCommand` to `src/net/commands.rs`:

```rust
#[derive(Event, Debug, Clone, Serialize, Deserialize, MapEntities)]
pub struct TargetByContactCommand {
    #[entities]
    pub ship: Entity,
    #[entities]
    pub contact: Entity,
}
```

Server handles it by looking up `ContactSourceShip` on the contact entity and converting to a standard target designation.

- [ ] **Step 2: Register `TargetByContactCommand`**

Add to `src/net/replication.rs`:
```rust
.add_mapped_client_event::<TargetByContactCommand>(Channel::Ordered)
```

Add handler in `src/net/server.rs`:
```rust
fn handle_target_by_contact(
    trigger: On<FromClient<TargetByContactCommand>>,
    mut commands: Commands,
    client_teams: Res<ClientTeams>,
    team_query: Query<&Team, With<Ship>>,
    contact_query: Query<&ContactSourceShip, With<RadarContact>>,
) {
    let from = trigger.event();
    let cmd = &from.message;

    let Some(_) = validate_ownership(
        from.client_id,
        cmd.ship,
        &client_teams,
        &team_query,
        "TargetByContactCommand",
    ) else {
        return;
    };

    // Resolve contact to source ship
    let Ok(source) = contact_query.get(cmd.contact) else {
        return;
    };

    commands.entity(cmd.ship).insert(TargetDesignation(source.0));
}
```

- [ ] **Step 3: Update K mode number key handler**

In the K mode number key handler, when a target is a radar track (not visually present), send `TargetByContactCommand` instead of `TargetCommand`.

- [ ] **Step 4: Verify compilation and run tests**

Run: `cargo check && cargo test`
Expected: Compiles, all tests pass

- [ ] **Step 5: Commit**

```bash
git add src/input/mod.rs src/net/commands.rs src/net/replication.rs src/net/server.rs
git commit -m "feat: K/M mode targeting integrates with radar tracks via TargetByContactCommand"
```

---

## Task 12: Update Default Loadouts and Fleet Builder

**Files:**
- Modify: `src/ship/mod.rs` (update default loadouts)

- [ ] **Step 1: Update default loadouts to include radar**

Give at least one ship per default fleet a radar for testing. Suggested: give the Destroyer a SearchRadar by replacing one of its medium weapons:

```rust
ShipClass::Destroyer => vec![
    Some(WeaponType::Railgun),        // Large
    Some(WeaponType::SearchRadar),    // Medium (was Cannon)
    Some(WeaponType::LaserPD),        // Medium
    Some(WeaponType::CWIS),           // Small
],
```

This means the default Destroyer is the fleet's radar ship — thematic and functional.

- [ ] **Step 2: Update tests that reference default Destroyer loadout**

The test `destroyer_has_four_mounts` in `src/ship/mod.rs` (~line 1082) asserts the second mount weapon is `WeaponType::Cannon`. Update it to expect `WeaponType::SearchRadar`.

- [ ] **Step 3: Run full test suite**

Run: `cargo test`
Expected: All tests pass

- [ ] **Step 3: Commit**

```bash
git add src/ship/mod.rs
git commit -m "feat: default Destroyer loadout includes SearchRadar"
```

---

## Task 13: Manual Playtest and Cleanup

- [ ] **Step 1: Run full test suite**

Run: `cargo test`
Expected: All tests pass

- [ ] **Step 2: Fix any warnings**

Run: `cargo check 2>&1`
Fix unused imports, dead code warnings.

- [ ] **Step 3: Verify test count increased**

Run: `cargo test 2>&1 | tail -1`
Expected: Test count ~165+ (was 143, added ~22 new radar/RWR tests)

- [ ] **Step 4: Manual playtest**

Run: `./run_game.sh`

Test checklist:
- [ ] Ships spawn with radar OFF (grey indicator gizmo)
- [ ] R key toggles radar ON for selected ship (blue indicator gizmo)
- [ ] Distant enemy ships show pulsing orange circle (signature) on radar
- [ ] Closer enemy ships show red diamond marker (track) on radar
- [ ] K mode numbers appear on tracked enemies
- [ ] Can target a track via number key (TargetByContactCommand)
- [ ] RWR yellow bearing lines appear when enemy radar pings your ship
- [ ] PD engages radar-tracked missiles outside visual range
- [ ] Tracked enemy missiles show orange X markers
- [ ] Visual LOS still works at 400m (ship model appears/disappears)
- [ ] Fleet builder shows SearchRadar and NavRadar in weapon picker
- [ ] Ghost fade-out still works on visual LOS loss

- [ ] **Step 5: Commit any cleanup**

```bash
git add -A
git commit -m "chore: cleanup and finalize Phase 4a radar detection"
```
