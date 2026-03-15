# Phase 3a: Mount Points & Cannons — Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ships can target enemies and fire cannons — three weapon types, simulated projectile entities, damage, destruction, and a win condition.

**Architecture:** New `weapon/` module handles mounts, firing, projectiles, and damage as server-side systems. Projectiles are replicated entities. Targeting uses K-key mode with existing command channel pattern. Client materializes projectile meshes and destruction visuals.

**Tech Stack:** Bevy 0.18, bevy_replicon 0.39, existing command/trigger patterns.

**Design doc:** `docs/plans/2026-03-15-phase3a-weapons-design.md`

---

## Chunk 1: Foundation — Types & Mounts

### Task 1: Create weapon module with core types

**Files:**
- Create: `src/weapon/mod.rs`
- Create: `src/weapon/projectile.rs` (empty)
- Create: `src/weapon/firing.rs` (empty)
- Create: `src/weapon/damage.rs` (empty)
- Modify: `src/lib.rs`

- [ ] **Step 1: Create `src/weapon/mod.rs` with mount and weapon types**

Define the following types:
- `MountSize` enum: `Large`, `Medium`, `Small` (with Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)
- `WeaponType` enum: `HeavyCannon`, `Cannon`, `Railgun` (same derives)
- `FiringArc` enum: `Turret` (360°), `Forward` (ship-facing only, ±10°)
- `WeaponProfile` struct (not a component, just data): `fire_rate_secs: f32`, `burst_count: u8`, `damage: u16`, `firing_range: f32`, `projectile_speed: f32`, `spread_degrees: f32`, `arc: FiringArc`, `max_ammo: u16`
- `WeaponType::profile(&self) -> WeaponProfile` method returning the stats from the design doc
- `WeaponType::mount_size(&self) -> MountSize` method
- `WeaponState` struct: `weapon_type: WeaponType`, `ammo: u16`, `cooldown: f32` (time remaining until next shot)
- `Mount` struct: `size: MountSize`, `offset: Vec2`, `weapon: Option<WeaponState>`
- `Mounts` component: wraps `Vec<Mount>` (with Serialize, Deserialize, Clone, Debug, Component)

- [ ] **Step 2: Add `pub mod weapon;` to `src/lib.rs`**

Also add submodule declarations in `weapon/mod.rs`: `pub mod projectile;`, `pub mod firing;`, `pub mod damage;` — create these as empty files for now.

- [ ] **Step 3: Write tests for weapon profiles**

In `src/weapon/mod.rs`, add `#[cfg(test)]` tests:
- `heavy_cannon_profile_values`: verify damage=15, burst=3, fire_rate=3.0, range=300.0, arc=Turret
- `cannon_profile_values`: verify damage=8, burst=1, fire_rate=1.0, range=200.0, arc=Turret
- `railgun_profile_values`: verify damage=50, burst=1, fire_rate=7.0, range=1000.0, arc=Forward
- `weapon_type_mount_size`: verify HeavyCannon and Railgun are Large, Cannon is Medium

- [ ] **Step 4: Verify**

Run: `cargo check && cargo test`

- [ ] **Step 5: Commit**

Commit message: "feat: weapon module with mount, weapon type, and profile definitions"

---

### Task 2: Add mounts to ship spawning

**Files:**
- Modify: `src/ship/mod.rs`
- Modify: `src/net/server.rs`
- Modify: `src/net/client.rs`

- [ ] **Step 1: Define default mount layouts per ship class**

Add a method `ShipClass::default_mounts(&self) -> Vec<Mount>` that returns the mount layout with default weapons:
- Battleship: 2 large (heavy cannon), 2 medium (empty), 2 small (empty)
- Destroyer: 1 large (heavy cannon), 2 medium (empty), 1 small (empty)
- Scout: 1 medium (cannon), 1 small (empty)

Mount offsets: spread weapons along the ship's length. Use simple symmetric offsets (e.g., large mounts at ±8.0 on X for battleship). Exact positions are cosmetic and can be tuned later.

- [ ] **Step 2: Add Mounts to `spawn_server_ship`**

In `spawn_server_ship`, add `Mounts(class.default_mounts())` to the spawned entity's component bundle.

- [ ] **Step 3: Register Mounts for replication on both server and client**

In `src/net/server.rs` `ServerNetPlugin::build` and `src/net/client.rs` `ClientNetPlugin::build`, add `app.replicate::<Mounts>()`.

