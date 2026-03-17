# Fleet Status Sidebar

Left-edge panel showing real-time status of all friendly ships during
gameplay. Inspired by Nebulous: Fleet Command's compact fleet overview.

---

## Layout

Narrow panel (~200px wide) on the left edge. One card per ship, stacked
vertically. Semi-transparent dark background. Spawned on
`OnEnter(GameState::Playing)`, despawned on exit.

---

## Ship Card (~70px tall)

```
┌──────────────────────┐
│ 1  Destroyer         │  ship number + class name
│ ██████████░░  Hull   │  green bar (red <25%), permanent damage
│ ████████░░░░  Eng    │  blue bar, "OFFLINE" overlay at 0 HP
│ ┌────┬────┬────┬────┐│
│ │HC12│PD  │VLS3│SR  ││  weapon abbrev + ammo count
│ │ ●  │ ●  │ ●  │ ●  ││  green=online, red=offline, gray=empty
│ └────┴────┴────┴────┘│
└──────────────────────┘
```

### Health bars

- Hull: green bar, percentage of max. Turns red below 25%. Never
  repairs — only goes down.
- Engines: blue bar, current/max. Shows red "OFFLINE" text overlay when
  hp == 0.

### Weapon slots

One cell per mount. Each cell shows:

- 2-3 letter abbreviation: HC, CN, RG, HV, LV, LP, CW, SR, NR
- Ammo count for all weapons except LaserPD (energy-based, no ammo).
  For VLS: tubes loaded (e.g., "3/4"). For cannons/railgun/CWIS:
  remaining rounds.
- Status dot: green = online, red = offline (0 HP), gray = empty slot.

### Weapon abbreviations

| WeaponType   | Abbrev |
|-------------|--------|
| HeavyCannon  | HC     |
| Cannon       | CN     |
| Railgun      | RG     |
| HeavyVLS     | HV     |
| LightVLS     | LV     |
| LaserPD      | LP     |
| CWIS         | CW     |
| SearchRadar  | SR     |
| NavRadar     | NR     |

---

## Destroyed Ships

Entire card grayed out. Red "DESTROYED" text overlay on the card.
Card stays in the sidebar (not removed) so fleet attrition is visible.

---

## Selection

Clicking a ship card selects that ship in-game (same as pressing its
number key). Selected ship's card gets a green left border matching the
in-game selection circle color.

---

## Data Sources

All data reads from existing replicated components — no new network
data needed:

- Health (hull HP) — replicated
- EngineHealth (engine HP, offline state) — replicated
- Mounts (per-mount HP, weapon type, ammo, tubes_loaded) — replicated
- ShipNumber, ShipClass — replicated
- Destroyed marker — replicated
- Selected marker — client-only, already exists

---

## Stretch Goal: Cooldown Bars

Thin horizontal fill bar behind each weapon cell showing reload progress:

- Cannons/Railgun: fills during cooldown timer (fire_rate_secs)
- VLS: fills during tube_reload_timer (per-tube reload)
- LaserPD: fills during cooldown timer
- CWIS: no bar (fires too fast, would just flicker)

Empty = cooling down, full = ready to fire.

---

## What This Does Not Include

- Enemy ship status (own fleet only)
- Detailed damage log or repair progress
- Per-component health bars (just online/offline dots)
- Draggable/resizable panel
