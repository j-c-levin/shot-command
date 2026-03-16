# Phase 3c: Fleet Composition Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a pre-game fleet composition screen where players build fleets from a 1000-point budget, assign weapons to mount slots, and submit before the match begins.

**Architecture:** Server-authoritative lobby. Clients build fleets locally in a new `FleetComposition` game state, submit via network trigger. Server validates, tracks readiness, runs 3s countdown when both submitted, then spawns fleets from specs. Existing `spawn_server_ship` gains a `ShipSpec` parameter. New `src/fleet/` module for costs, specs, and validation. New `src/ui/` module for the Bevy UI overlay.

**Tech Stack:** Bevy 0.18 UI nodes, bevy_replicon 0.39 client/server events, serde for network serialization.

---

## File Structure

### New files
| File | Responsibility |
|------|----------------|
| `src/fleet/mod.rs` | `ShipSpec`, `FleetSpec`, point cost constants, `fleet_cost()`, `validate_fleet()`, `MountSize::fits()`, `FleetPlugin` |
| `src/fleet/lobby.rs` | Server-side `LobbyTracker` resource, lobby systems (`handle_fleet_submission`, `handle_cancel_submission`, `tick_lobby_countdown`), `LobbyPlugin` |
| `src/ui/mod.rs` | `FleetUiPlugin`, root UI spawn/despawn on state enter/exit |
| `src/ui/fleet_builder.rs` | Fleet builder UI components, interaction systems (add ship, select ship, change weapon, remove weapon, remove ship, submit, cancel) |

### Modified files
| File | Changes |
|------|---------|
| `src/lib.rs` | Add `pub mod fleet;` and `pub mod ui;` |
| `src/game/mod.rs:14` | Add `FleetComposition` variant to `GameState` |
| `src/net/commands.rs` | Add `FleetSubmission`, `CancelSubmission`, `LobbyStatus` event types |
| `src/net/replication.rs` | Register new events (client: `FleetSubmission`, `CancelSubmission`; server: `LobbyStatus`) |
| `src/net/server.rs:65-116` | Add lobby observers + systems to `ServerNetPlugin`, remove auto-transition to `Playing` from `on_client_connected`, move `TeamAssignment` send to `on_client_connected` |
| `src/net/server.rs:195-275` | Refactor `server_setup_game` to spawn from `LobbyTracker` specs instead of hardcoded fleets, add asteroid exclusion zones |
| `src/net/client.rs:110-123` | `on_team_assignment` transitions to `FleetComposition` instead of `Playing` |
| `src/net/client.rs:24-61` | Add `LobbyStatus` observer, add `FleetUiPlugin` |
| `src/ship/mod.rs:715-745` | `spawn_server_ship` takes `&ShipSpec` instead of `ShipClass`, builds mounts from spec |
| `src/weapon/mod.rs:158-168` | Add `MountSize::fits()` method and `Ord` impl |
| `src/bin/server.rs` | Add `FleetPlugin` and `LobbyPlugin` |
| `src/bin/client.rs` | Add `FleetPlugin` and `FleetUiPlugin` |

---

## Chunk 1: Data Model, Costs & Validation

### Task 1: Point cost constants and ShipSpec type

**Files:**
- Create: `src/fleet/mod.rs`
- Modify: `src/lib.rs`

- [ ] **Step 1: Write the failing test — hull costs**

In `src/fleet/mod.rs`:

```rust
use crate::ship::ShipClass;
use crate::weapon::{MountSize, WeaponType};
use serde::{Deserialize, Serialize};

/// Point cost for a ship hull.
pub fn hull_cost(class: ShipClass) -> u16 {
    todo!()
}

/// Point cost for a weapon.
pub fn weapon_cost(weapon: WeaponType) -> u16 {
    todo!()
}

/// Total fleet budget.
pub const FLEET_BUDGET: u16 = 1000;

/// A single ship's specification: class + weapon assignments per mount slot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShipSpec {
    pub class: ShipClass,
    /// One entry per mount slot (same order as `ShipClass::mount_layout()`).
    /// `None` means the slot is empty.
    pub loadout: Vec<Option<WeaponType>>,
}

/// Cost of a single ship spec (hull + weapons).
pub fn ship_spec_cost(spec: &ShipSpec) -> u16 {
    todo!()
}

/// Cost of an entire fleet.
pub fn fleet_cost(specs: &[ShipSpec]) -> u16 {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hull_costs() {
        assert_eq!(hull_cost(ShipClass::Battleship), 375);
        assert_eq!(hull_cost(ShipClass::Destroyer), 150);
        assert_eq!(hull_cost(ShipClass::Scout), 45);
    }
}
```

