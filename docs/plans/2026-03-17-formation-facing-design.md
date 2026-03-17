# Formation Facing & Waypoint Visuals — Design

Right-click drag to set facing at destination. Formation offsets rotate to
match. Waypoint markers replaced with gizmo lines.

---

## 1. Waypoint Line Gizmos

Replace the blue sphere waypoint markers with gizmo lines.

**Visual:** For each selected friendly ship that has waypoints (via ShipSecrets):
- Draw a line from ship position to first waypoint
- Draw lines between consecutive waypoints
- All lines at Y=1.0 (ground level), blue color

Remove the existing `WaypointMarker` entity spawning in `src/ship/mod.rs`
(`update_waypoint_markers`). Replace with a gizmo system.

---

## 2. Right-Click Hold+Drag for Facing

Currently right-click in move mode instantly sends a MoveCommand. Change to
detect hold+drag for facing control.

### Input flow

- Right-click press in move mode → record click position on ground plane,
  enter "drag pending" state
- Mouse moves while held → show a short direction line gizmo from the
  destination point in the drag direction
- Right-click release:
  - If drag distance < threshold (5px screen space) → normal move (no facing)
  - If drag distance >= threshold → move + face lock in drag direction

### Data model

New client-only resource:

```
MoveGestureState {
    active: bool,
    destination: Vec2,       // ground XZ position where right-click landed
    screen_start: Vec2,      // screen position of initial click
}
```

Set on right-click press. Cleared on release.

### Visual during drag

While dragging, draw:
- Blue circle at destination (small, ~3m radius)
- Short cyan line from destination in the drag direction (~20m long)
- This previews where the ship will go and which way it will face

### Network commands

On release with drag:
- Send `MoveCommand { ship, destination, append }` as normal
- Send `FacingLockCommand { ship, direction }` where direction is the
  drag vector normalized

On release without drag (quick click):
- Send `MoveCommand` only (no facing lock — ship auto-faces waypoint)

### Shift+right-click

Shift held → append waypoint. Same drag logic applies — the facing lock
is set for the final waypoint direction. Only the last waypoint in a
shift-click sequence sets facing.

---

## 3. Formation Rotation

When a leader receives a move+facing command, followers' offsets rotate
to match the leader's new facing direction.

### How it works

Current system: follower destination = leader destination + static offset.
The offset was captured as `follower_pos - leader_pos` at join time.

New system:
1. Leader gets move + facing lock (from drag)
2. Compute angle delta: `new_facing_angle - leader_current_heading`
3. Rotate each follower's offset by that delta
4. Follower destination = leader destination + rotated offset
5. Followers also get the same facing lock direction

### Rotation math

```
rotated_offset = Vec2::new(
    offset.x * cos(delta) - offset.y * sin(delta),
    offset.x * sin(delta) + offset.y * cos(delta),
)
```

This is a standard 2D rotation matrix applied to the offset vector.

### Move without facing (quick click)

If the leader gets a move without facing (quick right-click), offsets are
NOT rotated — followers get straight translation as before. Rotation only
happens when an explicit facing direction is set.

### Server-side implementation

In `handle_move_command`:
- Currently propagates moves with static offsets
- Add: if the leader also has a `FacingLockCommand` in the same frame
  (or a `FacingTarget` was just set), compute the rotation delta and
  apply it to follower offsets

Actually, simpler: add an optional `facing_direction: Option<Vec2>` field
to `MoveCommand`. When present, the server:
1. Sets the facing lock on the leader
2. Computes rotation delta from current heading to new facing
3. Rotates follower offsets
4. Sets facing lock on all followers

This bundles move+facing into a single command, avoiding frame-timing issues
with separate commands.

### Updated MoveCommand

```
MoveCommand {
    ship: Entity,
    destination: Vec2,
    append: bool,
    facing: Option<Vec2>,  // NEW: if Some, lock facing to this direction
}
```

`facing: None` = move only (current behavior, no rotation).
`facing: Some(dir)` = move + face + rotate formation.

---

## 4. S Key (Stop) with Squads

S key on a squad leader should stop all members. Currently it sends a
MoveCommand to the leader's current position, which propagates. This works
but doesn't clear facing locks on followers.

Change: S key sends `MoveCommand { destination: current_pos, facing: None }`
which propagates stop to followers. Also send `FacingUnlockCommand` for all
followers (via propagation in the server handler).

---

## Testing

| Area | Tests |
|------|-------|
| Offset rotation | Pure function: rotate_offset(offset, delta) for 0/90/180/270 degrees |
| Move+facing propagation | Rotated offset produces correct follower destinations |
| Quick click vs drag | facing: None vs facing: Some distinction |
