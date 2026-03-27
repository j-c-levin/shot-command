# camera/

Free camera controller for top-down tactical view.

## Files

- `mod.rs` — CameraLookAt resource, LeftDragState resource (drag vs click discrimination), strategic zoom (cursor zoom-in, center zoom-out), WASD pan (S is stop-only, not camera pan), left-click drag pan, right-click drag orbit (yaw+pitch, Normal mode only)

## Key behavior

- Camera zoom and left-drag pan are gated out of Editor state (editor provides its own scroll handler)
- Orbit only active in InputMode::Normal
