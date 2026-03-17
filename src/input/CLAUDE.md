# input/

Player input handling via Bevy picking observers.

## Files

- `mod.rs` — LockMode resource (bool), on_ship_clicked (left-click: select player ship; alt+right-click on own ship: unlock facing), on_ground_clicked (right-click: set waypoint; shift+right-click: append waypoint; alt+right-click: set facing + lock; lock mode: next right-click sets facing), handle_keyboard (Escape: deselect + exit lock mode; L: unlock locked ships or toggle lock mode), selection indicator torus (Pickable::IGNORE, follows selected ship)

## Observer attachment

Observers are attached per-entity in `main.rs::setup_game`:
- `on_ship_clicked` → each ship entity
- `on_ground_clicked` → global observer (ground plane)
