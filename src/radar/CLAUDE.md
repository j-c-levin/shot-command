# radar/

SNR-based radar detection, contact tracking, and RWR.

## Files

- `mod.rs` — Constants (SIGNATURE_THRESHOLD 0.1, TRACK_THRESHOLD 0.4, SIGNATURE_FUZZ_RADIUS 75m), compute_aspect_factor/compute_snr pure functions, RadarActive marker (server-only), RadarActiveSecret (on ShipSecrets), ContactLevel (Signature/Track), RadarContact components, ContactTracker resource
- `contacts.rs` — update_radar_contacts (SNR-based for ships+missiles+projectiles), cleanup_stale_contacts, best_radar_range
- `rwr.rs` — RwrBearings component (on ShipSecrets), is_in_rwr_range, update_rwr_bearings (with asteroid LOS blocking)
- `visuals.rs` — Client gizmos: radar range circle, signature pulse (orange), track diamond (red), tracked missiles (orange X), RWR bearing lines (yellow)

## Detection model

- SNR formula: `(BaseRange²/Distance²) × RCS × AspectFactor`
- Three awareness layers: (1) Signature (low SNR, fuzzed position), (2) Track (high SNR, precise position + fire control), (3) Visual LOS (400m, ship model)
- Radar starts OFF, R key toggles. SearchRadar 800m (medium mount, 35pts), NavRadar 500m (small mount, 20pts).
- Aspect factor: broadside highest, nose-on/tail-on lowest
- Asteroids block radar LOS
- Team-shared: any teammate's track is everyone's track
- RWR: free with radar hardware, gives bearing lines toward enemy radar sources

## RadarContact entities

- Standalone (like ShipSecrets), replicated to detecting team only via RadarBit
- Missiles/projectiles always instantly tracked if inside radar range
- PD integration: CWIS range doubled (200m/300m) for radar-tracked missiles