- [ ] **Step 4: Write test for default mount counts**

Test that each class returns the expected number of mounts and the correct weapon types are filled.

- [ ] **Step 5: Verify**

Run: `cargo check --bin server && cargo check --bin client && cargo test`

- [ ] **Step 6: Commit**

Commit message: "feat: add mount points with default weapon loadouts to ships"

---

### Task 3: Change Health from u8 to u16 and set class HP

**Files:**
- Modify: `src/game/mod.rs`
- Modify: `src/ship/mod.rs`

- [ ] **Step 1: Change Health.hp to u16**

In `src/game/mod.rs`, change `pub hp: u8` to `pub hp: u16`. Update the existing tests that reference `hp` to use u16 values.

- [ ] **Step 2: Add HP values to ShipProfile**

Add `pub hp: u16` to `ShipProfile`. Set: Battleship=200, Destroyer=100, Scout=50.

- [ ] **Step 3: Update spawn_server_ship to use class HP**

Change `Health { hp: 3 }` to `Health { hp: class.profile().hp }`.

- [ ] **Step 4: Update existing Health tests**

Adjust `health_takes_damage` and `health_saturates_at_zero` tests for u16.

- [ ] **Step 5: Verify**

Run: `cargo test`

- [ ] **Step 6: Commit**

Commit message: "feat: Health as u16 with class-specific HP pools"

---

## Chunk 2: Targeting

### Task 4: Add targeting commands and TargetDesignation component

**Files:**
- Modify: `src/net/commands.rs`
- Modify: `src/ship/mod.rs`

- [ ] **Step 1: Add TargetCommand and ClearTargetCommand**

In `src/net/commands.rs`, add:
- `TargetCommand { ship: Entity, target: Entity }` — derives Event, Serialize, Deserialize, MapEntities, Debug, Clone. Both `ship` and `target` fields need `#[entities]`.
- `ClearTargetCommand { ship: Entity }` — derives Event, Serialize, Deserialize, MapEntities, Debug, Clone. `ship` field needs `#[entities]`.

- [ ] **Step 2: Add TargetDesignation component**

In `src/ship/mod.rs`, add:
- `TargetDesignation(pub Entity)` — derives Component, Serialize, Deserialize, MapEntities, Debug, Clone. Entity field needs `#[entities]`.

This component is placed on Ship entities on the server and synced to ShipSecrets for per-team visibility.

- [ ] **Step 3: Verify**

Run: `cargo check`

- [ ] **Step 4: Commit**

Commit message: "feat: add targeting commands and TargetDesignation component"

---

### Task 5: Register targeting in server and client

**Files:**
- Modify: `src/net/server.rs`
- Modify: `src/net/client.rs`

- [ ] **Step 1: Register TargetCommand and ClearTargetCommand triggers**

In both `ServerNetPlugin` and `ClientNetPlugin`, register:
- `app.add_mapped_client_event::<TargetCommand>(Channel::Ordered)`
- `app.add_mapped_client_event::<ClearTargetCommand>(Channel::Ordered)`

- [ ] **Step 2: Register TargetDesignation for replication**

Add `app.replicate::<TargetDesignation>()` on both server and client. This replicates on the ShipSecrets entity (owning team only).

- [ ] **Step 3: Add target command handlers on server**

Add observers in `ServerNetPlugin::build`:
- `handle_target_command`: validates ship ownership, checks target exists and is an enemy ship (different Team), inserts `TargetDesignation(target)` on the ship entity
- `handle_clear_target_command`: validates ownership, removes `TargetDesignation` from the ship entity

- [ ] **Step 4: Sync TargetDesignation to ShipSecrets**

In `sync_ship_secrets`, add sync for `TargetDesignation`: if the Ship has it, insert on ShipSecrets; if not, remove from ShipSecrets.

- [ ] **Step 5: Add target visibility clearing**

In the `server_update_visibility` system (or a new system that runs alongside it), check each ship with `TargetDesignation`: if the target ship is no longer visible to the ship's team (not in LOS), remove `TargetDesignation`. This ensures targeting clears automatically when the enemy escapes detection.

- [ ] **Step 6: Verify**

Run: `cargo check --bin server && cargo check --bin client`

- [ ] **Step 7: Commit**

Commit message: "feat: register targeting triggers, server handlers, and visibility clearing"

---

### Task 6: K-key targeting input and L-key input change

