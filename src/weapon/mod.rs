pub mod damage;
pub mod firing;
pub mod missile;
pub mod projectile;

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum MountSize {
    Large,
    Medium,
    Small,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum WeaponType {
    HeavyCannon,
    Cannon,
    Railgun,
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
    pub max_ammo: u16,
}

impl WeaponType {
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
                max_ammo: 60,
            },
            WeaponType::Cannon => WeaponProfile {
                fire_rate_secs: 1.0,
                burst_count: 1,
                damage: 8,
                firing_range: 200.0,
                projectile_speed: 120.0,
                spread_degrees: 2.0,
                arc: FiringArc::Turret,
                max_ammo: 120,
            },
            WeaponType::Railgun => WeaponProfile {
                fire_rate_secs: 7.0,
                burst_count: 1,
                damage: 50,
                firing_range: 1000.0,
                projectile_speed: 300.0,
                spread_degrees: 0.5,
                arc: FiringArc::Forward,
                max_ammo: 10,
            },
        }
    }

    pub fn mount_size(&self) -> MountSize {
        match self {
            WeaponType::HeavyCannon => MountSize::Large,
            WeaponType::Cannon => MountSize::Medium,
            WeaponType::Railgun => MountSize::Large,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct WeaponState {
    pub weapon_type: WeaponType,
    pub ammo: u16,
    pub cooldown: f32,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Mount {
    pub size: MountSize,
    pub offset: Vec2,
    pub weapon: Option<WeaponState>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Component)]
pub struct Mounts(pub Vec<Mount>);

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
}
