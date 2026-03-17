# Phase 5: Directional Damage & Repair

Beam weapons deferred. This phase focuses on making damage tactically
meaningful through directional routing and a passive repair model that
creates permanent degradation without player micro.

---

## HP Pools

Each ship has three distinct HP pools:

| Pool | Repairable | Notes |
|------|-----------|-------|
| Hull | No | Permanent damage; highest HP pool; ship destroyed at 0 |
| Engines | Yes (to 10% floor) | At 0: fully adrift, coasts to stop via space drag |
| Components | Yes (to 10% floor) | Per-mount; each weapon, PD, sensor has its own HP |

Components are individual mounts — cannons, railguns, VLS, PD, radar — each
tracking HP independently. "Weapons" in code = all mounted equipment.

---

## Damage Routing

Hit angle is calculated relative to the target ship's facing direction.
Incoming projectile/missile direction vs ship forward vector.

| Zone | Angle from nose | Primary (70%) | Secondary (30%) |
|------|----------------|--------------|-----------------|
| Front | ±45° | Hull | Random component |
| Rear | ±135–180° | Engines | Random component |
| Broadside | ±45–135° | Random component | Hull or Engines (random) |

"Random component" = randomly pick one mounted weapon/sensor to receive
the damage. If a ship has no components, that portion goes to hull.

---

## Repair Mechanics

- A few seconds after the last damage hit, passive auto-repair kicks in
- Repairs the pool back up to 10% of its max HP — never above that
- Once a pool drops below 90% max HP, it never fully recovers (permanent degradation)
- Components at 0 HP go offline for a cooldown, then restore to exactly 10% HP
- Hull never repairs under any circumstances

Suggested tuning values (adjust in testing):
- Repair delay: 5 seconds after last hit
- Repair rate: fast enough to feel responsive (~2s to reach 10% floor from 0)
- Offline cooldown: 10 seconds before a 0 HP component restores to 10%

---

## Binary Performance

No degradation curves. Systems are either fully operational or offline (at 0 HP).
Partial damage accumulates silently until a system crosses zero.

---

## Engine Offline Behavior

Engines at 0 HP: no thrust whatsoever. Ship drifts on current velocity.
The existing ~26%/s space drag bleeds velocity to zero over ~15 seconds.
A drifting ship is essentially dead in the water — easy to finish or ignore.
Engines restore to 10% HP after the offline cooldown (same as components).

---

## Suggested HP Values

Hull is the tank — hard to kill through the front but permanent damage.
Components are fragile — a few hits from the right angle knocks them out.

| Ship Class | Hull HP | Engine HP | Per-Component HP |
|------------|---------|-----------|-----------------|
| Battleship | 1200 | 400 | 150 |
| Destroyer | 600 | 300 | 100 |
| Scout | 300 | 200 | 75 |

Note: existing `ShipProfile.hp` maps to Hull HP. Engine and component HP
are new pools to add to ShipProfile.

---

## Tactical Implications

- Target rear to strand a ship (engines → adrift, space drag bleeds velocity)
- Target broadside to grind down weapons/sensors over time
- Protect your nose — front hits go to hull (permanent) but hull is large
- A ship at 10% engine HP is one hit from going adrift

---

## What This Does Not Include

- Beam weapons (deferred)
- Repair system (passive auto-repair only, no player-triggered repair)
- Reactor as a system (cut for simplicity)
- Sensors as a separate pool (sensors are components like any other mount)
- Visual damage indicators (out of scope for this phase)
