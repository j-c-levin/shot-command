pub mod damage;
pub mod firing;
pub mod missile;
pub mod pd;
pub mod projectile;

use bevy::ecs::entity::{EntityMapper, MapEntities};
use bevy::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum MountSize {
    Large,
    Medium,
    Small,
}

impl MountSize {
    /// Numeric rank for size comparison: Large=2, Medium=1, Small=0.
    pub fn rank(self) -> u8 {
        match self {
            MountSize::Large => 2,
            MountSize::Medium => 1,
            MountSize::Small => 0,
        }
    }

    /// Returns true if this mount slot can hold a weapon of the given size.
    /// A slot fits weapons of its own size or smaller.
    pub fn fits(self, weapon_size: MountSize) -> bool {
        self.rank() >= weapon_size.rank()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WeaponCategory {
    Cannon,
    Missile,
    PointDefense,
    Sensor,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum WeaponType {
    HeavyCannon,
    Cannon,
    Railgun,
    HeavyVLS,
    LightVLS,
    LaserPD,
    CWIS,
    SearchRadar,
    NavRadar,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum FiringArc {
    /// 360-degree turret rotation.
    Turret,
    /// Ship-facing only, +/-10 degrees.
    Forward,
}

#[derive(Clone, Debug)]
pub struct WeaponProfile {
    pub fire_rate_secs: f32,
    pub burst_count: u8,
    pub damage: u16,
    pub firing_range: f32,
    pub projectile_speed: f32,
    pub spread_degrees: f32,
    pub arc: FiringArc,
    pub tubes: u8,
    pub missile_fuel: f32,
    pub pd_cylinder_radius: f32,
}

impl WeaponType {
    pub fn category(&self) -> WeaponCategory {
        match self {
            WeaponType::HeavyCannon | WeaponType::Cannon | WeaponType::Railgun => {
                WeaponCategory::Cannon
            }
            WeaponType::HeavyVLS | WeaponType::LightVLS => WeaponCategory::Missile,
            WeaponType::LaserPD | WeaponType::CWIS => WeaponCategory::PointDefense,
            WeaponType::SearchRadar | WeaponType::NavRadar => WeaponCategory::Sensor,
        }
    }

    pub fn profile(&self) -> WeaponProfile {
        match self {
            WeaponType::HeavyCannon => WeaponProfile {
                fire_rate_secs: 3.0,
                burst_count: 3,
                damage: 15,
                firing_range: 300.0,
                projectile_speed: 150.0,
                spread_degrees: 2.0,
                arc: FiringArc::Turret,
                tubes: 0,
                missile_fuel: 0.0,
                pd_cylinder_radius: 0.0,
            },
            WeaponType::Cannon => WeaponProfile {
                fire_rate_secs: 1.0,
                burst_count: 1,
                damage: 8,
                firing_range: 200.0,
                projectile_speed: 120.0,
                spread_degrees: 2.0,
                arc: FiringArc::Turret,
                tubes: 0,
                missile_fuel: 0.0,
                pd_cylinder_radius: 0.0,
            },
            WeaponType::Railgun => WeaponProfile {
                fire_rate_secs: 7.0,
                burst_count: 1,
                damage: 50,
                firing_range: 1000.0,
                projectile_speed: 300.0,
                spread_degrees: 0.5,
                arc: FiringArc::Forward,
                tubes: 0,
                missile_fuel: 0.0,
                pd_cylinder_radius: 0.0,
            },
            WeaponType::HeavyVLS => WeaponProfile {
                fire_rate_secs: 3.0, // per-tube reload time
                burst_count: 1,
                damage: 30,
                firing_range: 500.0,
                projectile_speed: 150.0,
                spread_degrees: 0.0,
                arc: FiringArc::Turret,
                tubes: 8,
                missile_fuel: 800.0,
                pd_cylinder_radius: 0.0,
            },
            WeaponType::LightVLS => WeaponProfile {
                fire_rate_secs: 3.0, // per-tube reload time
                burst_count: 1,
                damage: 30,
                firing_range: 500.0,
                projectile_speed: 150.0,
                spread_degrees: 0.0,
                arc: FiringArc::Turret,
                tubes: 4,
                missile_fuel: 800.0,
                pd_cylinder_radius: 0.0,
            },
            WeaponType::LaserPD => WeaponProfile {
                fire_rate_secs: 1.0,
                burst_count: 1,
                damage: 10,
                firing_range: 300.0,
                projectile_speed: 0.0,
                spread_degrees: 0.0,
                arc: FiringArc::Turret,
                tubes: 0,
                missile_fuel: 0.0,
                pd_cylinder_radius: 300.0,
            },
            WeaponType::CWIS => WeaponProfile {
                fire_rate_secs: 0.1,
                burst_count: 1,
                damage: 2,
                firing_range: 100.0,
                projectile_speed: 200.0,
                spread_degrees: 2.0,
                arc: FiringArc::Turret,
                tubes: 0,
                missile_fuel: 0.0,
                pd_cylinder_radius: 100.0,
            },
            WeaponType::SearchRadar => WeaponProfile {
                fire_rate_secs: 0.0,
                burst_count: 0,
                damage: 0,
                firing_range: 800.0,
                projectile_speed: 0.0,
                spread_degrees: 0.0,
                arc: FiringArc::Turret,
                tubes: 0,
                missile_fuel: 0.0,
                pd_cylinder_radius: 0.0,
            },
            WeaponType::NavRadar => WeaponProfile {
                fire_rate_secs: 0.0,
                burst_count: 0,
                damage: 0,
                firing_range: 500.0,
                projectile_speed: 0.0,
                spread_degrees: 0.0,
                arc: FiringArc::Turret,
                tubes: 0,
                missile_fuel: 0.0,
                pd_cylinder_radius: 0.0,
            },
        }
    }

    pub fn mount_size(&self) -> MountSize {
        match self {
            WeaponType::HeavyCannon => MountSize::Large,
            WeaponType::Cannon => MountSize::Medium,
            WeaponType::Railgun => MountSize::Large,
            WeaponType::HeavyVLS => MountSize::Large,
            WeaponType::LightVLS => MountSize::Medium,
            WeaponType::LaserPD => MountSize::Medium,
            WeaponType::CWIS => MountSize::Small,
            WeaponType::SearchRadar => MountSize::Medium,
            WeaponType::NavRadar => MountSize::Small,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct WeaponState {
    pub weapon_type: WeaponType,
    pub ammo: u16,
    pub cooldown: f32,
    /// Seconds before a PD mount can engage a new target after a kill.
    /// Only meaningful for LaserPD and CWIS; always 0.0 for other weapon types.
    #[serde(default)]
    pub pd_retarget_cooldown: f32,
    /// Number of tubes currently loaded and ready to fire (VLS only).
    /// Initialized to `profile.tubes`. Decrements on fire, individual tubes
    /// reload after `fire_rate_secs` seconds.
    #[serde(default)]
    pub tubes_loaded: u8,
    /// Time until the next tube finishes reloading (VLS only).
    /// Counts down; when it reaches 0, one tube is reloaded (tubes_loaded += 1).
    #[serde(default)]
    pub tube_reload_timer: f32,
    /// Stagger delay for cannon volleys. While > 0, this cannon cannot fire.
    /// Set by a sibling cannon firing to create staggered volleys.
    #[serde(default)]
    pub fire_delay: f32,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Mount {
    pub size: MountSize,
    pub offset: Vec2,
    pub weapon: Option<WeaponState>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Component)]
pub struct Mounts(pub Vec<Mount>);

/// Queued missile launches. Lives on Ship entities, synced to ShipSecrets.
#[derive(Component, Serialize, Deserialize, Clone, Debug, Default)]
pub struct MissileQueue(pub Vec<MissileQueueEntry>);

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct MissileQueueEntry {
    pub target_point: Vec2,
    pub target_entity: Option<Entity>,
}

impl MapEntities for MissileQueue {
    fn map_entities<M: EntityMapper>(&mut self, entity_mapper: &mut M) {
        for entry in &mut self.0 {
            if let Some(entity) = &mut entry.target_entity {
                *entity = entity_mapper.get_mapped(*entity);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn heavy_cannon_profile_values() {
        let p = WeaponType::HeavyCannon.profile();
        assert_eq!(p.damage, 15);
        assert_eq!(p.burst_count, 3);
        assert_eq!(p.fire_rate_secs, 3.0);
        assert_eq!(p.firing_range, 300.0);
        assert_eq!(p.arc, FiringArc::Turret);
    }

    #[test]
    fn cannon_profile_values() {
        let p = WeaponType::Cannon.profile();
        assert_eq!(p.damage, 8);
        assert_eq!(p.burst_count, 1);
        assert_eq!(p.fire_rate_secs, 1.0);
        assert_eq!(p.firing_range, 200.0);
        assert_eq!(p.arc, FiringArc::Turret);
    }

    #[test]
    fn railgun_profile_values() {
        let p = WeaponType::Railgun.profile();
        assert_eq!(p.damage, 50);
        assert_eq!(p.burst_count, 1);
        assert_eq!(p.fire_rate_secs, 7.0);
        assert_eq!(p.firing_range, 1000.0);
        assert_eq!(p.arc, FiringArc::Forward);
    }

    #[test]
    fn weapon_type_mount_size() {
        assert_eq!(WeaponType::HeavyCannon.mount_size(), MountSize::Large);
        assert_eq!(WeaponType::Railgun.mount_size(), MountSize::Large);
        assert_eq!(WeaponType::Cannon.mount_size(), MountSize::Medium);
    }

    #[test]
    fn heavy_vls_profile_values() {
        let p = WeaponType::HeavyVLS.profile();
        assert_eq!(p.damage, 30);
        assert_eq!(p.burst_count, 1);
        assert_eq!(p.fire_rate_secs, 3.0);
        assert_eq!(p.firing_range, 500.0);
        assert_eq!(p.tubes, 8);
        assert_eq!(p.projectile_speed, 150.0);
        assert_eq!(p.missile_fuel, 800.0);
        assert_eq!(p.arc, FiringArc::Turret);
    }

    #[test]
    fn light_vls_profile_values() {
        let p = WeaponType::LightVLS.profile();
        assert_eq!(p.damage, 30);
        assert_eq!(p.burst_count, 1);
        assert_eq!(p.fire_rate_secs, 3.0);
        assert_eq!(p.firing_range, 500.0);
        assert_eq!(p.tubes, 4);
        assert_eq!(p.projectile_speed, 150.0);
        assert_eq!(p.missile_fuel, 800.0);
        assert_eq!(p.arc, FiringArc::Turret);
    }

    #[test]
    fn laser_pd_profile_values() {
        let p = WeaponType::LaserPD.profile();
        assert_eq!(p.damage, 10);
        assert_eq!(p.fire_rate_secs, 1.0);
        assert_eq!(p.firing_range, 300.0);
        assert_eq!(p.projectile_speed, 0.0);
        assert_eq!(p.pd_cylinder_radius, 300.0);
        assert_eq!(p.arc, FiringArc::Turret);
    }

    #[test]
    fn cwis_profile_values() {
        let p = WeaponType::CWIS.profile();
        assert_eq!(p.damage, 2);
        assert_eq!(p.fire_rate_secs, 0.1);
        assert_eq!(p.firing_range, 100.0);
        assert_eq!(p.projectile_speed, 200.0);
        assert_eq!(p.spread_degrees, 2.0);
        assert_eq!(p.pd_cylinder_radius, 100.0);
        assert_eq!(p.arc, FiringArc::Turret);
    }

    #[test]
    fn vls_mount_sizes() {
        assert_eq!(WeaponType::HeavyVLS.mount_size(), MountSize::Large);
        assert_eq!(WeaponType::LightVLS.mount_size(), MountSize::Medium);
        assert_eq!(WeaponType::LaserPD.mount_size(), MountSize::Medium);
        assert_eq!(WeaponType::CWIS.mount_size(), MountSize::Small);
    }

    #[test]
    fn mount_size_fits_same_size() {
        assert!(MountSize::Large.fits(MountSize::Large));
        assert!(MountSize::Medium.fits(MountSize::Medium));
        assert!(MountSize::Small.fits(MountSize::Small));
    }

    #[test]
    fn mount_size_fits_smaller() {
        assert!(MountSize::Large.fits(MountSize::Medium));
        assert!(MountSize::Large.fits(MountSize::Small));
        assert!(MountSize::Medium.fits(MountSize::Small));
    }

    #[test]
    fn mount_size_rejects_larger() {
        assert!(!MountSize::Small.fits(MountSize::Medium));
        assert!(!MountSize::Small.fits(MountSize::Large));
        assert!(!MountSize::Medium.fits(MountSize::Large));
    }

    #[test]
    fn weapon_categories() {
        assert_eq!(WeaponType::HeavyCannon.category(), WeaponCategory::Cannon);
        assert_eq!(WeaponType::Cannon.category(), WeaponCategory::Cannon);
        assert_eq!(WeaponType::Railgun.category(), WeaponCategory::Cannon);
        assert_eq!(WeaponType::HeavyVLS.category(), WeaponCategory::Missile);
        assert_eq!(WeaponType::LightVLS.category(), WeaponCategory::Missile);
        assert_eq!(WeaponType::LaserPD.category(), WeaponCategory::PointDefense);
        assert_eq!(WeaponType::CWIS.category(), WeaponCategory::PointDefense);
    }

    #[test]
    fn search_radar_is_sensor() {
        assert_eq!(WeaponType::SearchRadar.category(), WeaponCategory::Sensor);
    }

    #[test]
    fn nav_radar_is_sensor() {
        assert_eq!(WeaponType::NavRadar.category(), WeaponCategory::Sensor);
    }

    #[test]
    fn search_radar_mount_size_medium() {
        assert_eq!(WeaponType::SearchRadar.mount_size(), MountSize::Medium);
    }

    #[test]
    fn nav_radar_mount_size_small() {
        assert_eq!(WeaponType::NavRadar.mount_size(), MountSize::Small);
    }

    #[test]
    fn search_radar_range_800() {
        assert_eq!(WeaponType::SearchRadar.profile().firing_range, 800.0);
    }

    #[test]
    fn nav_radar_range_500() {
        assert_eq!(WeaponType::NavRadar.profile().firing_range, 500.0);
    }
}
