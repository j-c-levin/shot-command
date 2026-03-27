# weapon/

Weapons, projectiles, missiles, point defense, and damage/repair systems.

## Files

- `mod.rs` — MountSize (hp: Large=150, Medium=100, Small=75), WeaponType (HeavyCannon/Cannon/Railgun/HeavyVLS/LightVLS/LaserPD/CWIS/SearchRadar/NavRadar), WeaponCategory, FiringArc, WeaponProfile, WeaponState (incl. tubes_loaded, tube_reload_timer for VLS), Mount (with hp/max_hp/offline_timer), Mounts component, MissileQueue
- `projectile.rs` — Projectile/ProjectileVelocity/ProjectileDamage/ProjectileOwner, RailgunRound/CwisRound markers, spawn_projectile, ProjectilePlugin
- `firing.rs` — compute_lead_position, is_in_firing_arc, tick_weapon_cooldowns, auto_fire system. Cannon stagger: 0.5s between each cannon on a ship.
- `missile.rs` — Missile components, compute_intercept_point, is_in_seeker_cone, spawn_missile, MissilePlugin (flat flight, seeker cone acquisition, asteroid collision)
- `pd.rs` — LaserPD (300m, visible beam tracking, 0.15s delayed kill) + CWIS (100m kill / 150m visual, doubled for radar-tracked). Probability-based kills. 0.2s retarget delay.
- `damage.rs` — Directional damage system: HitZone/DamageTarget, classify_hit_zone, route_damage (70/30 split), apply_damage_to_ship, tick_repair, DamagePlugin chain

## Damage model

Three HP pools: Hull (permanent, no repair), Engines (EngineHealth), Components (per-mount HP by MountSize).

**Hit zones** (angle from ship facing):
- Front (±45°): 70% hull / 30% component
- Rear (±45° from tail): 70% engines / 30% component
- Broadside (45-135°): 70% component / 30% hull-or-engines
- Railgun override: 90% component / 10% hull

**Repair**: 5s after last hit, auto-repair toward 10% floor at 20hp/s. Offline timer by mount size: Small=10s, Medium=15s, Large=20s, engines=15s. Hull never repairs.

## Weapon stats

| Weapon | Damage | Notes |
|---|---|---|
| HeavyCannon | 25×3=75/burst | Large mount |
| Cannon | 20/shot | Medium mount, 0.5s stagger |
| Railgun | 50 | Forward-facing ±10°, precision component targeting |
| HeavyVLS | 80 (missile) | Large mount, 3s per-tube reload |
| LightVLS | 80 (missile) | Medium mount, 3s per-tube reload |
| LaserPD | — | Medium mount, 300m, beam tracking |
| CWIS | — | Small mount, 100m (200m radar-tracked) |