In `src/lib.rs`, add `pub mod fleet;` after existing modules.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test hull_costs`
Expected: FAIL with `todo!()` panic.

- [ ] **Step 3: Implement hull_cost**

```rust
pub fn hull_cost(class: ShipClass) -> u16 {
    match class {
        ShipClass::Battleship => 375,
        ShipClass::Destroyer => 150,
        ShipClass::Scout => 45,
    }
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test hull_costs`
Expected: PASS

- [ ] **Step 5: Write failing test — weapon costs**

```rust
#[test]
fn weapon_costs() {
    assert_eq!(weapon_cost(WeaponType::HeavyCannon), 30);
    assert_eq!(weapon_cost(WeaponType::Railgun), 40);
    assert_eq!(weapon_cost(WeaponType::HeavyVLS), 35);
    assert_eq!(weapon_cost(WeaponType::Cannon), 15);
    assert_eq!(weapon_cost(WeaponType::LightVLS), 20);
    assert_eq!(weapon_cost(WeaponType::LaserPD), 25);
    assert_eq!(weapon_cost(WeaponType::CWIS), 10);
}
```

- [ ] **Step 6: Implement weapon_cost**

```rust
pub fn weapon_cost(weapon: WeaponType) -> u16 {
    match weapon {
        WeaponType::HeavyCannon => 30,
        WeaponType::Railgun => 40,
        WeaponType::HeavyVLS => 35,
        WeaponType::Cannon => 15,
        WeaponType::LightVLS => 20,
        WeaponType::LaserPD => 25,
        WeaponType::CWIS => 10,
    }
}
```

- [ ] **Step 7: Run test, verify pass**

Run: `cargo test weapon_costs`
Expected: PASS

- [ ] **Step 8: Write failing test — ship_spec_cost and fleet_cost**

```rust
#[test]
fn ship_spec_cost_full_destroyer() {
    let spec = ShipSpec {
        class: ShipClass::Destroyer,
        loadout: vec![
            Some(WeaponType::Railgun),   // Large
            Some(WeaponType::Cannon),    // Medium
            Some(WeaponType::LaserPD),   // Medium
            Some(WeaponType::CWIS),      // Small
        ],
    };
    // 150 + 40 + 15 + 25 + 10 = 240
    assert_eq!(ship_spec_cost(&spec), 240);
}

#[test]
fn ship_spec_cost_empty_slots() {
    let spec = ShipSpec {
        class: ShipClass::Scout,
        loadout: vec![None, None],
    };
    assert_eq!(ship_spec_cost(&spec), 45); // hull only
}

#[test]
fn fleet_cost_multiple_ships() {
    let specs = vec![
        ShipSpec {
            class: ShipClass::Battleship,
            loadout: vec![None, None, None, None, None, None],
        },
        ShipSpec {
            class: ShipClass::Scout,
            loadout: vec![Some(WeaponType::Cannon), Some(WeaponType::CWIS)],
        },
    ];
    // 375 + 45 + 15 + 10 = 445
    assert_eq!(fleet_cost(&specs), 445);
}
```

- [ ] **Step 9: Implement ship_spec_cost and fleet_cost**

```rust
pub fn ship_spec_cost(spec: &ShipSpec) -> u16 {
    let weapons: u16 = spec
        .loadout
        .iter()
        .filter_map(|w| w.map(weapon_cost))
        .sum();
    hull_cost(spec.class) + weapons
}

pub fn fleet_cost(specs: &[ShipSpec]) -> u16 {
    specs.iter().map(ship_spec_cost).sum()
}
```

- [ ] **Step 10: Run tests, verify pass**

Run: `cargo test -p nebulous_shot_command fleet`
Expected: PASS (all fleet module tests)

- [ ] **Step 11: Commit**

```bash
git add src/lib.rs src/fleet/mod.rs
git commit -m "feat: add fleet module — ShipSpec, point costs, fleet_cost"
```

---

### Task 2: MountSize ordering and fits() method

**Files:**
- Modify: `src/weapon/mod.rs:11-16` (MountSize enum)

- [ ] **Step 1: Write failing test — MountSize::fits**

Add to `src/weapon/mod.rs` test block:

```rust
#[test]
fn mount_size_fits_same_size() {
    assert!(MountSize::Large.fits(MountSize::Large));
    assert!(MountSize::Medium.fits(MountSize::Medium));
    assert!(MountSize::Small.fits(MountSize::Small));
}

#[test]
fn mount_size_fits_smaller() {
    assert!(MountSize::Large.fits(MountSize::Medium));
    assert!(MountSize::Large.fits(MountSize::Small));
    assert!(MountSize::Medium.fits(MountSize::Small));
}

#[test]
fn mount_size_rejects_larger() {
    assert!(!MountSize::Small.fits(MountSize::Medium));
    assert!(!MountSize::Small.fits(MountSize::Large));
    assert!(!MountSize::Medium.fits(MountSize::Large));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test mount_size_fits`
Expected: FAIL — `fits` method doesn't exist.

- [ ] **Step 3: Implement MountSize::fits**

In `src/weapon/mod.rs`, add to `MountSize` impl block:

```rust
impl MountSize {
    /// Returns the numeric rank of this mount size (Large=2, Medium=1, Small=0).
    fn rank(self) -> u8 {
        match self {
            MountSize::Large => 2,
            MountSize::Medium => 1,
            MountSize::Small => 0,
        }
    }

    /// Whether this slot can accept a weapon of the given size.
    /// Larger slots accept smaller weapons (downsizing).
    pub fn fits(self, weapon_size: MountSize) -> bool {
        self.rank() >= weapon_size.rank()
    }
}
```

- [ ] **Step 4: Run tests, verify pass**

Run: `cargo test mount_size_fits`
Expected: PASS (all 3 tests)

- [ ] **Step 5: Commit**

```bash
git add src/weapon/mod.rs
git commit -m "feat: MountSize::fits() — downsizing support for mount slots"
```

---

### Task 3: Fleet validation

**Files:**
- Modify: `src/fleet/mod.rs`
- Modify: `src/ship/mod.rs` (make `mount_layout` and `default_loadout` pub)

- [ ] **Step 1: Make mount_layout and default_loadout public**

In `src/ship/mod.rs:79`, change `fn mount_layout` to `pub fn mount_layout`.
In `src/ship/mod.rs:109`, change `fn default_loadout` to `pub fn default_loadout`.
Both are needed: `mount_layout` for validation, `default_loadout` for the `spawn_server_ship_default` convenience function.

- [ ] **Step 2: Write failing test — validate_fleet**

In `src/fleet/mod.rs`, add:

```rust
use crate::weapon::MountSize;

/// Validation error for a fleet submission.
#[derive(Debug, Clone, PartialEq)]
pub enum FleetError {
    OverBudget { cost: u16, budget: u16 },
    WrongSlotCount { ship_index: usize, expected: usize, got: usize },
    WeaponTooLarge { ship_index: usize, slot_index: usize, slot_size: MountSize, weapon_size: MountSize },
    EmptyFleet,
}

/// Validate a fleet submission. Returns Ok(()) or the first error found.
pub fn validate_fleet(specs: &[ShipSpec]) -> Result<(), FleetError> {
    todo!()
}

#[cfg(test)]
mod tests {
    // ... existing tests ...

    #[test]
    fn validate_valid_fleet() {
        let specs = vec![ShipSpec {
            class: ShipClass::Destroyer,
            loadout: vec![
                Some(WeaponType::Railgun),
                Some(WeaponType::Cannon),
                Some(WeaponType::LaserPD),
                Some(WeaponType::CWIS),
            ],
        }];
        assert!(validate_fleet(&specs).is_ok());
    }

    #[test]
    fn validate_over_budget() {
        // 4 battleships = 1500, over 1000
        let specs = vec![
            ShipSpec { class: ShipClass::Battleship, loadout: vec![None; 6] },
            ShipSpec { class: ShipClass::Battleship, loadout: vec![None; 6] },
            ShipSpec { class: ShipClass::Battleship, loadout: vec![None; 6] },
            ShipSpec { class: ShipClass::Battleship, loadout: vec![None; 6] },
        ];
        assert_eq!(
            validate_fleet(&specs),
            Err(FleetError::OverBudget { cost: 1500, budget: 1000 }),
        );
    }

    #[test]
    fn validate_wrong_slot_count() {
        let specs = vec![ShipSpec {
            class: ShipClass::Scout,
            loadout: vec![None, None, None], // Scout has 2 slots, not 3
        }];
        assert_eq!(
            validate_fleet(&specs),
            Err(FleetError::WrongSlotCount { ship_index: 0, expected: 2, got: 3 }),
        );
    }

    #[test]
    fn validate_weapon_too_large() {
        let specs = vec![ShipSpec {
            class: ShipClass::Scout,
            loadout: vec![
                Some(WeaponType::HeavyCannon), // Large weapon in Medium slot
                Some(WeaponType::CWIS),
            ],
        }];
        assert_eq!(
            validate_fleet(&specs),
            Err(FleetError::WeaponTooLarge {
                ship_index: 0,
                slot_index: 0,
                slot_size: MountSize::Medium,
                weapon_size: MountSize::Large,
            }),
        );
    }

    #[test]
    fn validate_empty_fleet() {
        assert_eq!(validate_fleet(&[]), Err(FleetError::EmptyFleet));
    }

    #[test]
    fn validate_downsized_weapon_ok() {
        let specs = vec![ShipSpec {
            class: ShipClass::Destroyer,
            loadout: vec![
                Some(WeaponType::Cannon),   // Medium weapon in Large slot — ok
                Some(WeaponType::CWIS),     // Small weapon in Medium slot — ok
                None,
                Some(WeaponType::CWIS),
            ],
        }];
        assert!(validate_fleet(&specs).is_ok());
    }
}
```

- [ ] **Step 3: Run test to verify it fails**

Run: `cargo test validate_`
Expected: FAIL with `todo!()` panic.

- [ ] **Step 4: Implement validate_fleet**

```rust
pub fn validate_fleet(specs: &[ShipSpec]) -> Result<(), FleetError> {
    if specs.is_empty() {
        return Err(FleetError::EmptyFleet);
    }

    let cost = fleet_cost(specs);
    if cost > FLEET_BUDGET {
        return Err(FleetError::OverBudget { cost, budget: FLEET_BUDGET });
    }

    for (i, spec) in specs.iter().enumerate() {
        let layout = spec.class.mount_layout();
        if spec.loadout.len() != layout.len() {
            return Err(FleetError::WrongSlotCount {
                ship_index: i,
                expected: layout.len(),
                got: spec.loadout.len(),
            });
        }
        for (j, (weapon_opt, (slot_size, _offset))) in
            spec.loadout.iter().zip(layout.iter()).enumerate()
        {
            if let Some(weapon) = weapon_opt {
                let weapon_size = weapon.mount_size();
                if !slot_size.fits(weapon_size) {
                    return Err(FleetError::WeaponTooLarge {
                        ship_index: i,
                        slot_index: j,
                        slot_size: *slot_size,
                        weapon_size,
                    });
                }
            }
        }
    }

    Ok(())
}
```

- [ ] **Step 5: Run tests, verify pass**

Run: `cargo test validate_`
Expected: PASS (all 6 validation tests)

- [ ] **Step 6: Commit**

```bash
git add src/fleet/mod.rs src/ship/mod.rs
git commit -m "feat: fleet validation — budget, slot count, weapon size checks"
```

---

### Task 4: FleetPlugin (shared, no systems yet)

**Files:**
- Modify: `src/fleet/mod.rs`
- Modify: `src/bin/server.rs`
- Modify: `src/bin/client.rs`

- [ ] **Step 1: Add FleetPlugin struct**

In `src/fleet/mod.rs`, add:

```rust
pub mod lobby;

use bevy::prelude::*;

pub struct FleetPlugin;

impl Plugin for FleetPlugin {
    fn build(&self, _app: &mut App) {
        // Types and functions only — no systems registered here.
        // Lobby systems are in LobbyPlugin (server-only).
        // UI systems are in FleetUiPlugin (client-only).
    }
}
```

Create empty `src/fleet/lobby.rs`:

```rust
//! Server-side lobby tracking and fleet submission handling.
```

- [ ] **Step 2: Add FleetPlugin to both binaries**

In `src/bin/server.rs`, add `use nebulous_shot_command::fleet::FleetPlugin;` and add `FleetPlugin` to the plugin tuple.

In `src/bin/client.rs`, add `use nebulous_shot_command::fleet::FleetPlugin;` and add `FleetPlugin` to the plugin tuple.

- [ ] **Step 3: Run cargo check**

Run: `cargo check`
Expected: compiles cleanly.

- [ ] **Step 4: Commit**

```bash
git add src/fleet/mod.rs src/fleet/lobby.rs src/bin/server.rs src/bin/client.rs
git commit -m "feat: add FleetPlugin shell — wired into server and client binaries"
```

---

## Chunk 2: Game State & Network Events

### Task 5: Add FleetComposition game state

**Files:**
- Modify: `src/game/mod.rs:14-23`

- [ ] **Step 1: Add FleetComposition variant**

In `src/game/mod.rs`, add `FleetComposition` between `Connecting` and `Playing`:

```rust
#[derive(Clone, Copy, Default, Eq, PartialEq, Debug, Hash, States, Serialize, Deserialize)]
pub enum GameState {
    #[default]
    Setup,
    /// Server: waiting for both clients to connect and submit fleets
    WaitingForPlayers,
    /// Client: connecting to server, waiting for team assignment
    Connecting,
    /// Client: building fleet in the composition screen
    FleetComposition,
    Playing,
    GameOver,
}
```

- [ ] **Step 2: Add test**

```rust
#[test]
fn game_state_has_fleet_composition() {
    let state = GameState::FleetComposition;
    assert_ne!(state, GameState::Playing);
    assert_ne!(state, GameState::Connecting);
}
```

- [ ] **Step 3: Run cargo test**

Run: `cargo test game_state_has_fleet`
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add src/game/mod.rs
git commit -m "feat: add GameState::FleetComposition variant"
```

---

### Task 6: Network event types

**Files:**
- Modify: `src/net/commands.rs`

- [ ] **Step 1: Add FleetSubmission, CancelSubmission, LobbyStatus**

Append to `src/net/commands.rs`:

```rust
use bevy::ecs::entity::MapEntities;
use crate::fleet::ShipSpec;

/// Client → server: submit fleet composition.
/// No Entity fields, but MapEntities is required by add_mapped_client_event.
#[derive(Event, Debug, Clone, Serialize, Deserialize)]
pub struct FleetSubmission {
    pub ships: Vec<ShipSpec>,
}

impl MapEntities for FleetSubmission {
    fn map_entities<M: bevy::ecs::entity::EntityMapper>(&mut self, _mapper: &mut M) {}
}

/// Client → server: cancel fleet submission to re-edit.
#[derive(Event, Debug, Clone, Serialize, Deserialize)]
pub struct CancelSubmission;

impl MapEntities for CancelSubmission {
    fn map_entities<M: bevy::ecs::entity::EntityMapper>(&mut self, _mapper: &mut M) {}
}

/// The current state of the pre-game lobby.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LobbyState {
    /// You've connected, build your fleet.
    Composing,
    /// You've submitted, opponent not connected yet.
    WaitingForOpponent,
    /// You've submitted, opponent is still composing.
    OpponentComposing,
    /// Both submitted, countdown running (seconds remaining).
    Countdown(f32),
    /// Your submission was rejected.
    Rejected(String),
}

/// Server → client: lobby status update.
#[derive(Event, Debug, Clone, Serialize, Deserialize)]
pub struct LobbyStatus {
    pub state: LobbyState,
}
```

- [ ] **Step 2: Run cargo check**

Run: `cargo check`
Expected: compiles (events not yet registered, just defined).

- [ ] **Step 3: Commit**

```bash
git add src/net/commands.rs
git commit -m "feat: add FleetSubmission, CancelSubmission, LobbyStatus network events"
```

---

### Task 7: Register events in replication plugin

**Files:**
- Modify: `src/net/replication.rs`

- [ ] **Step 1: Register new events**

In `src/net/replication.rs`, add imports:

```rust
use crate::net::commands::{
    CancelMissilesCommand, CancelSubmission, ClearTargetCommand, FacingLockCommand,
    FacingUnlockCommand, FireMissileCommand, FleetSubmission, GameResult, LobbyStatus,
    MoveCommand, TargetCommand, TeamAssignment,
};
```

Add to the client→server section (after `CancelMissilesCommand`):

```rust
.add_mapped_client_event::<FleetSubmission>(Channel::Ordered)
.add_mapped_client_event::<CancelSubmission>(Channel::Ordered);
```

Add to the server→client section (after `GameResult`):

```rust
app.add_server_event::<LobbyStatus>(Channel::Ordered);
```

Note: `FleetSubmission` and `CancelSubmission` have no Entity fields, but we use `add_mapped_client_event` (with no-op `MapEntities` impls) to match the existing codebase pattern. The `FromClient<T>` observer pattern requires this registration method.

- [ ] **Step 2: Run cargo check**

Run: `cargo check`
Expected: compiles cleanly.

- [ ] **Step 3: Commit**

```bash
git add src/net/replication.rs
git commit -m "feat: register fleet events in SharedReplicationPlugin"
```

---

### Task 8: Update client state transition

**Files:**
- Modify: `src/net/client.rs:110-123`

- [ ] **Step 1: Change on_team_assignment to transition to FleetComposition**

```rust
fn on_team_assignment(
    trigger: On<TeamAssignment>,
    mut local_team: ResMut<LocalTeam>,
    mut next_state: ResMut<NextState<GameState>>,
) {
    let assignment = &*trigger;
    let team = assignment.team;

    info!("Received team assignment: Team({})", team.0);
    local_team.0 = Some(team);

    next_state.set(GameState::FleetComposition);
    info!("Transitioning to FleetComposition state");
}
```

- [ ] **Step 2: Add LobbyStatus observer to ClientNetPlugin**

In `ClientNetPlugin::build`, add:

```rust
app.add_observer(on_lobby_status);
```

And add the observer function:

```rust
/// Resource tracking the latest lobby state from server.
#[derive(Resource, Debug, Clone, Default)]
pub struct CurrentLobbyState(pub Option<crate::net::commands::LobbyState>);

fn on_lobby_status(
    trigger: On<crate::net::commands::LobbyStatus>,
    mut lobby_state: ResMut<CurrentLobbyState>,
    mut next_state: ResMut<NextState<GameState>>,
) {
    let status = &*trigger;
    info!("Lobby status: {:?}", status.state);
    lobby_state.0 = Some(status.state.clone());

    // When countdown reaches zero, server will transition to Playing via
    // a separate mechanism (TeamAssignment or new event). For now, just store state.
}
```

Init the resource in `ClientNetPlugin::build`:

```rust
app.init_resource::<CurrentLobbyState>();
```

- [ ] **Step 3: Run cargo check**

Run: `cargo check`
Expected: compiles cleanly.

- [ ] **Step 4: Commit**

```bash
git add src/net/client.rs
git commit -m "feat: client transitions to FleetComposition on team assignment"
```

---

### Task 9: Update server — send TeamAssignment on connect, remove auto-Playing transition

**Files:**
- Modify: `src/net/server.rs:164-190` (`on_client_connected`)

- [ ] **Step 1: Refactor on_client_connected**

Send `TeamAssignment` immediately on connect. Remove the auto-transition to `Playing`:

```rust
fn on_client_connected(
    trigger: On<Add, AuthorizedClient>,
    mut commands: Commands,
    mut client_teams: ResMut<ClientTeams>,
) {
    let client_entity = trigger.entity;
    let team_id = client_teams.map.len() as u8;
    let team = Team(team_id);

    client_teams.map.insert(client_entity, team);

    info!(
        "Client {:?} connected, assigned Team({}). Total clients: {}",
        client_entity,
        team_id,
        client_teams.map.len()
    );

    // Send TeamAssignment immediately so client can enter FleetComposition
    commands.server_trigger(ToClients {
        mode: SendMode::Direct(ClientId::Client(client_entity)),
        message: TeamAssignment { team },
    });
}
```

- [ ] **Step 2: Remove TeamAssignment send from server_setup_game**

In `server_setup_game` (around lines 263-269), delete the TeamAssignment loop since it's now sent on connect.

- [ ] **Step 3: Run cargo check**

Run: `cargo check`
Expected: compiles (some warnings about unused imports are ok for now).

- [ ] **Step 4: Commit**

```bash
git add src/net/server.rs
git commit -m "feat: send TeamAssignment on connect, remove auto-Playing transition"
```

---

## Chunk 3: Server Lobby Systems

### Task 10: LobbyTracker resource and submission handler

**Files:**
- Modify: `src/fleet/lobby.rs`

- [ ] **Step 1: Write the LobbyTracker and LobbyPlugin**

```rust
use std::collections::HashMap;

use bevy::prelude::*;
use bevy_replicon::prelude::*;
use bevy_replicon::shared::message::client_message::FromClient;

use crate::fleet::{validate_fleet, ShipSpec};
use crate::game::GameState;
use crate::net::commands::{CancelSubmission, FleetSubmission, LobbyState, LobbyStatus};
use crate::net::server::ClientTeams;

/// Tracks fleet submissions and countdown state.
#[derive(Resource, Debug, Default)]
pub struct LobbyTracker {
    pub submissions: HashMap<Entity, Vec<ShipSpec>>,
    pub countdown: Option<f32>,
}

pub struct LobbyPlugin;

impl Plugin for LobbyPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<LobbyTracker>();
        app.add_observer(handle_fleet_submission);
        app.add_observer(handle_cancel_submission);
        app.add_systems(
            Update,
            tick_lobby_countdown.run_if(in_state(GameState::WaitingForPlayers)),
        );
    }
}
```

- [ ] **Step 2: Implement handle_fleet_submission**

```rust
fn handle_fleet_submission(
    trigger: On<FromClient<FleetSubmission>>,
    mut commands: Commands,
    mut lobby: ResMut<LobbyTracker>,
    client_teams: Res<ClientTeams>,
) {
    let from = trigger.event();
    let submission = &from.message;
    let client_entity = match from.client_id {
        ClientId::Client(e) => e,
        ClientId::Server => return,
    };

    // Validate
    match validate_fleet(&submission.ships) {
        Ok(()) => {
            info!("Client {:?} submitted valid fleet ({} ships)", client_entity, submission.ships.len());
            lobby.submissions.insert(client_entity, submission.ships.clone());

            // Check if both players have submitted
            if lobby.submissions.len() >= 2 {
                lobby.countdown = Some(3.0);
                // Notify both clients
                commands.server_trigger(ToClients {
                    mode: SendMode::Broadcast,
                    message: LobbyStatus { state: LobbyState::Countdown(3.0) },
                });
            } else {
                // Notify submitter they're waiting
                let other_connected = client_teams.map.len() >= 2;
                let state = if other_connected {
                    LobbyState::OpponentComposing
                } else {
                    LobbyState::WaitingForOpponent
                };
                commands.server_trigger(ToClients {
                    mode: SendMode::Direct(from.client_id),
                    message: LobbyStatus { state },
                });
            }
        }
        Err(e) => {
            warn!("Client {:?} fleet rejected: {:?}", client_entity, e);
            commands.server_trigger(ToClients {
                mode: SendMode::Direct(from.client_id),
                message: LobbyStatus {
                    state: LobbyState::Rejected(format!("{:?}", e)),
                },
            });
        }
    }
}
```

- [ ] **Step 3: Implement handle_cancel_submission**

```rust
fn handle_cancel_submission(
    trigger: On<FromClient<CancelSubmission>>,
    mut commands: Commands,
    mut lobby: ResMut<LobbyTracker>,
    client_teams: Res<ClientTeams>,
) {
    let from = trigger.event();
    let client_entity = match from.client_id {
        ClientId::Client(e) => e,
        ClientId::Server => return,
    };

    if lobby.submissions.remove(&client_entity).is_some() {
        info!("Client {:?} cancelled fleet submission", client_entity);

        // Reset countdown if it was running
        lobby.countdown = None;

        // Notify canceller they're back to composing
        commands.server_trigger(ToClients {
            mode: SendMode::Direct(from.client_id),
            message: LobbyStatus { state: LobbyState::Composing },
        });

        // Notify the other client (if they've submitted) that opponent is composing
        for (&other_entity, _) in &client_teams.map {
            if other_entity != client_entity && lobby.submissions.contains_key(&other_entity) {
                commands.server_trigger(ToClients {
                    mode: SendMode::Direct(ClientId::Client(other_entity)),
                    message: LobbyStatus { state: LobbyState::OpponentComposing },
                });
            }
        }
    }
}
```

- [ ] **Step 4: Run cargo check**

Run: `cargo check`
Expected: compiles.

- [ ] **Step 5: Commit**

```bash
git add src/fleet/lobby.rs
git commit -m "feat: lobby submission/cancel handlers with validation"
```

---

### Task 11: Lobby countdown and transition to Playing

**Files:**
- Modify: `src/fleet/lobby.rs`
- Modify: `src/net/server.rs:195-275` (`server_setup_game`)
- Modify: `src/ship/mod.rs:715-745` (`spawn_server_ship`)

- [ ] **Step 1: Implement tick_lobby_countdown**

```rust
fn tick_lobby_countdown(
    mut commands: Commands,
    mut lobby: ResMut<LobbyTracker>,
    mut next_state: ResMut<NextState<GameState>>,
    time: Res<Time>,
) {
    let Some(ref mut remaining) = lobby.countdown else {
        return;
    };

    *remaining -= time.delta_secs();

    if *remaining <= 0.0 {
        info!("Lobby countdown complete, transitioning to Playing");
        next_state.set(GameState::Playing);
    } else {
        // Broadcast countdown tick (~every frame, clients can throttle display)
        commands.server_trigger(ToClients {
            mode: SendMode::Broadcast,
            message: LobbyStatus {
                state: LobbyState::Countdown(*remaining),
            },
        });
    }
}
```

- [ ] **Step 2: Refactor spawn_server_ship to accept ShipSpec**

In `src/ship/mod.rs`, change the signature and implementation:

```rust
pub fn spawn_server_ship(
    commands: &mut Commands,
    position: Vec2,
    team: Team,
    spec: &crate::fleet::ShipSpec,
) -> Entity {
    let class = spec.class;
    let layout = class.mount_layout();
    let mounts: Vec<Mount> = layout
        .into_iter()
        .zip(spec.loadout.iter())
        .map(|((size, offset), weapon_type)| Mount {
            size,
            offset,
            weapon: weapon_type.map(|wt| {
                let profile = wt.profile();
                WeaponState {
                    weapon_type: wt,
                    ammo: 0,
                    cooldown: 0.0,
                    pd_retarget_cooldown: 0.0,
                    tubes_loaded: profile.tubes,
                    tube_reload_timer: 0.0,
                }
            }),
        })
        .collect();

    let ship_entity = commands
        .spawn((
            Ship,
            Replicated,
            team,
            class,
            Velocity::default(),
            WaypointQueue::default(),
            MissileQueue::default(),
            Transform::from_xyz(position.x, 5.0, position.y),
            Health { hp: class.profile().hp },
            Mounts(mounts),
        ))
        .id();

    commands.spawn((
        ShipSecrets,
        ShipSecretsOwner(ship_entity),
        Replicated,
        WaypointQueue::default(),
        MissileQueue::default(),
    ));

    ship_entity
}

/// Convenience: spawn with default loadout (for tests and AI).
pub fn spawn_server_ship_default(
    commands: &mut Commands,
    position: Vec2,
    team: Team,
    class: ShipClass,
) -> Entity {
    let spec = crate::fleet::ShipSpec {
        class,
        loadout: class.default_loadout(),
    };
    spawn_server_ship(commands, position, team, &spec)
}
```

- [ ] **Step 3: Update server_setup_game to use LobbyTracker**

In `src/net/server.rs`, refactor `server_setup_game`:

```rust
fn server_setup_game(
    mut commands: Commands,
    client_teams: Res<ClientTeams>,
    lobby: Res<crate::fleet::lobby::LobbyTracker>,
) {
    commands.insert_resource(MapBounds {
        half_extents: Vec2::splat(500.0),
    });

    // Spawn fleets from submitted specs
    for (&client_entity, &team) in &client_teams.map {
        if let Some(specs) = lobby.submissions.get(&client_entity) {
            let ship_count = specs.len();
            let base = match team.0 {
                0 => Vec2::new(-300.0, -300.0),
                _ => Vec2::new(300.0, 300.0),
            };

            for (i, spec) in specs.iter().enumerate() {
                let spacing = 30.0;
                let offset = (i as f32 - (ship_count - 1) as f32 / 2.0) * spacing;
                let position = base + Vec2::new(-offset, offset) * 0.707;

                let entity = spawn_server_ship(&mut commands, position, team, spec);
                info!("Spawned {:?} for Team {}: {:?}", spec.class, team.0, entity);
            }
        }
    }

    // Spawn asteroids with exclusion zones
    let bounds = MapBounds {
        half_extents: Vec2::splat(500.0),
    };
    let spawn_zones = [
        Vec2::new(-300.0, -300.0),
        Vec2::new(300.0, 300.0),
    ];
    let spawn_exclusion = 100.0;
    let mut rng = rand::rng();
    let asteroid_count = 12;
    let min_distance_from_edge = 50.0;
    let min_distance_from_center = 100.0;

    for _ in 0..asteroid_count {
        let radius = rng.random_range(15.0..40.0);

        let pos = loop {
            let candidate = Vec2::new(
                rng.random_range(
                    (-bounds.half_extents.x + min_distance_from_edge)
                        ..(bounds.half_extents.x - min_distance_from_edge),
                ),
                rng.random_range(
                    (-bounds.half_extents.y + min_distance_from_edge)
                        ..(bounds.half_extents.y - min_distance_from_edge),
                ),
            );
            let too_close_to_spawn = spawn_zones
                .iter()
                .any(|zone| (candidate - *zone).length() < spawn_exclusion);
            if candidate.length() > min_distance_from_center && !too_close_to_spawn {
                break candidate;
            }
        };

        commands.spawn((
            Asteroid,
            AsteroidSize { radius },
            Transform::from_xyz(pos.x, 0.0, pos.y),
            Replicated,
        ));
    }

    info!("Server: spawned fleets from lobby specs and {} asteroids", asteroid_count);
}
```

- [ ] **Step 4: Update all call sites of spawn_server_ship**

Search for any remaining calls to `spawn_server_ship` with the old signature and update them to use `spawn_server_ship_default` (likely only in tests, since `server_setup_game` was the main call site).

Run: `grep -rn "spawn_server_ship" src/`

- [ ] **Step 5: Run cargo check**

Run: `cargo check`
Expected: compiles.

- [ ] **Step 6: Write test — asteroid exclusion zone**

In `src/net/server.rs` or `src/fleet/mod.rs`, add a pure-function test:

```rust
#[test]
fn asteroid_exclusion_rejects_near_spawn() {
    let spawn_zones = [
        Vec2::new(-300.0, -300.0),
        Vec2::new(300.0, 300.0),
    ];
    let exclusion = 100.0;

    // Point right on spawn zone
    let too_close = spawn_zones
        .iter()
        .any(|zone| (Vec2::new(-300.0, -300.0) - *zone).length() < exclusion);
    assert!(too_close);

    // Point far away
    let far = spawn_zones
        .iter()
        .any(|zone| (Vec2::new(0.0, 0.0) - *zone).length() < exclusion);
    assert!(!far);
}
```

- [ ] **Step 7: Run tests**

Run: `cargo test`
Expected: all pass.

- [ ] **Step 8: Commit**

```bash
git add src/fleet/lobby.rs src/ship/mod.rs src/net/server.rs
git commit -m "feat: lobby countdown, spec-based spawning, asteroid exclusion zones"
```

---

### Task 12: Wire LobbyPlugin into server binary

**Files:**
- Modify: `src/bin/server.rs`
- Modify: `src/net/server.rs:65-116` (ServerNetPlugin)

- [ ] **Step 1: Add LobbyPlugin to server**

In `src/bin/server.rs`, add:

```rust
use nebulous_shot_command::fleet::lobby::LobbyPlugin;
```

Add `LobbyPlugin` to the plugin tuple.

- [ ] **Step 2: Run cargo check**

Run: `cargo check`
Expected: compiles.

- [ ] **Step 3: Commit**

```bash
git add src/bin/server.rs
git commit -m "feat: wire LobbyPlugin into server binary"
```

---

## Chunk 3: Client-to-Playing Transition

### Task 13: Client transitions to Playing on countdown complete

**Files:**
- Modify: `src/net/client.rs`

- [ ] **Step 1: Add a system that watches for Playing transition**

The server transitions to `Playing` which causes replication to start flowing. The client needs to know when to transition. We add a `GameStarted` server event:

In `src/net/commands.rs`, add:

```rust
/// Server → client: game is starting (countdown complete).
#[derive(Event, Debug, Clone, Serialize, Deserialize)]
pub struct GameStarted;
```

In `src/net/replication.rs`, register it:

```rust
app.add_server_event::<GameStarted>(Channel::Ordered);
```

In `src/fleet/lobby.rs`, in `tick_lobby_countdown` when countdown hits 0, add before `next_state.set`:

```rust
commands.server_trigger(ToClients {
    mode: SendMode::Broadcast,
    message: crate::net::commands::GameStarted,
});
```

In `src/net/client.rs`, add observer:

```rust
fn on_game_started(
    _trigger: On<crate::net::commands::GameStarted>,
    mut next_state: ResMut<NextState<GameState>>,
) {
    info!("Game started! Transitioning to Playing");
    next_state.set(GameState::Playing);
}
```

Register in `ClientNetPlugin::build`:

```rust
app.add_observer(on_game_started);
```

- [ ] **Step 2: Run cargo check**

Run: `cargo check`
Expected: compiles.

- [ ] **Step 3: Commit**

```bash
git add src/net/commands.rs src/net/replication.rs src/fleet/lobby.rs src/net/client.rs
git commit -m "feat: GameStarted event triggers client transition to Playing"
```

---

## Chunk 4: Client Fleet Builder UI

### Task 14: UI module scaffold and root node

**Files:**
- Create: `src/ui/mod.rs`
- Create: `src/ui/fleet_builder.rs`
- Modify: `src/lib.rs`
- Modify: `src/bin/client.rs`

- [ ] **Step 1: Create UI module**

`src/ui/mod.rs`:

```rust
pub mod fleet_builder;

use bevy::prelude::*;

use crate::game::GameState;
use fleet_builder::{
    spawn_fleet_ui, despawn_fleet_ui, FleetUiRoot, FleetBuilderState,
};

pub struct FleetUiPlugin;

impl Plugin for FleetUiPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<FleetBuilderState>();
        app.add_systems(OnEnter(GameState::FleetComposition), spawn_fleet_ui);
        app.add_systems(OnExit(GameState::FleetComposition), despawn_fleet_ui);
        app.add_systems(
            Update,
            fleet_builder::update_fleet_ui
                .run_if(in_state(GameState::FleetComposition)),
        );
    }
}
```

In `src/lib.rs`, add `pub mod ui;`.

In `src/bin/client.rs`, add:

```rust
use nebulous_shot_command::ui::FleetUiPlugin;
```

Add `FleetUiPlugin` to the plugin tuple.

- [ ] **Step 2: Create FleetBuilderState resource**

`src/ui/fleet_builder.rs`:

```rust
use bevy::prelude::*;

use bevy_replicon::prelude::*; // for ClientTriggerExt (commands.client_trigger)

use crate::fleet::{self, ShipSpec, FLEET_BUDGET, fleet_cost, ship_spec_cost};
use crate::net::commands::{FleetSubmission, CancelSubmission, LobbyState};
use crate::net::client::CurrentLobbyState;
use crate::ship::ShipClass;
use crate::weapon::{MountSize, WeaponType};

/// Marker for the fleet UI root entity (for cleanup).
#[derive(Component)]
pub struct FleetUiRoot;

/// Client-local fleet building state.
#[derive(Resource, Debug, Default)]
pub struct FleetBuilderState {
    /// Ships the player is building.
    pub ships: Vec<ShipSpec>,
    /// Index of currently selected ship (for detail panel).
    pub selected_ship: Option<usize>,
    /// Whether the player has submitted.
    pub submitted: bool,
    /// Active popup: which ship/slot is being edited.
    pub popup: Option<PopupKind>,
}

#[derive(Debug, Clone)]
pub enum PopupKind {
    AddShip,
    ChangeWeapon { ship_index: usize, slot_index: usize },
}
```

- [ ] **Step 3: Implement spawn_fleet_ui and despawn_fleet_ui**

```rust
pub fn spawn_fleet_ui(mut commands: Commands) {
    commands
        .spawn((
            FleetUiRoot,
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                ..default()
            },
            BackgroundColor(Color::srgba(0.05, 0.05, 0.1, 0.95)),
        ))
        .with_children(|parent| {
            // Header bar
            parent
                .spawn(Node {
                    width: Val::Percent(100.0),
                    height: Val::Px(50.0),
                    justify_content: JustifyContent::SpaceBetween,
                    align_items: AlignItems::Center,
                    padding: UiRect::horizontal(Val::Px(20.0)),
                    ..default()
                })
                .with_children(|header| {
                    header.spawn((
                        Text::new("FLEET COMPOSITION"),
                        TextFont {
                            font_size: 24.0,
                            ..default()
                        },
                        TextColor(Color::WHITE),
                    ));
                    header.spawn((
                        BudgetText,
                        Text::new("Budget: 0 / 1000"),
                        TextFont {
                            font_size: 20.0,
                            ..default()
                        },
                        TextColor(Color::WHITE),
                    ));
                });

            // Main content area (two panels)
            parent
                .spawn(Node {
                    width: Val::Percent(100.0),
                    flex_grow: 1.0,
                    flex_direction: FlexDirection::Row,
                    ..default()
                })
                .with_children(|main| {
                    // Left panel — fleet list
                    main.spawn((
                        FleetListPanel,
                        Node {
                            width: Val::Percent(35.0),
                            height: Val::Percent(100.0),
                            flex_direction: FlexDirection::Column,
                            padding: UiRect::all(Val::Px(10.0)),
                            row_gap: Val::Px(5.0),
                            ..default()
                        },
                    ));

                    // Right panel — ship detail
                    main.spawn((
                        ShipDetailPanel,
                        Node {
                            width: Val::Percent(65.0),
                            height: Val::Percent(100.0),
                            flex_direction: FlexDirection::Column,
                            padding: UiRect::all(Val::Px(10.0)),
                            row_gap: Val::Px(5.0),
                            ..default()
                        },
                    ));
                });

            // Bottom bar
            parent
                .spawn(Node {
                    width: Val::Percent(100.0),
                    height: Val::Px(50.0),
                    justify_content: JustifyContent::SpaceBetween,
                    align_items: AlignItems::Center,
                    padding: UiRect::horizontal(Val::Px(20.0)),
                    ..default()
                })
                .with_children(|bottom| {
                    bottom.spawn((
                        SubmitButton,
                        Button,
                        Node {
                            padding: UiRect::axes(Val::Px(20.0), Val::Px(10.0)),
                            ..default()
                        },
                        BackgroundColor(Color::srgb(0.2, 0.5, 0.2)),
                    ))
                    .with_child((
                        Text::new("Submit Fleet"),
                        TextFont { font_size: 18.0, ..default() },
                        TextColor(Color::WHITE),
                    ));

                    bottom.spawn((
                        StatusText,
                        Text::new("Composing..."),
                        TextFont { font_size: 16.0, ..default() },
                        TextColor(Color::srgb(0.7, 0.7, 0.7)),
                    ));
                });
        });
}

pub fn despawn_fleet_ui(
    mut commands: Commands,
    query: Query<Entity, With<FleetUiRoot>>,
    mut state: ResMut<FleetBuilderState>,
) {
    for entity in &query {
        commands.entity(entity).despawn();
    }
    // Reset state to avoid stale data if returning to FleetComposition
    *state = FleetBuilderState::default();
}

// Marker components for UI elements that need dynamic updates
#[derive(Component)]
pub struct BudgetText;

#[derive(Component)]
pub struct FleetListPanel;

#[derive(Component)]
pub struct ShipDetailPanel;

#[derive(Component)]
pub struct SubmitButton;

#[derive(Component)]
pub struct StatusText;
```

- [ ] **Step 4: Implement stub update_fleet_ui**

```rust
pub fn update_fleet_ui() {
    // Will be filled in next tasks
}
```

- [ ] **Step 5: Run cargo check**

Run: `cargo check`
Expected: compiles.

- [ ] **Step 6: Commit**

```bash
git add src/ui/mod.rs src/ui/fleet_builder.rs src/lib.rs src/bin/client.rs
git commit -m "feat: fleet UI scaffold — root layout with panels and budget display"
```

---

### Task 15: Fleet list panel — add ship, select ship, remove ship

**Files:**
- Modify: `src/ui/fleet_builder.rs`

- [ ] **Step 1: Add interaction button markers and ship picker types**

```rust
#[derive(Component)]
pub struct AddShipButton;

#[derive(Component)]
pub struct ShipEntry(pub usize);

#[derive(Component)]
pub struct RemoveShipButton(pub usize);

#[derive(Component)]
pub struct ShipPickerOption(pub ShipClass);

#[derive(Component)]
pub struct PopupOverlay;
```

- [ ] **Step 2: Implement rebuild_fleet_list system**

This system rebuilds the left panel children whenever fleet state changes:

```rust
fn rebuild_fleet_list(
    mut commands: Commands,
    state: Res<FleetBuilderState>,
    panel_query: Query<Entity, With<FleetListPanel>>,
) {
    let Ok(panel) = panel_query.single() else { return };

    // Despawn existing children
    commands.entity(panel).despawn_descendants();

    commands.entity(panel).with_children(|parent| {
        // Title
        parent.spawn((
            Text::new("YOUR FLEET"),
            TextFont { font_size: 18.0, ..default() },
            TextColor(Color::WHITE),
        ));

        // Ship entries
        for (i, spec) in state.ships.iter().enumerate() {
            let cost = ship_spec_cost(spec);
            let selected = state.selected_ship == Some(i);
            let bg = if selected {
                Color::srgb(0.2, 0.3, 0.5)
            } else {
                Color::srgb(0.15, 0.15, 0.2)
            };

            parent
                .spawn((
                    ShipEntry(i),
                    Button,
                    Node {
                        width: Val::Percent(100.0),
                        padding: UiRect::all(Val::Px(8.0)),
                        justify_content: JustifyContent::SpaceBetween,
                        ..default()
                    },
                    BackgroundColor(bg),
                ))
                .with_children(|row| {
                    row.spawn((
                        Text::new(format!("{:?}", spec.class)),
                        TextFont { font_size: 16.0, ..default() },
                        TextColor(Color::WHITE),
                    ));
                    row.spawn((
                        Text::new(format!("{} pts", cost)),
                        TextFont { font_size: 14.0, ..default() },
                        TextColor(Color::srgb(0.7, 0.7, 0.7)),
                    ));
                });
        }

        // Add Ship button
        parent
            .spawn((
                AddShipButton,
                Button,
                Node {
                    width: Val::Percent(100.0),
                    padding: UiRect::all(Val::Px(8.0)),
                    justify_content: JustifyContent::Center,
                    ..default()
                },
                BackgroundColor(Color::srgb(0.2, 0.4, 0.2)),
            ))
            .with_child((
                Text::new("+ Add Ship"),
                TextFont { font_size: 16.0, ..default() },
                TextColor(Color::WHITE),
            ));
    });
}
```

- [ ] **Step 3: Implement click handlers**

```rust
fn handle_add_ship_click(
    interaction_query: Query<&Interaction, (Changed<Interaction>, With<AddShipButton>)>,
    mut state: ResMut<FleetBuilderState>,
) {
    for interaction in &interaction_query {
        if *interaction == Interaction::Pressed {
            state.popup = Some(PopupKind::AddShip);
        }
    }
}

fn handle_ship_entry_click(
    interaction_query: Query<(&Interaction, &ShipEntry), Changed<Interaction>>,
    mut state: ResMut<FleetBuilderState>,
) {
    for (interaction, entry) in &interaction_query {
        if *interaction == Interaction::Pressed {
            state.selected_ship = Some(entry.0);
        }
    }
}

fn handle_ship_picker_click(
    interaction_query: Query<(&Interaction, &ShipPickerOption), Changed<Interaction>>,
    mut state: ResMut<FleetBuilderState>,
) {
    for (interaction, option) in &interaction_query {
        if *interaction == Interaction::Pressed {
            let class = option.0;
            let slot_count = class.mount_layout().len();
            state.ships.push(ShipSpec {
                class,
                loadout: vec![None; slot_count],
            });
            state.selected_ship = Some(state.ships.len() - 1);
            state.popup = None;
        }
    }
}
```

- [ ] **Step 4: Wire systems into update_fleet_ui**

Replace the stub `update_fleet_ui` with a system set in `FleetUiPlugin`:

```rust
// In FleetUiPlugin::build, replace the single update_fleet_ui with:
app.add_systems(
    Update,
    (
        rebuild_fleet_list,
        rebuild_ship_detail,
        handle_add_ship_click,
        handle_ship_entry_click,
        handle_ship_picker_click,
        handle_weapon_picker_click,
        handle_remove_weapon_click,
        handle_remove_ship_click,
        handle_submit_click,
        handle_cancel_click,
        update_budget_text,
        update_status_text,
        spawn_popup,
    )
        .run_if(in_state(GameState::FleetComposition)),
);
```

(Many of these will be stub functions initially, implemented in subsequent steps.)

- [ ] **Step 5: Run cargo check**

Run: `cargo check`
Expected: compiles (with stub functions).

- [ ] **Step 6: Commit**

```bash
git add src/ui/fleet_builder.rs src/ui/mod.rs
git commit -m "feat: fleet list panel — add/select ships with click handlers"
```

---

### Task 16: Ship detail panel — weapon slots, change/remove

**Files:**
- Modify: `src/ui/fleet_builder.rs`

- [ ] **Step 1: Add weapon slot markers**

```rust
#[derive(Component)]
pub struct ChangeWeaponButton { pub ship_index: usize, pub slot_index: usize }

#[derive(Component)]
pub struct RemoveWeaponButton { pub ship_index: usize, pub slot_index: usize }

#[derive(Component)]
pub struct WeaponPickerOption { pub weapon: WeaponType }
```

- [ ] **Step 2: Implement rebuild_ship_detail**

```rust
fn rebuild_ship_detail(
    mut commands: Commands,
    state: Res<FleetBuilderState>,
    panel_query: Query<Entity, With<ShipDetailPanel>>,
) {
    let Ok(panel) = panel_query.single() else { return };
    commands.entity(panel).despawn_descendants();

    let Some(idx) = state.selected_ship else {
        commands.entity(panel).with_children(|p| {
            p.spawn((
                Text::new("Select a ship to view details"),
                TextFont { font_size: 16.0, ..default() },
                TextColor(Color::srgb(0.5, 0.5, 0.5)),
            ));
        });
        return;
    };

    let Some(spec) = state.ships.get(idx) else { return };
    let layout = spec.class.mount_layout();

    commands.entity(panel).with_children(|parent| {
        // Ship header
        parent.spawn((
            Text::new(format!("{:?} ({} pts hull)", spec.class, fleet::hull_cost(spec.class))),
            TextFont { font_size: 18.0, ..default() },
            TextColor(Color::WHITE),
        ));

        // Mount slots
        for (slot_idx, ((size, _offset), weapon_opt)) in
            layout.iter().zip(spec.loadout.iter()).enumerate()
        {
            let size_label = format!("{:?}", size);
            let weapon_label = match weapon_opt {
                Some(w) => format!("{:?} ({} pts)", w, fleet::weapon_cost(*w)),
                None => "Empty".to_string(),
            };

            parent
                .spawn(Node {
                    width: Val::Percent(100.0),
                    padding: UiRect::all(Val::Px(6.0)),
                    justify_content: JustifyContent::SpaceBetween,
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(10.0),
                    ..default()
                })
                .with_children(|row| {
                    // Slot info
                    row.spawn((
                        Text::new(format!("Slot {} [{}]  {}", slot_idx + 1, size_label, weapon_label)),
                        TextFont { font_size: 14.0, ..default() },
                        TextColor(Color::WHITE),
                    ));

                    // Buttons
                    row.spawn(Node {
                        column_gap: Val::Px(5.0),
                        ..default()
                    })
                    .with_children(|btns| {
                        btns.spawn((
                            ChangeWeaponButton { ship_index: idx, slot_index: slot_idx },
                            Button,
                            Node { padding: UiRect::axes(Val::Px(10.0), Val::Px(4.0)), ..default() },
                            BackgroundColor(Color::srgb(0.3, 0.3, 0.5)),
                        ))
                        .with_child((
                            Text::new("Change"),
                            TextFont { font_size: 12.0, ..default() },
                            TextColor(Color::WHITE),
                        ));

                        if weapon_opt.is_some() {
                            btns.spawn((
                                RemoveWeaponButton { ship_index: idx, slot_index: slot_idx },
                                Button,
                                Node { padding: UiRect::axes(Val::Px(10.0), Val::Px(4.0)), ..default() },
                                BackgroundColor(Color::srgb(0.5, 0.2, 0.2)),
                            ))
                            .with_child((
                                Text::new("Remove"),
                                TextFont { font_size: 12.0, ..default() },
                                TextColor(Color::WHITE),
                            ));
                        }
                    });
                });
        }

        // Remove Ship button
        parent
            .spawn((
                RemoveShipButton(idx),
                Button,
                Node {
                    padding: UiRect::axes(Val::Px(15.0), Val::Px(8.0)),
                    margin: UiRect::top(Val::Px(10.0)),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.6, 0.15, 0.15)),
            ))
            .with_child((
                Text::new("Remove Ship"),
                TextFont { font_size: 14.0, ..default() },
                TextColor(Color::WHITE),
            ));
    });
}
```

- [ ] **Step 3: Implement weapon/remove click handlers**

```rust
fn handle_change_weapon_click(
    interaction_query: Query<(&Interaction, &ChangeWeaponButton), Changed<Interaction>>,
    mut state: ResMut<FleetBuilderState>,
) {
    for (interaction, btn) in &interaction_query {
        if *interaction == Interaction::Pressed {
            state.popup = Some(PopupKind::ChangeWeapon {
                ship_index: btn.ship_index,
                slot_index: btn.slot_index,
            });
        }
    }
}

fn handle_remove_weapon_click(
    interaction_query: Query<(&Interaction, &RemoveWeaponButton), Changed<Interaction>>,
    mut state: ResMut<FleetBuilderState>,
) {
    for (interaction, btn) in &interaction_query {
        if *interaction == Interaction::Pressed {
            if let Some(spec) = state.ships.get_mut(btn.ship_index) {
                if let Some(slot) = spec.loadout.get_mut(btn.slot_index) {
                    *slot = None;
                }
            }
        }
    }
}

fn handle_remove_ship_click(
    interaction_query: Query<(&Interaction, &RemoveShipButton), Changed<Interaction>>,
    mut state: ResMut<FleetBuilderState>,
) {
    for (interaction, btn) in &interaction_query {
        if *interaction == Interaction::Pressed {
            if btn.0 < state.ships.len() {
                state.ships.remove(btn.0);
                // Fix selected index
                if state.selected_ship == Some(btn.0) {
                    state.selected_ship = None;
                } else if let Some(ref mut sel) = state.selected_ship {
                    if *sel > btn.0 {
                        *sel -= 1;
                    }
                }
            }
        }
    }
}
```

- [ ] **Step 4: Run cargo check**

Run: `cargo check`
Expected: compiles.

- [ ] **Step 5: Commit**

```bash
git add src/ui/fleet_builder.rs
git commit -m "feat: ship detail panel — weapon slots with change/remove buttons"
```

---

### Task 17: Popup system — ship picker and weapon picker

**Files:**
- Modify: `src/ui/fleet_builder.rs`

- [ ] **Step 1: Implement spawn_popup**

```rust
fn spawn_popup(
    mut commands: Commands,
    state: Res<FleetBuilderState>,
    existing_popups: Query<Entity, With<PopupOverlay>>,
) {
    // Only rebuild if state changed
    if !state.is_changed() {
        return;
    }

    // Despawn existing popups
    for entity in &existing_popups {
        commands.entity(entity).despawn();
    }

    let Some(ref popup) = state.popup else { return };

    commands
        .spawn((
            PopupOverlay,
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.5)),
            GlobalZIndex(10),
        ))
        .with_children(|overlay| {
            overlay
                .spawn((
                    Node {
                        width: Val::Px(300.0),
                        flex_direction: FlexDirection::Column,
                        padding: UiRect::all(Val::Px(15.0)),
                        row_gap: Val::Px(8.0),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.1, 0.1, 0.15)),
                ))
                .with_children(|panel| {
                    match popup {
                        PopupKind::AddShip => {
                            panel.spawn((
                                Text::new("Select Ship Class"),
                                TextFont { font_size: 18.0, ..default() },
                                TextColor(Color::WHITE),
                            ));
                            for class in [ShipClass::Battleship, ShipClass::Destroyer, ShipClass::Scout] {
                                let cost = fleet::hull_cost(class);
                                panel
                                    .spawn((
                                        ShipPickerOption(class),
                                        Button,
                                        Node {
                                            width: Val::Percent(100.0),
                                            padding: UiRect::all(Val::Px(10.0)),
                                            justify_content: JustifyContent::SpaceBetween,
                                            ..default()
                                        },
                                        BackgroundColor(Color::srgb(0.2, 0.2, 0.3)),
                                    ))
                                    .with_children(|row| {
                                        row.spawn((
                                            Text::new(format!("{:?}", class)),
                                            TextFont { font_size: 16.0, ..default() },
                                            TextColor(Color::WHITE),
                                        ));
                                        row.spawn((
                                            Text::new(format!("{} pts", cost)),
                                            TextFont { font_size: 14.0, ..default() },
                                            TextColor(Color::srgb(0.7, 0.7, 0.7)),
                                        ));
                                    });
                            }
                        }
                        PopupKind::ChangeWeapon { ship_index, slot_index } => {
                            if let Some(spec) = state.ships.get(*ship_index) {
                                let layout = spec.class.mount_layout();
                                if let Some((slot_size, _)) = layout.get(*slot_index) {
                                    panel.spawn((
                                        Text::new(format!("Select Weapon ({:?} slot)", slot_size)),
                                        TextFont { font_size: 18.0, ..default() },
                                        TextColor(Color::WHITE),
                                    ));

                                    let all_weapons = [
                                        WeaponType::HeavyCannon,
                                        WeaponType::Railgun,
                                        WeaponType::HeavyVLS,
                                        WeaponType::Cannon,
                                        WeaponType::LightVLS,
                                        WeaponType::LaserPD,
                                        WeaponType::CWIS,
                                    ];

                                    for weapon in all_weapons {
                                        if slot_size.fits(weapon.mount_size()) {
                                            let cost = fleet::weapon_cost(weapon);
                                            panel
                                                .spawn((
                                                    WeaponPickerOption { weapon },
                                                    Button,
                                                    Node {
                                                        width: Val::Percent(100.0),
                                                        padding: UiRect::all(Val::Px(8.0)),
                                                        justify_content: JustifyContent::SpaceBetween,
                                                        ..default()
                                                    },
                                                    BackgroundColor(Color::srgb(0.2, 0.2, 0.3)),
                                                ))
                                                .with_children(|row| {
                                                    row.spawn((
                                                        Text::new(format!("{:?}", weapon)),
                                                        TextFont { font_size: 14.0, ..default() },
                                                        TextColor(Color::WHITE),
                                                    ));
                                                    row.spawn((
                                                        Text::new(format!("{} pts", cost)),
                                                        TextFont { font_size: 12.0, ..default() },
                                                        TextColor(Color::srgb(0.7, 0.7, 0.7)),
                                                    ));
                                                });
                                        }
                                    }
                                }
                            }
                        }
                    }
                });
        });
}
```

- [ ] **Step 2: Implement weapon picker click handler**

```rust
fn handle_weapon_picker_click(
    interaction_query: Query<(&Interaction, &WeaponPickerOption), Changed<Interaction>>,
    mut state: ResMut<FleetBuilderState>,
) {
    for (interaction, option) in &interaction_query {
        if *interaction == Interaction::Pressed {
            if let Some(PopupKind::ChangeWeapon { ship_index, slot_index }) = state.popup {
                if let Some(spec) = state.ships.get_mut(ship_index) {
                    if let Some(slot) = spec.loadout.get_mut(slot_index) {
                        *slot = Some(option.weapon);
                    }
                }
            }
            state.popup = None;
        }
    }
}
```

- [ ] **Step 3: Run cargo check**

Run: `cargo check`
Expected: compiles.

- [ ] **Step 4: Commit**

```bash
git add src/ui/fleet_builder.rs
git commit -m "feat: popup system — ship class picker and weapon picker"
```

---

### Task 18: Submit/cancel, budget display, status text

**Files:**
- Modify: `src/ui/fleet_builder.rs`

- [ ] **Step 1: Implement submit and cancel handlers**

```rust
#[derive(Component)]
pub struct CancelButton;

fn handle_submit_click(
    interaction_query: Query<&Interaction, (Changed<Interaction>, With<SubmitButton>)>,
    mut state: ResMut<FleetBuilderState>,
    mut commands: Commands,
) {
    for interaction in &interaction_query {
        if *interaction == Interaction::Pressed && !state.submitted {
            let cost = fleet_cost(&state.ships);
            if cost <= FLEET_BUDGET && !state.ships.is_empty() {
                state.submitted = true;
                commands.client_trigger(FleetSubmission {
                    ships: state.ships.clone(),
                });
            }
        }
    }
}

fn handle_cancel_click(
    interaction_query: Query<&Interaction, (Changed<Interaction>, With<CancelButton>)>,
    mut state: ResMut<FleetBuilderState>,
    mut commands: Commands,
) {
    for interaction in &interaction_query {
        if *interaction == Interaction::Pressed && state.submitted {
            state.submitted = false;
            commands.client_trigger(CancelSubmission);
        }
    }
}
```

Note: The submit button should change to a cancel button when submitted. Handle this in `rebuild_fleet_list` or a dedicated bottom-bar rebuild system.

- [ ] **Step 2: Implement budget and status text updates**

```rust
fn update_budget_text(
    state: Res<FleetBuilderState>,
    mut query: Query<(&mut Text, &mut TextColor), With<BudgetText>>,
) {
    let cost = fleet_cost(&state.ships);
    for (mut text, mut color) in &mut query {
        **text = format!("Budget: {} / {}", cost, FLEET_BUDGET);
        color.0 = if cost > FLEET_BUDGET {
            Color::srgb(1.0, 0.3, 0.3) // Red when over budget
        } else {
            Color::WHITE
        };
    }
}

fn update_status_text(
    lobby_state: Res<CurrentLobbyState>,
    fleet_state: Res<FleetBuilderState>,
    mut query: Query<&mut Text, With<StatusText>>,
) {
    let text = match &lobby_state.0 {
        Some(LobbyState::Composing) => "Composing...".to_string(),
        Some(LobbyState::WaitingForOpponent) => "Waiting for opponent to connect...".to_string(),
        Some(LobbyState::OpponentComposing) => "Waiting for opponent to submit...".to_string(),
        Some(LobbyState::Countdown(t)) => format!("Starting in {:.0}...", t.ceil()),
        Some(LobbyState::Rejected(reason)) => format!("Rejected: {}", reason),
        None => {
            if fleet_state.submitted {
                "Submitted, waiting...".to_string()
            } else {
                "Build your fleet".to_string()
            }
        }
    };

    for mut t in &mut query {
        **t = text.clone();
    }
}
```

- [ ] **Step 3: Update submit button appearance based on state**

Add a system to gray out the submit button when over budget or fleet is empty, and swap to cancel button when submitted:

```rust
fn update_submit_button(
    state: Res<FleetBuilderState>,
    mut query: Query<(&mut BackgroundColor, &Children), With<SubmitButton>>,
    mut text_query: Query<&mut Text>,
) {
    let cost = fleet_cost(&state.ships);
    let can_submit = cost <= FLEET_BUDGET && !state.ships.is_empty() && !state.submitted;

    for (mut bg, children) in &mut query {
        bg.0 = if state.submitted {
            Color::srgb(0.5, 0.3, 0.2) // Orange-ish for cancel
        } else if can_submit {
            Color::srgb(0.2, 0.5, 0.2) // Green
        } else {
            Color::srgb(0.3, 0.3, 0.3) // Gray
        };

        // Update button text
        for &child in children.iter() {
            if let Ok(mut text) = text_query.get_mut(child) {
                **text = if state.submitted {
                    "Cancel & Edit".to_string()
                } else {
                    "Submit Fleet".to_string()
                };
            }
        }
    }
}
```

Modify the submit button click handler to also act as cancel when submitted:

```rust
fn handle_submit_click(
    interaction_query: Query<&Interaction, (Changed<Interaction>, With<SubmitButton>)>,
    mut state: ResMut<FleetBuilderState>,
    mut commands: Commands,
) {
    for interaction in &interaction_query {
        if *interaction == Interaction::Pressed {
            if state.submitted {
                // Act as cancel
                state.submitted = false;
                commands.client_trigger(CancelSubmission);
            } else {
                // Act as submit
                let cost = fleet_cost(&state.ships);
                if cost <= FLEET_BUDGET && !state.ships.is_empty() {
                    state.submitted = true;
                    commands.client_trigger(FleetSubmission {
                        ships: state.ships.clone(),
                    });
                }
            }
        }
    }
}
```

(Remove the separate `CancelButton` component and `handle_cancel_click` — the submit button toggles.)

- [ ] **Step 4: Run cargo check**

Run: `cargo check`
Expected: compiles.

- [ ] **Step 5: Run full test suite**

Run: `cargo test`
Expected: all existing + new tests pass.

- [ ] **Step 6: Commit**

```bash
git add src/ui/fleet_builder.rs src/ui/mod.rs
git commit -m "feat: submit/cancel toggle, budget display, lobby status text"
```

---

## Chunk 5: Integration & Polish

### Task 19: End-to-end integration test (manual)

**Files:** None (manual testing)

- [ ] **Step 1: Run server + client**

```bash
cargo run --bin server &
cargo run --bin client &
cargo run --bin client -- --connect 127.0.0.1:5000 &
```

- [ ] **Step 2: Verify flow**

1. Both clients connect → both enter FleetComposition screen
2. Add ships, assign weapons, verify budget display
3. Submit one fleet → status shows "waiting for opponent"
4. Cancel → back to composing
5. Re-submit both → countdown 3...2...1 → Playing state
6. Ships spawn with correct loadouts (check weapon types in game)

- [ ] **Step 3: Fix any issues found**

- [ ] **Step 4: Commit any fixes**

---

### Task 20: Update CLAUDE.md

**Files:**
- Modify: `CLAUDE.md`

- [ ] **Step 1: Update architecture section**

Add `src/fleet/` and `src/ui/` module descriptions. Update GameState flow diagram. Update test count. Mark Phase 3c as complete in the roadmap section.

- [ ] **Step 2: Commit**

```bash
git add CLAUDE.md
git commit -m "docs: update CLAUDE.md for Phase 3c — fleet composition, UI, lobby"
```
