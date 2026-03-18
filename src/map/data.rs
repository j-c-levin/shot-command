use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BoundsDef {
    pub half_x: f32,
    pub half_y: f32,
}

impl Default for BoundsDef {
    fn default() -> Self {
        Self {
            half_x: 500.0,
            half_y: 500.0,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SpawnPoint {
    pub team: u8,
    pub position: (f32, f32),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AsteroidDef {
    pub position: (f32, f32),
    pub radius: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ControlPointDef {
    pub position: (f32, f32),
    pub radius: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MapData {
    pub bounds: BoundsDef,
    pub spawns: Vec<SpawnPoint>,
    pub asteroids: Vec<AsteroidDef>,
    pub control_points: Vec<ControlPointDef>,
}

impl Default for MapData {
    fn default() -> Self {
        Self {
            bounds: BoundsDef::default(),
            spawns: Vec::new(),
            asteroids: Vec::new(),
            control_points: Vec::new(),
        }
    }
}

pub fn save_map_data(map: &MapData, path: &Path) -> Result<(), String> {
    let pretty = ron::ser::PrettyConfig::default();
    let s = ron::ser::to_string_pretty(map, pretty).map_err(|e| e.to_string())?;
    std::fs::write(path, s).map_err(|e| e.to_string())
}

pub fn load_map_data(path: &Path) -> Result<MapData, String> {
    let s = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    ron::from_str(&s).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn map_data_roundtrip_ron() {
        let map = MapData {
            bounds: BoundsDef {
                half_x: 600.0,
                half_y: 400.0,
            },
            spawns: vec![
                SpawnPoint {
                    team: 0,
                    position: (-300.0, -300.0),
                },
                SpawnPoint {
                    team: 1,
                    position: (300.0, 300.0),
                },
            ],
            asteroids: vec![AsteroidDef {
                position: (50.0, -75.0),
                radius: 25.0,
            }],
            control_points: vec![ControlPointDef {
                position: (0.0, 0.0),
                radius: 100.0,
            }],
        };

        let pretty = ron::ser::PrettyConfig::default();
        let s = ron::ser::to_string_pretty(&map, pretty).unwrap();
        let loaded: MapData = ron::from_str(&s).unwrap();

        assert_eq!(loaded.bounds.half_x, 600.0);
        assert_eq!(loaded.bounds.half_y, 400.0);
        assert_eq!(loaded.spawns.len(), 2);
        assert_eq!(loaded.spawns[0].team, 0);
        assert_eq!(loaded.spawns[0].position, (-300.0, -300.0));
        assert_eq!(loaded.spawns[1].team, 1);
        assert_eq!(loaded.spawns[1].position, (300.0, 300.0));
        assert_eq!(loaded.asteroids.len(), 1);
        assert_eq!(loaded.asteroids[0].position, (50.0, -75.0));
        assert_eq!(loaded.asteroids[0].radius, 25.0);
        assert_eq!(loaded.control_points.len(), 1);
        assert_eq!(loaded.control_points[0].position, (0.0, 0.0));
        assert_eq!(loaded.control_points[0].radius, 100.0);
    }

    #[test]
    fn map_data_default_is_empty() {
        let map = MapData::default();
        assert_eq!(map.bounds.half_x, 500.0);
        assert_eq!(map.bounds.half_y, 500.0);
        assert!(map.spawns.is_empty());
        assert!(map.asteroids.is_empty());
        assert!(map.control_points.is_empty());
    }

    #[test]
    fn save_and_load_file() {
        let dir = std::env::temp_dir().join("nebulous_test_map_data");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("test_map.ron");

        let map = MapData {
            bounds: BoundsDef {
                half_x: 800.0,
                half_y: 600.0,
            },
            spawns: vec![SpawnPoint {
                team: 0,
                position: (10.0, 20.0),
            }],
            asteroids: vec![AsteroidDef {
                position: (100.0, 200.0),
                radius: 30.0,
            }],
            control_points: vec![ControlPointDef {
                position: (0.0, 0.0),
                radius: 150.0,
            }],
        };

        save_map_data(&map, &path).unwrap();
        let loaded = load_map_data(&path).unwrap();

        assert_eq!(loaded.bounds.half_x, 800.0);
        assert_eq!(loaded.bounds.half_y, 600.0);
        assert_eq!(loaded.spawns.len(), 1);
        assert_eq!(loaded.spawns[0].team, 0);
        assert_eq!(loaded.spawns[0].position, (10.0, 20.0));
        assert_eq!(loaded.asteroids.len(), 1);
        assert_eq!(loaded.asteroids[0].position, (100.0, 200.0));
        assert_eq!(loaded.asteroids[0].radius, 30.0);
        assert_eq!(loaded.control_points.len(), 1);
        assert_eq!(loaded.control_points[0].radius, 150.0);

        // Cleanup
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_dir(&dir);
    }
}
