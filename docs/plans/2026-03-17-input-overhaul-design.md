# Input Overhaul & Squad Improvements — Design

Explicit move mode, enemy numbering, keyboard targeting flow, squad speed
limiting, and visual polish.

---

## 1. Move Mode (Spacebar Toggle)

Right-click no longer moves by default. Player must enter move mode first.

### Input flow

- Press Space → enter move mode (like K/M/J modes)
- Press Space again → exit move mode
- Right-click ground in move mode → issue move command
- Shift+right-click in move mode → append waypoint
- Right-click outside move mode → does nothing
- Entering any other mode (K/M/J) exits move mode
- Entering move mode exits other modes

### LOS circle

When in move mode with a ship selected, draw a circle on the ground plane
showing the ship's vision range (400m). Use a thin ring mesh at ground level,
centered on the selected ship, updating position each frame. Color: semi-
transparent green.

### Mode indicator

Show current mode in the UI somewhere (bottom-left or near cursor):
"MOVE", "TARGET", "MISSILE", "JOIN", or nothing when no mode active.

---

## 2. Number Labels — Below Models, Smaller

Move ship number labels from above the ship model to below it. Reduce font
size. The label should float at ground level (Y=0) offset slightly below the
ship's collision radius.

Use world-to-viewport projection as before, but with a lower Y offset.

---

## 3. Enemy Numbering in Offensive Modes

When entering K (target) or M (missile) mode, assign consistent numbers to
all currently visible enemy ships. Numbers persist as long as the mode is
active and the enemy is visible.

### Data model

`EnemyNumbers` resource (client-side):

```
EnemyNumbers {
    assignments: HashMap<Entity, u8>,  // enemy entity → number (1-9)
    active: bool,                       // whether enemy numbers are shown
}
```

Populated when entering K or M mode by sorting visible enemies by some stable
criterion (e.g., by entity index or by distance from map center) and assigning
1-9. Cleared when exiting offensive modes.

### Visual

Show enemy numbers as red-tinted labels below enemy ship models (same
positioning as friendly numbers, but red color). Only visible when
`EnemyNumbers.active`.

---

## 4. Keyboard Targeting Flow

In K mode, pressing a number key (1-9) targets the enemy with that number
(from EnemyNumbers) instead of selecting a friendly ship.

In M mode, pressing a number key fires a missile at the enemy with that number.

This enables the flow: `1k1m1122` meaning:
- 1 → select my ship 1
- k → enter target mode
- 1 → target enemy 1 (assigns cannon target)
- m → enter missile mode
- 1 → fire missile at enemy 1
- 1 → fire missile at enemy 1 again
- 2 → fire missile at enemy 2
- 2 → fire missile at enemy 2 again

Number keys have dual meaning based on mode:
- No mode / Move mode / Join mode → select friendly ship by number
- K mode → target enemy by number (then exit K mode)
- M mode → fire missile at enemy by number (stay in M mode)

---

## 5. K Mode Empty Space

Right-click on ground in K mode does nothing (currently processes as a move).
Only clicking an enemy ship in K mode should designate a target.

Same for M mode: right-click on ground fires missile at point (this already
works correctly). No change needed for M.

---

## 6. Squad Speed Limiting

When a leader receives a move order that propagates to followers, all ships
in the squad (leader + followers) should have their effective speed limited
to the slowest member's `top_speed`.

### Implementation

Add `squad_speed_override: Option<f32>` to the Ship entity (or a separate
component `SquadSpeedLimit(f32)`). When a ship joins a squad or a squad move
is issued, compute `min(leader.top_speed, follower1.top_speed, ...)` and set
it on all members.

In `apply_thrust`, cap the target speed at `squad_speed_override` if present.

When a ship leaves the squad (direct order, leader destroyed), remove the
speed override.

---

## 7. Squad Leader Reassignment

Already covered in code review Fix 2: when a leader joins another squad, all
its current followers become followers of the new leader with recomputed
offsets.
