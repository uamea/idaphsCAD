use crate::kernel::TruckRenderer;
use crate::lyon_renderer::LyonLineRenderer;
use cad_core::{AppContext, CadData};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use truck_meshalgo::prelude::*;
use truck_modeling::{EdgeID, FaceID};
use truck_platform::wgpu;
use truck_platform::*;
use truck_rendimpl::*;

pub enum XYZPlane {
    XY,
    YZ,
    ZX,
}

pub struct SceneRenderer {
    pub ctx: Arc<AppContext>,
    pub scene: Scene,
    pub face_instances: HashMap<FaceID, PolygonInstance>,
    pub edge_instances: HashMap<EdgeID, WireFrameInstance>,
    pub lyon_renderer: LyonLineRenderer,
}

/*
* Default
* rendering_size: (640, 480)
* cam_pos: Point3::new(4.5, 4.5, 4.5),
* base_axis: Vector3::unit_y(),
* cam_perspective: ProjectionMethod(Rad(PI / 4.0)),
* near_clip: 0.1,
* far_clip: 40.0,
* background_color: wgpu::Color {
                        r: 230.0 / 256.0,
                        g: 230.0 / 256.0,
                        b: 230.0 / 256.0,
                        a: 0.8,
                    }
*
*
* */

impl SceneRenderer {
    pub fn new(ctx: Arc<AppContext>, device_handler: DeviceHandler) -> Self {
        let rendering_size = (
            ctx.config.read().window_width,
            ctx.config.read().window_height,
        );
        let conf = ctx.config.read().cam_light_config.clone();

        let camera = Camera {
            matrix: Matrix4::look_at_rh(conf.init_cam_pos, Point3::origin(), conf.base_axis)
                .invert()
                .unwrap(),
            method: conf.cam_perspective,
            near_clip: conf.near_clip,
            far_clip: conf.far_clip,
        };

        let lights = vec![Light {
            position: conf.init_cam_pos, // same as the camera
            color: Vector3::new(1.0, 1.0, 1.0),
            light_type: LightType::Point,
        }];

        let mut scene = Scene::new(
            device_handler.clone(),
            &SceneDescriptor {
                studio: StudioConfig {
                    camera,
                    lights,
                    background: conf.background_color,
                },
                backend_buffer: BackendBufferConfig {
                    depth_test: true,
                    sample_count: 1,
                },
                render_texture: RenderTextureConfig {
                    canvas_size: rendering_size,
                    format: wgpu::TextureFormat::Rgba8Unorm,
                },
            },
        );

        let mut face_instances = HashMap::new();
        let mut edge_instances = HashMap::new();

        {
            let data = ctx.read_model();

            // Add faces
            for (face_id, mesh) in &data.face_meshes {
                let instance: PolygonInstance = scene.instance_creator().create_instance(
                    mesh,
                    &PolygonState {
                        material: Material {
                            albedo: Vector4::new(0.75, 0.75, 0.75, 1.0),
                            reflectance: 0.2,
                            roughness: 0.2,
                            ambient_ratio: 0.02,
                            ..Default::default()
                        },
                        ..Default::default()
                    },
                );
                scene.add_object(&instance);
                face_instances.insert(*face_id, instance);
            }

            // Add edges
            for (edge_id, points) in &data.edge_meshes {
                if points.len() >= 2 {
                    let instance: WireFrameInstance = scene.instance_creator().create_instance(
                        &vec![(points[0], points[1])],
                        &WireFrameState {
                            color: Vector4::new(0.2, 0.2, 0.2, 1.0),
                            ..Default::default()
                        },
                    );
                    scene.add_object(&instance);
                    edge_instances.insert(*edge_id, instance);
                }
            }
        }

        let mut renderer = Self {
            ctx,
            scene,
            face_instances,
            edge_instances,
            lyon_renderer: LyonLineRenderer::new(device_handler.device()),
        };

        // テスト用のダミー線を1本流してみる場合
        let dummy_lines = vec![vec![
            Point3::new(0.0, 0.0, 0.0),
            Point3::new(100.0, 1.0, 0.0),
        ]];
        renderer
            .lyon_renderer
            .update_buffers(renderer.scene.device(), &dummy_lines);
        renderer
    }

