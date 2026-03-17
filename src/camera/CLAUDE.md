# camera/

Free camera controller for top-down tactical view.

## Files

- `mod.rs` — GameCamera marker, CameraSettings resource (pan_speed, zoom_speed, rotate_speed, min/max_zoom), camera spawn with Camera3d + DirectionalLight + GlobalAmbientLight resource, camera_pan (WASD/arrows, relative to camera facing), camera_zoom (scroll wheel along forward vector, clamped), camera_rotate (middle-mouse orbit around ground intersection point)