**Files:**
- Modify: `src/input/mod.rs`

- [ ] **Step 1: Add TargetMode resource**

Similar to `LockMode`, add `TargetMode(pub bool)` resource. Init in `InputPlugin::build`.

- [ ] **Step 2: Add K-key handling in `handle_keyboard`**

When K is pressed:
- If selected ship has a target (check `TargetDesignation` on the ship's `ShipSecrets` entity via query — ShipSecrets is what the client receives for own-team ships): fire `ClearTargetCommand` trigger and don't enter target mode
- Else: toggle `TargetMode` on

- [ ] **Step 3: Handle target click in `on_ship_clicked`**

When `TargetMode` is active and left-click on enemy ship:
- Fire `TargetCommand { ship: selected_ship, target: clicked_enemy }`
- Reset `TargetMode` to false

- [ ] **Step 4: Change L-key to L + left-click pattern**

Currently L toggles LockMode which uses right-click. Change so:
- L enters LockMode
- Left-click on ground while in LockMode sets facing direction (fires FacingLockCommand), then resets LockMode
- This replaces the old right-click-in-lock-mode behavior
- Alt+right-click on ground still works as an alternative shortcut

- [ ] **Step 5: Add K-mode HUD text**

Similar to lock mode HUD: "TARGET MODE — Click enemy to designate" when TargetMode is active.

- [ ] **Step 6: Verify**

Run: `cargo check --bin client && cargo test`

- [ ] **Step 7: Commit**

Commit message: "feat: K-key targeting mode and L-key input change to left-click"

---

## Chunk 3: Projectiles

### Task 7: Projectile types and spawning

**Files:**
- Create: `src/weapon/projectile.rs` (replace empty file)

- [ ] **Step 1: Define projectile components**

In `src/weapon/projectile.rs`, define:
- `Projectile` marker component (Serialize, Deserialize, Component)
- `ProjectileVelocity(pub Vec3)` — direction × speed, constant (Serialize, Deserialize, Component, Clone)
- `ProjectileDamage(pub u16)` — HP to subtract on hit (Serialize, Deserialize, Component, Clone)
- `ProjectileOwner(pub Entity)` — the ship that fired, for self-hit skip (NOT friendly fire skip — friendly fire IS possible). Derives Serialize, Deserialize, Component, MapEntities, Clone, with `#[entities]` on the Entity field.

- [ ] **Step 2: Add `spawn_projectile` helper function**

A function that takes: `commands`, `origin: Vec3`, `direction: Vec3`, `speed: f32`, `damage: u16`, `owner: Entity` and spawns a projectile entity with all components + `Replicated` marker + `Transform`.

- [ ] **Step 3: Write tests**

Test `spawn_projectile` creates entity with correct components (World-level test).

- [ ] **Step 4: Verify**

Run: `cargo test`

- [ ] **Step 5: Commit**

Commit message: "feat: projectile components and spawn helper"

---

### Task 8: Projectile server systems

**Files:**
- Modify: `src/weapon/projectile.rs`
- Modify: `src/bin/server.rs`

- [ ] **Step 1: Implement `advance_projectiles` system**

Each tick: for each Projectile entity, advance `Transform.translation += velocity.0 * dt`. No drag on projectiles — they fly straight.

- [ ] **Step 2: Implement `check_projectile_bounds` system**

Each tick: for each Projectile entity, if position X or Z is outside `MapBounds` half_extents, despawn the projectile.

- [ ] **Step 3: Implement `check_projectile_hits` system**

Each tick: for each Projectile entity, check distance to every Ship entity (that is not Destroyed). If distance < ship's `collision_radius` AND the ship entity is not the `ProjectileOwner`: subtract `ProjectileDamage` from `Health.hp` (saturating at 0), despawn projectile. Only hit one ship per projectile — break after first hit.

- [ ] **Step 4: Write tests**

World-level tests:
- Projectile advances position by velocity × dt
- Projectile outside bounds is despawned

- [ ] **Step 5: Create `ProjectilePlugin`**

Register the three systems chained in Update during `GameState::Playing`. Register `Projectile`, `ProjectileVelocity`, `ProjectileDamage`, `ProjectileOwner` for replication on server.

- [ ] **Step 6: Add `ProjectilePlugin` to server binary**

- [ ] **Step 7: Verify**

Run: `cargo check --bin server && cargo test`

- [ ] **Step 8: Commit**

Commit message: "feat: projectile advancement, bounds checking, and hit detection"

---

## Chunk 4: Firing

### Task 9: Auto-fire system and weapon cooldown

**Files:**
- Create: `src/weapon/firing.rs` (replace empty file)
- Modify: `src/net/server.rs`

- [ ] **Step 1: Implement `tick_weapon_cooldowns` system**

A standalone system that decrements cooldown timers on ALL weapon mounts for ALL ships, every frame, regardless of whether the ship has a target. This ensures fire rate is respected across target switches. Query all `Mounts` components, for each mount with a weapon, decrement `cooldown` by `dt`, clamping at 0.

- [ ] **Step 2: Implement lead calculation pure function**

`compute_lead_position(shooter_pos: Vec3, target_pos: Vec3, target_velocity: Vec2, projectile_speed: f32) -> Vec3`

Predicts where the target will be when the projectile arrives. Uses iterative approximation: estimate travel time from current distance, predict target position at that time, repeat once for refinement.

- [ ] **Step 3: Implement `is_in_firing_arc` pure function**

`is_in_firing_arc(ship_facing: Vec2, target_direction: Vec2, arc: FiringArc) -> bool`

For `Turret`: always returns true. For `Forward`: returns true if angle between ship facing and target direction is ≤ 10°.

- [ ] **Step 4: Implement `auto_fire` system**

For each Ship with a `TargetDesignation`:
1. Check target still exists (query Ships). If not, remove `TargetDesignation`.
2. Get target's Transform and Velocity (for lead calculation).
3. For each mount with a weapon in `Mounts` where cooldown == 0 and ammo > 0:
   a. Check range: distance to target ≤ weapon firing_range
   b. Check arc via `is_in_firing_arc`
   c. If all pass: compute lead position, add random spread (weapon.spread_degrees converted to radians, applied as rotation around up axis), spawn projectile(s) per burst_count with small offset between burst rounds. Decrement ammo. Reset cooldown to fire_rate_secs.

- [ ] **Step 5: Write tests for lead calculation**

Pure function tests:
- Stationary target: lead position equals target position
- Moving target: lead position is ahead of target in movement direction

- [ ] **Step 6: Write test for firing arc check**

Test that Forward arc rejects targets outside ±10° cone and accepts targets within it. Turret arc always accepts.

- [ ] **Step 7: Register systems in server**

Add `tick_weapon_cooldowns` and `auto_fire` to the server's Update schedule during `GameState::Playing`. `tick_weapon_cooldowns` runs early (before auto_fire). `auto_fire` runs after `sync_ship_secrets`.

- [ ] **Step 8: Verify**

Run: `cargo check --bin server && cargo test`

- [ ] **Step 9: Commit**

Commit message: "feat: auto-fire system with lead calculation, arc checking, and cooldown ticking"

---

## Chunk 5: Damage & Destruction

### Task 10: Ship destruction, cleanup, and win condition

**Files:**
- Create: `src/weapon/damage.rs` (replace empty file)
- Modify: `src/game/mod.rs`
- Modify: `src/net/commands.rs`
- Modify: `src/net/server.rs`
- Modify: `src/net/client.rs`

- [ ] **Step 1: Add Destroyed marker, GameOver state, and GameResult event**

In `src/game/mod.rs`:
- Add `Destroyed` marker component (server-only, no serde)
- Add `GameOver` variant to `GameState` enum

In `src/net/commands.rs`:
- Add `GameResult { winning_team: Team }` — server→client event (Event, Serialize, Deserialize, Debug, Clone). Register with `add_server_event` on both server and client.

- [ ] **Step 2: Implement destruction system with delayed despawn**

In `src/weapon/damage.rs`, create `mark_destroyed` system:
- Query ships where `Health.hp == 0` and no `Destroyed` marker
- Insert `Destroyed` marker and a `DestroyTimer(Timer::from_seconds(1.0, TimerMode::Once))` component
- Log the destruction

Create `despawn_destroyed` system:
- Query ships with `Destroyed` and `DestroyTimer`
- Tick the timer. When finished:
  - Find and despawn the ship's ShipSecrets entity (query ShipSecretsOwner)
  - Despawn the ship entity (replicon handles client-side removal, ghost fade-out observer fires automatically)

The 1-second delay allows the client to see the destroyed ship before it disappears.

- [ ] **Step 3: Implement win condition check**

Create `check_win_condition` system:
- For each team (0 and 1), count ships that exist and are NOT `Destroyed`
- If either team has 0 non-destroyed ships:
  - Determine winning team
  - Send `GameResult { winning_team }` to all clients via `server_trigger(ToClients { mode: SendMode::Broadcast, ... })`
  - Transition to `GameOver`

- [ ] **Step 4: Add GameOver handling on client**

Add observer for `GameResult` event on client. Store the result. A system that runs on `OnEnter(GameState::GameOver)` spawns UI text: "Victory!" if `winning_team` matches `LocalTeam`, or "Defeat" otherwise.

- [ ] **Step 5: Register systems and events**

Create a `DamagePlugin` that runs `mark_destroyed`, `despawn_destroyed`, and `check_win_condition` in sequence during `GameState::Playing`, after projectile systems.

Register `GameResult` event with `add_server_event` on both server and client.

- [ ] **Step 6: Verify**

Run: `cargo check --bin server && cargo check --bin client && cargo test`

- [ ] **Step 7: Commit**

Commit message: "feat: ship destruction with delay, ShipSecrets cleanup, win/lose condition"

---

## Chunk 6: Client Visuals & Integration

### Task 11: Projectile materializer and targeting indicator

**Files:**
- Modify: `src/net/materializer.rs`
- Modify: `src/net/client.rs`

- [ ] **Step 1: Add projectile materializer**

In `src/net/materializer.rs`, add `materialize_projectiles` system:
- Watch for `Added<Projectile>` entities
- Spawn a small glowing sphere mesh as child (radius ~0.5, bright yellow/orange, emissive, unlit)
- Add `Visibility::Visible`

- [ ] **Step 2: Register projectile materializer and replication on client**

In `ClientNetPlugin`:
- Register `Projectile`, `ProjectileVelocity`, `ProjectileDamage`, `ProjectileOwner` for replication
- Add `materialize_projectiles` to Update systems during Playing state

- [ ] **Step 3: Add targeting indicator system**

Create a system that reads `TargetDesignation` from `ShipSecrets` entities and renders a small diamond/ring indicator on the targeted enemy ship. Use the existing indicator asset pattern (cached mesh/material resource). Only shows for own-team ships. Indicator despawns and re-creates each frame (same pattern as waypoint markers).

Note: the existing ghost entity fade-out via `On<Remove, Ship>` observer in `fog/mod.rs` already handles the death visual. Destroyed ships will use the same ghost fade-out as ships losing visibility, which is adequate for 3a.

- [ ] **Step 4: Verify**

Run: `cargo check --bin client && cargo test`

- [ ] **Step 5: Commit**

Commit message: "feat: projectile and targeting indicator visuals on client"

---

### Task 12: Integration and final wiring

**Files:**
- Modify: `src/bin/server.rs`
- Modify: `src/bin/client.rs`

- [ ] **Step 1: Ensure all weapon plugins are added to server binary**

Verify `ProjectilePlugin` and `DamagePlugin` are in the server app. If Task 8 already added ProjectilePlugin, ensure DamagePlugin is also present. Consider a `WeaponServerPlugin` that groups `ProjectilePlugin` + `DamagePlugin` + `tick_weapon_cooldowns` + `auto_fire` for cleaner server binary setup.

- [ ] **Step 2: Verify full compilation**

Run: `cargo check --bin server && cargo check --bin client && cargo test`

- [ ] **Step 3: Manual integration test**

Start server + two clients. Verify:
- Ships have weapons (mounts data exists)
- K key enters target mode, click enemy designates target
- Selected ship fires when enemy is in range
- Projectiles visible as small glowing spheres
- Projectiles hit enemies, HP decreases
- Ship at 0 HP shows briefly, then despawns with ghost fade-out
- When all enemy ships destroyed, "Victory!" or "Defeat" text appears
- Railgun only fires when ship faces the target
- Friendly fire works (projectiles can hit your own ships)
- L + left-click sets facing direction

- [ ] **Step 4: Commit any fixes**

Commit message: "feat: Phase 3a integration — ships fire cannons"

- [ ] **Step 5: Update CLAUDE.md**

Add Phase 3a to the architecture section: weapon module, targeting system, projectile simulation, win condition. Mark Phase 3a as complete in roadmap.

- [ ] **Step 6: Commit**

Commit message: "docs: update CLAUDE.md for Phase 3a completion"
