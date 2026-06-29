use crate::cad_data::CadData;
use crate::{CadFileManager, ConfigManager};
use std::sync::{Arc, RwLock};

use truck_meshalgo::prelude::*;
use truck_modeling::{Matrix4, Point3};
use truck_platform::ProjectionMethod;

pub struct CameraLightLayout {
    pub cam_orit: Matrix4,
    pub light_pos: Point3,
    pub pivot: Point3,
    pub perspective: ProjectionMethod,
}

impl CameraLightLayout {
    pub fn cam_pos(&self) -> Point3 {
        Point3::from_vec(self.cam_orit[3].truncate())
    }

    pub fn sync_light_with_camera(&mut self) {
        self.light_pos = self.cam_pos();
    }
}

pub struct AppContext {
    pub config: ConfigManager,
    pub cad_data: Arc<RwLock<CadData>>,
    pub cam_light_layout: Arc<RwLock<CameraLightLayout>>,
}

impl AppContext {
    pub fn new(
        config: ConfigManager,
        cad_data: CadData,
        cam_light_layout: CameraLightLayout,
    ) -> Self {
        Self {
            config,
            cad_data: Arc::new(RwLock::new(cad_data)),
            cam_light_layout: Arc::new(RwLock::new(cam_light_layout)),
        }
    }

    pub fn read_model(&self) -> std::sync::RwLockReadGuard<'_, CadData> {
        self.cad_data.read().unwrap()
    }

    pub fn write_model(&self) -> std::sync::RwLockWriteGuard<'_, CadData> {
        self.cad_data.write().unwrap()
    }

    pub fn read_cam_light_layout(&self) -> std::sync::RwLockReadGuard<'_, CameraLightLayout> {
        self.cam_light_layout.read().unwrap()
    }

    pub fn write_cam_light_layout(&self) -> std::sync::RwLockWriteGuard<'_, CameraLightLayout> {
        self.cam_light_layout.write().unwrap()
    }
}