    fn render_from_cad_data(&mut self) {
        let data = self.ctx.read_model();

        // Add faces
        for (face_id, mesh) in &data.face_meshes {
            let instance: PolygonInstance = self.scene.instance_creator().create_instance(
                mesh,
                &PolygonState {
                    material: Material {
                        albedo: Vector4::new(0.75, 0.75, 0.75, 1.0),
                        reflectance: 0.2,
                        roughness: 0.2,
                        ambient_ratio: 0.02,
                        ..Default::default()
                    },
                    ..Default::default()
                },
            );
            self.scene.add_object(&instance);
            self.face_instances.insert(*face_id, instance);
        }

        // Add edges
        for (edge_id, points) in &data.edge_meshes {
            if points.len() >= 2 {
                let instance: WireFrameInstance = self.scene.instance_creator().create_instance(
                    &vec![(points[0], points[1])],
                    &WireFrameState {
                        color: Vector4::new(0.2, 0.2, 0.2, 1.0),
                        ..Default::default()
                    },
                );
                self.scene.add_object(&instance);
                self.edge_instances.insert(*edge_id, instance);
            }
        }
    }
}

impl TruckRenderer for SceneRenderer {
    fn render_fn(&mut self, view: &wgpu::TextureView) {
        // Sync selection state colors before rendering
        {
            {
                let cam_light_layout = self.ctx.read_cam_light_layout();
                let (camera, lights): (&mut Camera, &mut Vec<Light>) = {
                    let studio = self.scene.studio_config_mut();

                    (&mut studio.camera, &mut studio.lights)
                };
                camera.matrix = cam_light_layout.cam_orit;
                lights[0].position = cam_light_layout.light_pos;
            }

            // self.scene.clear_objects();
            // self.render_from_cad_data();

            let data = self.ctx.read_model();

            // Reset all faces to default
            for instance in self.face_instances.values_mut() {
                instance.instance_state_mut().material.albedo = Vector4::new(0.75, 0.75, 0.75, 1.0);
                self.scene.update_bind_group(instance);
            }
            // Highlight selected faces
            for face_id in &data.selection.selected_faces {
                if let Some(instance) = self.face_instances.get_mut(face_id) {
                    instance.instance_state_mut().material.albedo =
                        Vector4::new(0.55, 0.74, 0.78, 1.0);
                    self.scene.update_bind_group(instance);
                }
            }

            // Reset all edges to default
            for instance in self.edge_instances.values_mut() {
                instance.instance_state_mut().color = Vector4::new(0.2, 0.2, 0.2, 1.0);
                self.scene.update_bind_group(instance);
            }
            // Highlight selected edges
            for edge_id in &data.selection.selected_edges {
                if let Some(instance) = self.edge_instances.get_mut(edge_id) {
                    instance.instance_state_mut().color = Vector4::new(0.55, 0.74, 0.78, 1.0); // Red highlight
                    self.scene.update_bind_group(instance);
                }
            }
        }

        self.scene.render(view);

        // move on to lyon overlay rendering
        let sc_desc = self.scene.descriptor();
        let aspect = sc_desc.render_texture.canvas_size.0 as f64
            / sc_desc.render_texture.canvas_size.1 as f64;
        let camera_matrix = sc_desc.studio.camera.projection(aspect)
            * sc_desc.studio.camera.matrix.invert().unwrap();
        self.lyon_renderer
            .update_camera(self.scene.queue(), camera_matrix);

        // 4. 生の wgpu CommandEncoder を手動で立ち上げてlyonの重ね書きを行う
        let mut encoder =
            self.scene
                .device()
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("Lyon Overlay Encoder"),
                });

        {
            let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Lyon Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load, // ★超重要: truckが描画した結果を消さずに保持して「上書き」する
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None, // 2Dオーバーレイのため深度は不要
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            self.lyon_renderer.draw(&mut rpass);
        }

        // 5. 新しく追加したlyonの描画コマンドをキューに送信
        self.scene.queue().submit(Some(encoder.finish()));
    }

    fn resize(&mut self, width: u32, height: u32) {
        // SceneDescriptorMut's Drop implementation automatically recreates
        // the depth texture to match the new canvas size.
        self.scene.descriptor_mut().render_texture.canvas_size = (width, height);
    }
}
