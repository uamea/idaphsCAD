use crate::lyon_renderer::LyonLineRenderer;
use crate::slint_truck_adapter::TruckRenderer;
use cad_core::{CadData, SelectionState};
use std::collections::{HashMap, HashSet};
use std::f64::consts::PI;
use std::marker::PhantomData;
use std::sync::{Arc, Mutex};
use truck_meshalgo::prelude::*;
use truck_modeling::{EdgeID, FaceID, Shell, Wire};
use truck_platform::wgpu;
use truck_platform::*;
use truck_rendimpl::*;

pub enum XYZPlane {
    XY,
    YZ,
    ZX,
}

pub struct SceneRenderer<T>
where
    T: Send + Clone + 'static,
{
    pub scene: Scene,
    pub event2handle: Mutex<Box<dyn FnMut(T) + Send + 'static>>,
    pub conf: RendererConfig,
    pub cad_data: Arc<Mutex<CadData>>,
    pub face_instances: HashMap<FaceID, PolygonInstance>,
    pub edge_instances: HashMap<EdgeID, WireFrameInstance>,
    pub pivot: Point3, // For camera operations
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
#[derive(Debug, Clone)]
pub struct RendererConfig {
    pub rendering_size: (u32, u32),
    pub cam_pos: Point3,
    pub base_axis: Vector3,
    pub cam_perspective: ProjectionMethod,
    pub near_clip: f64,
    pub far_clip: f64,
    pub background_color: wgpu::Color,
    pub pan_sensitivity: f64,
    pub rot_sensitivity: f64,
}

impl<T> SceneRenderer<T>
where
    T: Send + Clone + 'static,
{
    pub fn new(
        device_handler: DeviceHandler,
        conf: RendererConfig,
        event2handle: Box<dyn FnMut(T) + Send + 'static>,
        cad_data: Arc<Mutex<CadData>>,
    ) -> Self {
        let camera = Camera {
            matrix: Matrix4::look_at_rh(conf.cam_pos, Point3::origin(), conf.base_axis)
                .invert()
                .unwrap(),
            method: conf.cam_perspective,
            near_clip: conf.near_clip,
            far_clip: conf.far_clip,
        };

        let lights = vec![Light {
            position: conf.cam_pos, // same as the camera
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
                    canvas_size: conf.rendering_size,
                    format: wgpu::TextureFormat::Rgba8Unorm,
                },
            },
        );

        let mut face_instances = HashMap::new();
        let mut edge_instances = HashMap::new();

        {
            let data = cad_data.lock().unwrap();

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
            scene,
            cad_data,
            conf,
            event2handle: Mutex::new(Box::new(event2handle)),
            face_instances,
            edge_instances,
            pivot: Point3::origin(),
            lyon_renderer: LyonLineRenderer::new(device_handler.device()),
        };

        renderer.create_coord_plate(XYZPlane::XY);
        renderer.create_coord_plate(XYZPlane::YZ);
        renderer.create_coord_plate(XYZPlane::ZX);
        renderer.create_coord_axes();

        // テスト用のダミー線を1本流してみる場合
        let dummy_lines = vec![vec![
            Point3::new(-5.0, 0.0, 0.0),
            Point3::new(5.0, 5.0, 0.0),
        ]];
        renderer
            .lyon_renderer
            .update_buffers(renderer.scene.device(), &dummy_lines);
        renderer
    }

    // pub fn add_rendering_obj(&mut self, rendering_obj: RenderingData);

    // pub fn project_3dvec(&self, v: Vector3) -> Vector2;
    // pub fn create_dot(&mut self, v: Vector3);
    // pub fn create_dot_planar(&mut self, v: Vector2);
    // pub fn create_line(&mut self, v0: Vector3, v1: Vector3);
    // pub fn create_line_planar(&mut self, v0: Vector3, v1: Vector3);
    // pub fn create_circle(&mut self, center: Vector3, radius: f64);
    // pub fn create_circle_planar(&mut self, center: Vector2, radius: f64);

    pub fn create_coord_axes(&mut self) {
        let length = 2.0;
        let origin = Point3::origin();

        let axes = [
            (
                Point3::new(length, 0.0, 0.0),
                Vector4::new(1.0, 0.0, 0.0, 1.0),
            ), // X: Red
            (
                Point3::new(0.0, length, 0.0),
                Vector4::new(0.0, 1.0, 0.0, 1.0),
            ), // Y: Green
            (
                Point3::new(0.0, 0.0, length),
                Vector4::new(0.0, 0.0, 1.0, 1.0),
            ), // Z: Blue
        ];

        for (end_pt, color) in axes.iter() {
            let instance: WireFrameInstance = self.scene.instance_creator().create_instance(
                &vec![(origin, *end_pt)],
                &WireFrameState {
                    color: *color,
                    ..Default::default()
                },
            );
            self.scene.add_object(&instance);
        }
    }

    pub fn create_coord_plate(&mut self, plane: XYZPlane) {
        let h_size = 5.0;
        let w_size = 5.0;

        use truck_modeling::builder;
        let (v0, v1, v2, v3) = match plane {
            XYZPlane::XY => (
                builder::vertex(Point3::new(-h_size, -w_size, 0.0)),
                builder::vertex(Point3::new(h_size, -w_size, 0.0)),
                builder::vertex(Point3::new(h_size, w_size, 0.0)),
                builder::vertex(Point3::new(-h_size, w_size, 0.0)),
            ),
            XYZPlane::YZ => (
                builder::vertex(Point3::new(0.0, -h_size, -w_size)),
                builder::vertex(Point3::new(0.0, h_size, -w_size)),
                builder::vertex(Point3::new(0.0, h_size, w_size)),
                builder::vertex(Point3::new(0.0, -h_size, w_size)),
            ),
            XYZPlane::ZX => (
                builder::vertex(Point3::new(-w_size, 0.0, -h_size)),
                builder::vertex(Point3::new(-w_size, 0.0, h_size)),
                builder::vertex(Point3::new(w_size, 0.0, h_size)),
                builder::vertex(Point3::new(w_size, 0.0, -h_size)),
            ),
        };

        let edge0 = builder::line(&v0, &v1);
        let edge1 = builder::line(&v1, &v2);
        let edge2 = builder::line(&v2, &v3);
        let edge3 = builder::line(&v3, &v0);

        let wire = Wire::from(vec![edge0, edge1, edge2, edge3]);
        let face = builder::try_attach_plane(vec![wire.clone()]).unwrap();
        let shell = Shell::from(vec![face]);

        let mesh2 = shell.triangulation(0.01).to_polygon();

        let face_desc: PolygonInstance = self.scene.instance_creator().create_instance(
            &mesh2,
            &PolygonState {
                material: Material {
                    albedo: Vector4::new(0.0, 0.0, 1.0, 0.2),
                    roughness: 0.5,
                    reflectance: 0.1,
                    ambient_ratio: 0.5,
                    alpha_blend: true,
                    ..Default::default()
                },
                backface_culling: false,
                ..Default::default()
            },
        );

        let mut line_segments: Vec<(Point3, Point3)> = Vec::new();
        for edge in wire.edge_iter() {
            let curve = edge.curve();
            let p0 = curve.front();
            let p1 = curve.back();
            line_segments.push((p0, p1));
        }

        let edge_desc: WireFrameInstance = self.scene.instance_creator().create_instance(
            &line_segments,
            &WireFrameState {
                color: Vector4::new(0.0, 0.0, 1.0, 1.0),
                ..Default::default()
            },
        );

        self.scene.add_object(&face_desc);
        self.scene.add_object(&edge_desc);
    }
}

