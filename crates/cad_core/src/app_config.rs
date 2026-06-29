use arc_swap::ArcSwap;
use std::sync::Arc;
use truck_modeling::*;
use truck_platform::*;

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub window_width: u32,
    pub window_height: u32,
    pub debug_mode: bool,
    pub cam_light_config: CamLightConfig,
}

#[derive(Debug, Clone)]
pub struct CamLightConfig {
    pub init_cam_pos: Point3,
    pub base_axis: Vector3,
    pub cam_perspective: ProjectionMethod,
    pub near_clip: f64,
    pub far_clip: f64,
    pub background_color: wgpu::Color,
    pub pan_sensitivity: f64,
    pub rot_sensitivity: f64,
}

pub struct ConfigManager {
    current_config: ArcSwap<AppConfig>,
}

impl ConfigManager {
    pub fn new(initial: AppConfig) -> Self {
        Self {
            current_config: ArcSwap::from_pointee(initial),
        }
    }

    /// read config
    pub fn read(&self) -> Arc<AppConfig> {
        self.current_config.load_full()
    }

    /// update config
    pub fn update(&self, new_config: AppConfig) {
        self.current_config.store(Arc::new(new_config));
    }

    pub fn update_mut<F>(&self, f: F)
    where
        F: FnOnce(&mut AppConfig),
    {
        // copy the current state and apply the function to it
        let mut cloned = self.read().as_ref().clone();
        f(&mut cloned);
        // swap with new value
        self.update(cloned);
    }
}
