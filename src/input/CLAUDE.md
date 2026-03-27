# input/

Player input handling — ship selection, movement, targeting, and mode management.

## Files

- `mod.rs` — All input systems

## Input modes

InputMode enum (Normal/Move/Lock/Target/Missile/Join) — all mutually exclusive. Mode indicator text in bottom-left.

| Key | Action |
|---|---|
| Left-click | Select ship (no drag); click ground = deselect; Shift+click = multi-select |
| Space | Toggle move mode |
| Right-click (move mode) | Quick = move; hold+drag = move + face direction |
| Shift+right-click | Append waypoint |
| Alt+right-click | Facing lock (works on any surface incl. asteroids) |
| L | Facing lock mode |
| K | Target mode (number keys target enemies, incl. radar tracks) |
| M | Missile mode (number keys fire at enemies) |
| J | Join mode (click friendly or press number to assign squad) |
| R | Toggle radar on/off for selected ships |
| S | Full stop (propagates to squad) |
| 1-9 | Select ship by number |
| ] | Toggle debug visuals |

## Key patterns

- **2x collision radius** invisible picking sphere for easier selection
- **Enemy numbering**: K/M mode dynamically assigns 1-9 to visible enemies + radar tracks. Stable via `source_numbers` map. Friendly numbers hidden in K/M mode.
- **MoveGestureState**: tracks right-click drag for facing commands with preview gizmos
- **Systems chained**: handle_keyboard → update_enemy_numbers → handle_number_keys
- All commands emit network triggers (client→server)