impl<T> TruckRenderer<T> for SceneRenderer<T>
where
    T: Send + Clone + 'static,
{
    fn render_fn(&mut self, view: &wgpu::TextureView) {
        // Sync selection state colors before rendering
        {
            let data = self.cad_data.lock().unwrap();

            // Reset all faces to default
            for (_, instance) in &mut self.face_instances {
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
            for (_, instance) in &mut self.edge_instances {
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

    fn handle_event(&mut self, event: T) {
        self.event2handle.lock().unwrap()(event);
    }

    fn resize(&mut self, width: u32, height: u32) {
        // SceneDescriptorMut's Drop implementation automatically recreates
        // the depth texture to match the new canvas size.
        self.scene.descriptor_mut().render_texture.canvas_size = (width, height);
    }
}

impl<T> SceneRenderer<T>
where
    T: Send + Clone + 'static,
{
    pub fn look_at_origin(&mut self, from: Point3, to: Point3, dir: Vector3) {
        let (camera, lights): (&mut Camera, &mut Vec<Light>) = {
            let studio = self.scene.studio_config_mut();
            (&mut studio.camera, &mut studio.lights)
        };
        camera.matrix = Matrix4::look_at_rh(from, to, dir).invert().unwrap();
        lights[0].position = camera.position();
    }
}
