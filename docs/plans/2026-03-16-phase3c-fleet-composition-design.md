# Phase 3c: Fleet Composition Screen — Design

Pre-game loadout screen where players build fleets from a point budget, choose
ship classes, and assign weapons to mount slots before the match begins.

---

## Game Flow & States

### Client flow

```
Setup → Connecting → FleetComposition → Playing → GameOver
```

- `Connecting → FleetComposition`: immediately on receiving `TeamAssignment`.
  Player does not wait for the other client to connect.
- Client shows fleet builder UI. Player adds ships, assigns weapons, submits.
- `LobbyStatus` server events update the client on opponent state and countdown.
- `FleetComposition → Playing`: when server broadcasts countdown completion.

### Server flow

```
Setup → WaitingForPlayers → Playing → GameOver
```

- Server stays in `WaitingForPlayers` for the entire fleet composition phase.
- `TeamAssignment` fires on `AuthorizedClient` add (moved earlier from current
  `Playing` entry).
- Server tracks submissions via a `LobbyTracker` resource.
- When both clients have submitted: 3-second countdown begins.
- If either client cancels during countdown: countdown resets, both notified.
- Countdown hits zero: server spawns fleets from submitted specs, spawns
  asteroids, transitions to `Playing`.

### Submit / cancel dance

- Player submits fleet → put on hold if opponent hasn't submitted yet.
- While on hold, player can cancel and re-edit before re-submitting.
- Both submitted → 3s countdown visible to both.
- Either backs out → countdown resets, both return to composing/waiting.
- Countdown completes → fleets locked, game begins.

---

## Point Budget & Costs

**Budget: 1000 points per team.**

### Hull costs

| Ship Class | Hull Cost | % of Budget | HP  | Mount Slots      |
|------------|-----------|-------------|-----|------------------|
| Battleship | 375       | 37.5%       | 200 | 2L, 2M, 2S      |
| Destroyer  | 150       | 15%         | 100 | 1L, 2M, 1S      |
| Scout      | 45        | 4.5%        | 50  | 1M, 1S          |

### Weapon costs

| Weapon      | Mount Size | Cost | Key Stats                          |
|-------------|-----------|------|------------------------------------|
| HeavyCannon | Large     | 30   | Turret, 3-burst, 15dmg, 300m      |
| Railgun     | Large     | 40   | Forward ±10°, 50dmg, 1000m        |
| HeavyVLS    | Large     | 35   | 8 tubes, 30dmg missiles, 500m     |
| Cannon      | Medium    | 15   | Turret, fast fire, 8dmg, 200m     |
| LightVLS    | Medium    | 20   | 4 tubes, 30dmg missiles, 500m     |
| LaserPD     | Medium    | 25   | PD beam tracking, 300m            |
| CWIS        | Small     | 10   | PD burst, 100m kill / 150m visual |

### Example builds (1000pt budget)

- **Balanced:** 1 BB fully loaded (530) + 1 DD fully loaded (240) + 1 Scout
  fully loaded (70) = 840. Room for one more scout.
- **Battleship brawl:** 2 BB (750) — only 250 left for weapons across both.
  Lots of empty slots.
- **Destroyer wolfpack:** 4 DD fully loaded (960). No battleship, no scouts.
- **Swarm:** 3 DD (450) + 4 Scouts (180) = 630 hull. Only 370 for weapons —
  can't arm everything.
- **Glass cannon:** 1 BB with Railgun + HeavyVLS only (450) + 2 DD fully
  loaded (480) = 930. Battleship has no PD.

### Budget rules

- Player can freely go over budget while composing.
- Submit button is disabled (grayed out) when over budget.
- No fleet size limits, no per-class limits — purely budget-constrained.
- Empty mount slots are allowed with no penalty.

---

## Mount Compatibility

Downsizing allowed: a mount slot accepts weapons of its size or smaller.

| Slot Size | Accepts                                                     |
|-----------|-------------------------------------------------------------|
| Large     | HeavyCannon, Railgun, HeavyVLS, Cannon, LightVLS, LaserPD, CWIS |
| Medium    | Cannon, LightVLS, LaserPD, CWIS                            |
| Small     | CWIS                                                        |

Ordering: Large (2) > Medium (1) > Small (0). Compatibility check:
`weapon.mount_size() <= slot.size`.

---

## Network Protocol

Three new network events following existing patterns (client triggers with
`MapEntities`, server events with `Channel::Ordered`).

### Client → Server

```
FleetSubmission {
    ships: Vec<ShipSpec>,
}

ShipSpec {
    class: ShipClass,
    loadout: Vec<Option<WeaponType>>,  // indexed by mount slot
}

CancelSubmission  // empty payload
```

### Server → Client

```
LobbyStatus {
    state: LobbyState,
}

enum LobbyState {
    Composing,           // connected, build your fleet
    WaitingForOpponent,  // submitted, opponent not connected yet
    OpponentComposing,   // submitted, opponent still composing
    Countdown(f32),      // both submitted, seconds remaining
    Rejected(String),    // submission invalid, reason provided
}
```

### Server-side resource

```
LobbyTracker {
    submissions: HashMap<Entity, Vec<ShipSpec>>,  // client entity → fleet spec
    countdown: Option<f32>,                        // Some when both submitted
}
```

### Server-side validation (on FleetSubmission)

