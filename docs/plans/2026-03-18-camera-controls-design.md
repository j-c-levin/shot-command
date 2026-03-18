# Camera Controls Overhaul

## Goal

Replace keyboard-only pan and middle-mouse orbit with mouse-drag controls.
Left-click drag pans, right-click drag orbits. Double ship selection hit radius.

## Left-click drag → pan

- Track mouse-down position on left-click press.
- If mouse moves >5px before release, enter pan mode.
- Pan moves camera + CameraLookAt by the world-space delta of cursor movement
  (project screen delta to ground plane).
- On mouse-up without sufficient drag: fire existing ship selection logic.
- Lives in input module since it gates selection.

## Right-click drag → orbit (Normal mode only)

- Only activates when InputMode::Normal.
- Horizontal mouse delta → yaw (rotate camera around CameraLookAt on Y axis).
- Vertical mouse delta → pitch (tilt camera around CameraLookAt). No clamping.
- Replaces middle-mouse camera_rotate (removed).
- Right-click commands only fire in Move mode, so no conflict.

## Ship selection radius

- Double the distance threshold used for left-click ship selection.

## Files changed

- `src/camera/mod.rs` — Remove `camera_rotate`. Add `camera_orbit` (right-click
  yaw+pitch around look-at). Add `camera_drag_pan` (left-click drag).
- `src/input/mod.rs` — Add drag threshold to left-click. Only fire selection if
  no drag occurred. Double selection hit radius.

## Tests

- Pure function tests for orbit math (yaw, pitch around a point).
