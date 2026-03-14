# input/

Player input handling via Bevy picking observers.

## Files

- `mod.rs` — on_ship_clicked (left-click observer, selects player ships only), on_ground_clicked (right-click observer, sets MovementTarget from hit position), selection indicator torus (Pickable::IGNORE, follows selected ship, hidden when nothing selected), Escape to deselect

## Observer attachment

Observers are attached per-entity in `main.rs::setup_game`:
- `on_ship_clicked` → each ship entity
- `on_ground_clicked` → ground plane entity
