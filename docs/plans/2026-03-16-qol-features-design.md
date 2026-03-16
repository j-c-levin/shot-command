# QoL Features Design — Clone, Squads, Stagger, Number Keys

Four quality-of-life features spanning fleet builder UI, input, networking, and
weapon firing.

---

## 1. Clone Ship (Fleet Builder)

A "Clone" button in the fleet list panel next to each ship entry. Clicking it
pushes a copy of that ship's `ShipSpec` (same class, same loadout) to the end
of the fleet list and selects the new copy.

No special logic — just `state.ships.push(spec.clone())`. If over budget after
cloning, submit button stays grayed out as normal.

---

## 2. Squad Formation System

### Data model

New component on Ship entities (server-side):

```
SquadMember {
    leader: Entity,
    offset: Vec2,    // position offset from leader at time of joining
}
```

No squad registry. Leadership is implicit — a ship is a "leader" simply because
other ships have `SquadMember` referencing it.

### Joining (J key)

1. Select ship A, press J → enters "join mode" (like existing L/K/M modes)
2. Click a friendly ship B (or press B's number key) → A gets
   `SquadMember { leader: B, offset: A.pos - B.pos }`
3. Join mode resets after assignment

### Move order propagation

When a move order is issued to a leader, server finds all ships with
`SquadMember { leader: that_entity }` and issues them move orders at
`destination + their_offset`.

- Waypoint queue and shift-click work the same — followers get matching queued
  waypoints with offsets applied
- Facing lock does NOT propagate — each ship manages its own facing

### Breaking formation

- Direct move order to a follower → remove `SquadMember` component
- Direct facing lock does NOT break formation (facing is independent)
- Leader destroyed → followers lose `SquadMember` (orphan cleanup)
- Follower destroyed → nothing special, it just despawns

### Selection behavior

- Selecting a leader → also highlights followers (dimmed/gray selection indicator)
- Selecting a follower → normal green selection ring + a cyan connection
  line/arrow pointing toward the leader. Leader gets a small marker too.
- Number keys → pressing leader's number selects leader + highlights followers

### Network

New `JoinSquadCommand { ship: Entity, leader: Entity }` client→server trigger.
Server validates both are on the same team. Server adds `SquadMember` with
offset computed from current positions.

New `LeaveSquadCommand { ship: Entity }` — implicit when a follower receives a
direct move order. Server removes `SquadMember`.

SquadMember syncs via ShipSecrets (team-private, like waypoints).

---

## 3. Staggered Cannon Firing

New `fire_delay: f32` field on `WeaponState`. Ticks down each frame alongside
cooldown. A cannon can only fire when both `cooldown <= 0` AND `fire_delay <= 0`.

When a ship's first cannon fires at a target, all other ready cannons on that
ship get `fire_delay` set to `0.5 * stagger_index` (0.5s between each). So
with 3 ready cannons: first fires immediately, second 0.5s later, third 1.0s
later.

Only applies to `WeaponCategory::Cannon` — missiles and PD unaffected.

The stagger resets naturally each volley from cooldown differences.

### Constant

```
const CANNON_STAGGER_DELAY: f32 = 0.5;
```

---

## 4. Number-Key Ship Selection

### Fleet builder

Ships numbered 1-N based on position in fleet list. Number displayed next to
each entry (e.g., "1. Battleship 670pts"). Numbers update dynamically.

### In-game

Server assigns `ShipNumber(u8)` component when spawning from ShipSpec — ship 0
in spec list gets ShipNumber(1), ship 1 gets ShipNumber(2), etc. Replicated
via ShipSecrets (team-private).

### Input

- Press 1-9 → select ship with that number on your team
- If that ship is a squad leader → also highlight followers
- Works with mode keys: press 1, press J, press 2 = ship 1 joins ship 2's
  squad (keyboard-only flow, no mouse needed)
- Number keys in join mode target the numbered ship for the join command

### Visual

Small number label floating above each friendly ship (always visible for own
team, not visible to enemy).

---

## Testing Strategy

All tests remain pure-function or World-level. No full App.

| Feature | Tests |
|---------|-------|
| Clone ship | FleetBuilderState manipulation (unit test) |
| Squad offset | Pure function: compute offset from positions |
| Squad move propagation | offset applied correctly to destination |
| Squad orphan cleanup | leader entity gone → remove SquadMember |
| Stagger delay | fire_delay ticking, stagger index assignment |
| Ship number assignment | spec index → ShipNumber mapping |
