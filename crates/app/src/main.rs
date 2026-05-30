// Prevent console window in addition to Slint window in Windows release builds when, e.g., starting the app via file manager. Ignored on other platforms. #![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod base_ops;
mod cad_data;
mod camera;
mod extrusion;
mod messages;
mod msg_macro;
mod sketch;
mod slint_truck_adapter;
mod toolbar;

use base_ops::setup_baseops_controls;
use messages::{AppMessage, ControlMessage};
use slint_truck_adapter::{TruckRenderer, run_truck_kernel_with_slint};
use std::cell::RefCell;
use std::collections::HashSet;
use std::marker::PhantomData;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use toolbar::setup_toolbar_controls;
use truck_meshalgo::prelude::*;
use truck_modeling::{Edge, Face, Shell, Solid, Vertex, Wire, builder};
use truck_platform::wgpu::{self};
use truck_platform::*;
use truck_rendimpl::*;
slint::include_modules!();
use std::f64::consts::PI;

enum XYZPlane {
    XY,
    YZ,
    ZX,
}

struct MyTruckRenderer<AppMessage> {
    pub scene: Scene,
    pub(crate) is_middle_button_pressed: bool,
    pub(crate) last_mouse_pos: Vector2,
    pub(crate) pressed_keys: HashSet<String>,
    pub(crate) pivot: Point3,
    _marker: PhantomData<AppMessage>,
}

impl MyTruckRenderer<AppMessage> {
    pub fn new(device_handler: DeviceHandler) -> Self {
        let camera = Camera {
            matrix: Matrix4::look_at_rh(
                Point3::new(4.5, 4.5, 4.5),
                Point3::origin(),
                Vector3::unit_y(),
            )
            .invert()
            .unwrap(),
            method: ProjectionMethod::perspective(Rad(PI / 4.0)),
            near_clip: 0.1,
            far_clip: 40.0,
        };

        // ライトの配列
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
                    sample_count: 1, // create_textureのsample_count: 1と合わせる
                },
                render_texture: RenderTextureConfig {
                    canvas_size: (640, 480),                 // 現在のサイズを入れる
                    format: wgpu::TextureFormat::Rgba8Unorm, // create_textureのフォーマットと完全に一致させる
                },
            },
        );

        let vertex: Vertex = builder::vertex(Point3::new(-1.0, 0.0, -1.0));
        let edge: Edge = builder::tsweep(&vertex, 2.0 * Vector3::unit_z());
        let face: Face = builder::tsweep(&edge, 2.0 * Vector3::unit_x());
        let cube: Solid = builder::tsweep(&face, 2.0 * Vector3::unit_y());

        // creates the wireframe of the cube, which is necessary for rendering the edges of the cube
        for edge in cube.edge_iter() {
            let curve = edge.curve();
            let p0 = curve.front();
            let p1 = curve.back();
            let line_instance: WireFrameInstance = scene.instance_creator().create_instance(
                &vec![(p0, p1)],
                &WireFrameState {
                    color: Vector4::new(0.2, 0.2, 0.2, 1.0),
                    ..Default::default()
                },
            );
            scene.add_object(&line_instance);
        }

        // 境界表現のまま変換。引数の0.01はメッシュで近似する際の誤差の目安
        let mesh_with_topology = cube.triangulation(0.01);

        // 面のメッシュを統合して単一のメッシュにする
        let polygon = mesh_with_topology.to_polygon();

        let instance: PolygonInstance = scene.instance_creator().create_instance(
            &polygon,
            &PolygonState {
                // smooth plastic texture
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

        MyTruckRenderer::<AppMessage> {
            scene,
            is_middle_button_pressed: false,
            last_mouse_pos: Vector2::new(0.0, 0.0),
            pressed_keys: HashSet::new(),
            pivot: Point3::new(0.0, 1.5, 0.0),
            _marker: PhantomData,
        }
    }

    pub fn create_coord_plate(&mut self, plane: XYZPlane) {
        // truck_modelingのAPIで頂点、ワイヤー、面を構築
        let h_size = 5.0;
        let w_size = 5.0;

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
            // 直線（Line）であれば始点と終点を取得
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

        // Sceneに登録するためのインスタンスを生成
        self.scene.add_object(&face_desc);
        self.scene.add_object(&edge_desc);
    }
}

impl TruckRenderer<AppMessage> for MyTruckRenderer<AppMessage> {
    fn render_fn(&mut self, view: &wgpu::TextureView) {
        // let time = self.scene.elapsed().as_secs_f64();
        //
        // // カメラとライトのmutableな参照
        // let (camera, lights): (&mut Camera, &mut Vec<Light>) = {
        //     // StudioConfigのmutableな参照を受け取る
        //     let studio = self.scene.studio_config_mut();
        //     // カメラとライトのmutableな参照
        //     (&mut studio.camera, &mut studio.lights)
        // };
        //
        // // 回転行列
        // let rot = Matrix4::from_axis_angle(
        //     // 回転軸
        //     Vector3::unit_y(),
        //     // 毎秒1ラジアン
        //     Rad(time),
        // );
        //
        // // カメラ座標系の更新
        // camera.matrix = rot
        //     * Matrix4::look_at_rh(
        //         Point3::new(5.0, 6.0, 5.0),
        //         Point3::new(0.0, 1.5, 0.0),
        //         Vector3::unit_y(),
        //     )
        //     .invert()
        //     .unwrap();
        //
        // // ライト位置の更新
        // lights[0].position[1] = 6.0 * (time * 0.8).cos();
        // lights[1].position[1] = -6.0 * (time * 0.8).cos();
        // lights[2].position[1] = 6.0 * (time * 0.8).cos();

        self.scene.render(view);
    }