1. Total fleet cost ≤ 1000
2. Each weapon fits its slot (`weapon.mount_size() <= slot.size`)
3. Loadout length matches ship class mount count
4. Ship classes are valid enum variants

Rejection sends `LobbyStatus::Rejected(reason)`.

### Server systems (run in WaitingForPlayers)

- `handle_fleet_submission` — validate, store in `LobbyTracker`, send status
- `handle_cancel_submission` — remove from `LobbyTracker`, reset countdown,
  send status
- `tick_lobby_countdown` — decrement timer, broadcast `Countdown(remaining)`,
  on zero: spawn fleets + transition to `Playing`

---

## Client UI

Bevy UI overlay rendered during client `FleetComposition` state. Everything is
mouse-clickable. No 3D scene — dark background or simple starfield.

### Layout

```
┌─────────────────────────────────────────────────┐
│  FLEET COMPOSITION          Budget: 720 / 1000  │
├────────────────────┬────────────────────────────┤
│                    │                            │
│  YOUR FLEET        │  SHIP DETAIL               │
│                    │                            │
│  [+ Add Ship]      │  Destroyer (150 pts)       │
│                    │                            │
│  ▸ Battleship  530 │  Slot 1 [Large]  Railgun   │
│    Destroyer   240 │    [Change] [Remove]       │
│    Scout        70 │  Slot 2 [Medium] Cannon    │
│                    │    [Change] [Remove]       │
│                    │  Slot 3 [Medium] LaserPD   │
│                    │    [Change] [Remove]       │
│                    │  Slot 4 [Small]  CWIS      │
│                    │    [Change] [Remove]       │
│                    │                            │
│                    │  [Remove Ship]             │
│                    │                            │
├────────────────────┴────────────────────────────┤
│  [Submit Fleet]                Status: Composing │
└─────────────────────────────────────────────────┘
```

### Left panel — Fleet list

- "Add Ship" button opens a picker: Battleship (375), Destroyer (150),
  Scout (45).
- Each ship listed with class name and total cost (hull + weapons).
- Click a ship to select → detail shown in right panel.
- Ships can be added and removed freely.

### Right panel — Ship detail

- Shows selected ship's class, hull cost, and all mount slots.
- Each slot: mount size tag, current weapon (or "Empty"), [Change] and
  [Remove] buttons.
- [Change] opens a weapon picker showing compatible weapons for that slot
  size with costs and key stats.
- [Remove] clears the slot to empty.
- [Remove Ship] deletes the ship from the fleet.

### Bottom bar

- Budget display: `{spent} / 1000` — turns red when over budget.
- Submit button — grayed out / disabled when over budget.
- Status text — reflects latest `LobbyStatus`.

### Weapon picker popup

- Appears on [Change] click.
- Lists all weapons that fit the slot size.
- Shows: weapon name, cost, key stats (damage, range, fire rate).
- Click to assign weapon and close picker.

---

## Fleet Spawning

### Changes to spawn_server_ship

Takes a `&ShipSpec` instead of just `ShipClass`. Builds `Mounts` from the
spec's loadout instead of calling `default_mounts()`.

```
fn spawn_server_ship(
    commands: &mut Commands,
    position: Vec2,
    team: Team,
    spec: &ShipSpec,
) -> Entity
```

`default_mounts()` and `default_loadout()` are preserved for tests and
potential AI/single-player use. They stop being used by the multiplayer spawn
path.

### Spawn positions

Each team gets a spawn zone at existing corners (Team 0 near (-300, -300),
Team 1 near (300, 300)). Ships placed in a line formation within the zone,
spaced by collision radius + padding. No player control over initial positions.

```
fn spawn_position(team: Team, ship_index: usize, ship_count: usize) -> Vec2 {
    let base = match team.0 {
        0 => Vec2::new(-300.0, -300.0),
        _ => Vec2::new(300.0, 300.0),
    };
    let spacing = 30.0;
    let offset = (ship_index as f32 - (ship_count - 1) as f32 / 2.0) * spacing;
    base + Vec2::new(-offset, offset) * 0.707
}
```

### Asteroid exclusion zones

Asteroids cannot spawn within 100m of team spawn corners. This prevents
asteroids from blocking fleet spawn positions.

```
let spawn_zones = [
    Vec2::new(-300.0, -300.0),  // Team 0
    Vec2::new(300.0, 300.0),    // Team 1
];
let spawn_exclusion = 100.0;

// Reject asteroid candidates too close to spawn zones
let too_close = spawn_zones.iter()
    .any(|zone| (candidate - *zone).length() < spawn_exclusion);
```

---

## Testing Strategy

All tests remain pure-function or World-level. No full App, no render context.

### New test areas

| Area | Tests |
|------|-------|
| Point costs | Hull costs match table, weapon costs match table |
| Mount compatibility | Downsizing allowed, exact match works, oversize rejected |
| Fleet validation | Under budget accepted, over budget rejected, bad loadout rejected, empty slots accepted |
| Spawn positions | Ships don't overlap, positions within bounds |
| Asteroid exclusion | Candidates near spawn zones rejected |
| Lobby state machine | Submit/cancel transitions, countdown tick, reset on cancel |
| ShipSpec → Mounts | Spec with weapons produces correct Mount list, empty slots produce empty mounts |
