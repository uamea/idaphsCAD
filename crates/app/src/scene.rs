use cad_core::cad_data::CadData;
use cad_renderer::messages::AppMessage;
use cad_renderer::slint_truck_adapter::TruckRenderer;
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

pub struct SceneRenderer {
    pub scene: Scene,
    pub cad_data: Arc<Mutex<CadData>>,
    pub is_middle_button_pressed: bool,
    pub last_mouse_pos: Vector2,
    pub pressed_keys: HashSet<String>,
    pub ctrl_pressed: bool,
    pub pivot: Point3,
    pub face_instances: HashMap<FaceID, PolygonInstance>,
    pub edge_instances: HashMap<EdgeID, WireFrameInstance>,
}

impl SceneRenderer {
    pub fn new(device_handler: DeviceHandler, cad_data: Arc<Mutex<CadData>>) -> Self {
        let camera = Camera {
            matrix: Matrix4::look_at_rh(
                Point3::new(4.5, 4.5, 4.5),
                Point3::origin(),
                Vector3::unit_y(),
            )
            .invert()
            .unwrap(),
            method: ProjectionMethod::Parallel { screen_size: 10.0 },
            near_clip: 0.1,
            far_clip: 40.0,
        };

        let lights = vec![Light {
            position: Point3::new(4.5, 4.5, 4.5),
            color: Vector3::new(1.0, 1.0, 1.0),
            light_type: LightType::Point,
        }];

        let mut scene = Scene::new(
            device_handler.clone(),
            &SceneDescriptor {
                studio: StudioConfig {
                    camera,
                    lights,
                    background: wgpu::Color {
                        r: 230.0 / 256.0,
                        g: 230.0 / 256.0,
                        b: 230.0 / 256.0,
                        a: 0.8,
                    },
                },
                backend_buffer: BackendBufferConfig {
                    depth_test: true,
                    sample_count: 1,
                },
                render_texture: RenderTextureConfig {
                    canvas_size: (640, 480),
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
            is_middle_button_pressed: false,
            last_mouse_pos: Vector2::new(0.0, 0.0),
            pressed_keys: HashSet::new(),
            ctrl_pressed: false,
            pivot: Point3::origin(),
            face_instances,
            edge_instances,
        };

        renderer.create_coord_plate(XYZPlane::XY);
        renderer.create_coord_plate(XYZPlane::YZ);
        renderer.create_coord_plate(XYZPlane::ZX);
        renderer.create_coord_axes();

        renderer
    }

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

impl TruckRenderer<AppMessage> for SceneRenderer {
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
    }

    fn handle_event(&mut self, event: AppMessage) {
        match event {
            AppMessage::Cameramsg { msg } => {
                // handle camera msg
            }
            AppMessage::InputMsg { msg } => {
                self.handle_baseops_controls(msg);
            }
            AppMessage::ToolbarMsg { msg } => {
                use crate::messages::ToolbarMessage::*;
                match msg {
                    ExtrusionMsg(extrusion_msg) => self.handle_extrusion_controls(extrusion_msg),
                    SketchMsg(sketch_msg) => self.handle_sketch_controls(sketch_msg),
                }
            }
        }
    }

    fn resize(&mut self, width: u32, height: u32) {
        // SceneDescriptorMut's Drop implementation automatically recreates
        // the depth texture to match the new canvas size.
        self.scene.descriptor_mut().render_texture.canvas_size = (width, height);
    }
}

impl SceneRenderer {
    pub fn look_at_origin(&mut self, from: Point3, to: Point3, dir: Vector3) {
        let (camera, lights): (&mut Camera, &mut Vec<Light>) = {
            let studio = self.scene.studio_config_mut();
            (&mut studio.camera, &mut studio.lights)
        };
        camera.matrix = Matrix4::look_at_rh(from, to, dir).invert().unwrap();
        lights[0].position = camera.position();
        self.pivot = to;
    }
}