    fn handle_event(&mut self, event: AppMessage) {
        match event {
            AppMessage::Cameramsg { msg } => {
                use messages::CameraMessage::*;
                match msg {
                    Reset => {
                        // カメラをリセットする処理
                    }
                }
            }
            AppMessage::InputMsg { msg } => {
                self.handle_baseops_controls(msg);
            }
            AppMessage::ToolbarMsg { msg } => {
                use messages::ToolbarMessage::*;
                match msg {
                    ExtrusionMsg(extrusion_msg) => self.handle_extrusion_controls(extrusion_msg),
                    SketchMsg(sketch_msg) => self.handle_sketch_controls(sketch_msg),
                }
            }
        }
    }

    fn resize(&mut self, width: u32, height: u32) {
        // 必要に応じてカメラのアスペクト比などを更新
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Slint initializes first with wgpu — on linuxkms it creates its own wgpu instance
    // with DRM surface support. The wgpu resources are then passed to Bevy via the
    // rendering notifier callback.
    let mut wgpu_settings = slint::wgpu_27::WGPUSettings::default();
    wgpu_settings.device_required_limits = slint::wgpu_27::wgpu::Limits::default()
        .using_resolution(slint::wgpu_27::wgpu::Limits::downlevel_defaults());
    slint::BackendSelector::new()
        .require_wgpu_27(slint::wgpu_27::WGPUConfiguration::Automatic(wgpu_settings))
        .select()?;
    let ui = AppWindow::new().unwrap();
    // These will be filled once Truck is initialized from the rendering notifier.
    let truck_channels: Rc<
        RefCell<
            Option<(
                smol::channel::Receiver<slint::wgpu_27::wgpu::Texture>,
                smol::channel::Sender<ControlMessage<AppMessage>>,
            )>,
        >,
    > = Rc::new(RefCell::new(None));

    let app_weak = ui.as_weak();
    let truck_channels_setup = truck_channels.clone();

    let mut shared_renderer: Option<Arc<Mutex<dyn TruckRenderer<AppMessage> + Send>>> = None;

    ui.window()
        .set_rendering_notifier(move |state, graphics_api| {
            match state {
                slint::RenderingState::RenderingSetup => {
                    // Extract wgpu resources from Slint and initialize truck
                    let slint::GraphicsAPI::WGPU27 {
                        instance,
                        device,
                        queue,
                        ..
                    } = graphics_api
                    else {
                        return;
                    };

                    let adapter =
                        smol::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
                            #[cfg(not(feature = "webgl"))]
                            power_preference: wgpu::PowerPreference::HighPerformance,
                            #[cfg(feature = "webgl")]
                            power_preference: PowerPreference::LowPower,
                            compatible_surface: None,
                            force_fallback_adapter: false,
                        }))
                        .expect("Failed to find an appropriate adapter");

                    let device_handler = DeviceHandler::new(adapter, device.clone(), queue.clone());

                    let mut renderer = MyTruckRenderer::new(device_handler.clone());
                    renderer.create_coord_plate(XYZPlane::XY);
                    renderer.create_coord_plate(XYZPlane::YZ);
                    renderer.create_coord_plate(XYZPlane::ZX);
                    shared_renderer = Some(Arc::new(Mutex::new(renderer)));

                    if let Some(ref renderer) = shared_renderer {
                        let channels =
                            run_truck_kernel_with_slint(device_handler.clone(), renderer.clone());

                        *truck_channels_setup.borrow_mut() = Some(channels);
                    }

                    if let Some((_, control_message_sender)) =
                        truck_channels_setup.borrow().as_ref()
                    {
                        setup_toolbar_controls(app_weak.clone(), control_message_sender.clone());
                        setup_baseops_controls(app_weak.clone(), control_message_sender.clone());
                    };
                }
                slint::RenderingState::BeforeRendering => {
                    let Some(app) = app_weak.upgrade() else {
                        return;
                    };

                    let channels = truck_channels_setup.borrow();
                    let Some((new_texture_receiver, control_message_sender)) = channels.as_ref()
                    else {
                        return;
                    };

                    if let Ok(new_texture) = new_texture_receiver.try_recv() {
                        if let Some(old_texture) = app.get_texture().to_wgpu_27_texture() {
                            let _ = control_message_sender.try_send(
                                ControlMessage::ReleaseFrontBufferTexture {
                                    texture: old_texture,
                                },
                            );
                        }
                        if let Ok(image) = new_texture.try_into() {
                            app.set_texture(image);
                        }

                        app.window().request_redraw();
                    } else {
                        app.window().request_redraw();
                        return;
                    }

                    let requested_width = app.get_requested_texture_width().round() as u32;
                    let requested_height = app.get_requested_texture_height().round() as u32;
                    if requested_width > 0 && requested_height > 0 {
                        let control_message_sender = control_message_sender.clone();
                        slint::spawn_local(async move {
                            control_message_sender
                                .send(ControlMessage::ResizeBuffers {
                                    width: requested_width,
                                    height: requested_height,
                                })
                                .await
                                .unwrap();
                        })
                        .unwrap();
                    }
                }
                _ => {}
            }
        })?;

    let _ = ui.run();

    Ok(())
}
