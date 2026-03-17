# Phase 4a: Radar & Detection — Design

## Overview

Replace the uniform 400m vision system with a layered detection model. Radar is
mountable equipment that produces two tiers of awareness (signature and track)
on top of the existing visual LOS. Radar Warning Receivers come free with radar
hardware. Radar starts off — activating it is a deliberate decision that exposes
you to enemy RWR.

## Sensor Equipment

Two radar types, mountable in weapon slots:

| Radar        | Mount  | Base Range | Cost  |
|--------------|--------|-----------|-------|
| Search Radar | Medium | 800m      | 35pts |
| Nav Radar    | Small  | 500m      | 20pts |

- Radar starts **off** by default. Player toggles per-ship with R key.
- RWR is free on any ship with a radar equipped. Ships without radar have
  no sensor capability.
- Team-side gizmo icon next to ship: grey = radar off, blue = radar on.

## Signal-to-Noise Model

Radar detection uses an SNR calculation with three inputs:

**Signal = (BaseRange² / Distance²) × RCS × AspectFactor**

Inputs:
- **Distance** — inverse square falloff.
- **RCS (Radar Cross-Section)** — intrinsic to ship class. Battleship ~1.0,
  Destroyer ~0.5, Scout ~0.25.
- **Aspect Factor** — sine curve based on angle between radar bearing and
  target's facing. Broadside = 1.0, nose-on = 0.25.
  `aspect = 0.25 + 0.75 * sin(angle_between)`

Two thresholds:
- **Signature** (low, ~0.1) — fuzzy pulsing circle gizmo at approximate
  position (actual + random offset ~50-100m). No entity, no class, no targeting.
- **Track** (high, ~0.4) — precise diamond marker. Numbered in K/M mode.
  Full fire control. Team-shared.

Radar also tracks projectiles and missiles (with their own small RCS values).
Tracked missiles get a distinctive gizmo. PD can engage radar-tracked missiles
even outside visual range.

## RWR (Radar Warning Receiver)

When any enemy radar's signal hits your ship (your ship is within their radar's
base range, regardless of whether they achieve signature or track):

- Bearing line gizmo from your ship toward the radar source.
- No range, no identity — just direction.
- One line per distinct radar source.
- Only visible to owning team.
- Only works if your ship has radar equipped.

## Three Awareness Layers

### 1. Radar Signature (lowest fidelity)

Server creates a lightweight `RadarContact` entity replicated to the detecting
team. Contains approximate position (fuzzed) and contact type. Rendered as
pulsing circle gizmo. No ship entity on client.

### 2. Radar Track (fire-control fidelity)

Same `RadarContact` entity promoted to track status. Contains precise position
and velocity for fire control lead calculation. Rendered as diamond marker,
numbered in K/M mode. Full targeting and fire control. Team-shared — if any
teammate tracks a target, everyone gets the track.

### 3. Visual LOS (full fidelity)

Ship entity replicates as today (400m + asteroid raycasting). Ship model renders.
Track diamond still overlaid. Ghost fade-out still fires on visual loss.

Key points:
- A ship can have a track without visual LOS (shooting at a marker).
- A ship can have visual LOS without a track (radar off, close enough to see).
- `RadarContact` entities are standalone (not children), similar to ShipSecrets
  pattern. Server creates/updates/despawns each frame based on SNR across all
  team radars.
- Stable contact IDs so contacts don't flicker or renumber.

## Integration with Existing Systems

### Weapons & Fire Control

- Cannons and missiles can fire at radar tracks (target is RadarContact
  position/velocity).
- K mode numbering applies to tracks.
- M mode missile firing works against tracks.
- Losing a track mid-flight: missiles continue on last-known heading,
  cannons stop firing.

### Point Defense

- LaserPD and CWIS can engage radar-tracked missiles even outside visual range.
- Ships with no radar: PD only works within 400m visual range.

### Fog System

- Current LOS check stays for visual range (400m + asteroid raycasting).
- Radar runs alongside as a separate detection pass.
- Ghost fade-out fires on visual LOS loss only (not track loss).

### Fleet Builder

- Two new entries in weapon picker: Search Radar (medium, 35pts),
  Nav Radar (small, 20pts).
- Ships with no radar have no sensor capability — valid but risky loadout.

### Radar Toggle

- R key toggles radar on/off for selected ship(s).
- New `RadarToggleCommand` through command channel.
- Server-authoritative with team validation.

## NOT in Phase 4a

- Passive sensors — may come later.
- Fire control accuracy degradation (lock quality affecting aim) — Phase 4b.
  For now tracks give the same accuracy as the current system.
- Control points — Phase 4c.
- Thermal signatures / engine-cut stealth — deferred.
- Radar cone visualization — possible F3 debug visual, not core.
