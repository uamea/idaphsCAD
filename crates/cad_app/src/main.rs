// Prevent console window in addition to Slint window in Windows release builds when, e.g., starting the app via file manager. Ignored on other platforms. #![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod baseop;
mod menubar;
mod messages;
mod msg_macro;
mod selection_mode;
mod tool;

use crate::baseop::setup_baseops_controls;
use crate::menubar::setup_menubar_controls;
use crate::selection_mode::SelectionTool;
use crate::tool::{CadTool, ToolMode, ToolResult};
use cad_core::*;
use cad_renderer::*;
use messages::{AppMessage, InputMessage};
use std::cell::RefCell;
use std::f64::consts::PI;
use std::rc::Rc;
use std::sync::Arc;
use truck_meshalgo::prelude::*;
use truck_platform::wgpu::{self};
use truck_platform::*;

slint::include_modules!();

pub enum ActiveTool {
    Selection(SelectionTool),
    Extrusion,
    Sketch,
}

#[derive(Debug, Default)]
pub struct UserIOUtils {
    pub is_mouse_down: bool,
    pub is_middle_mouse_down: bool,
    pub is_shift_pressed: bool,
    pub is_ctrl_pressed: bool,
    pub current_pos: Option<(f64, f64)>,
    pub prev_pos: Option<(f64, f64)>,
    pub key_pressed: std::collections::HashSet<String>,
}

impl UserIOUtils {
    pub fn update_mouse_pos(&mut self, x: f64, y: f64) {
        self.prev_pos = self.current_pos;
        self.current_pos = Some((x, y));
    }

    pub fn get_mouse_delta(&self) -> Option<(f64, f64)> {
        if let (Some((prev_x, prev_y)), Some((curr_x, curr_y))) = (self.prev_pos, self.current_pos)
        {
            Some((curr_x - prev_x, curr_y - prev_y))
        } else {
            None
        }
    }
}

struct SceneEventHandler {
    ctx: Arc<AppContext>,
    active_tool: ActiveTool,
    user_io_utils: UserIOUtils,
}

impl SceneEventHandler {
    #[inline]
    fn read_config(&self) -> Arc<AppConfig> {
        self.ctx.config.read()
    }

    fn move_to_tool(&mut self, new_tool: ToolMode) {
        self.active_tool = match new_tool {
            ToolMode::Selection => ActiveTool::Selection(SelectionTool::new(self.ctx.clone())),
            ToolMode::Sketch => ActiveTool::Sketch,
            ToolMode::Extrusion => ActiveTool::Extrusion,
        };
    }

    fn preprocess_userio_event(&mut self, input_msg: InputMessage) -> bool {
        use InputMessage::*;
        match input_msg {
            Click { x, y } => {
                self.user_io_utils.is_mouse_down = true;
                self.user_io_utils.update_mouse_pos(x as f64, y as f64);
            }
            MouseMove { x, y, modifiers } => {
                self.user_io_utils.update_mouse_pos(x as f64, y as f64);
            }
            Wheel { delta } => {}
            MiddleClickUp => {
                self.user_io_utils.is_middle_mouse_down = false;
            }
            MiddleClickDown { x, y } => {
                self.user_io_utils.is_middle_mouse_down = true;
                self.user_io_utils.update_mouse_pos(x as f64, y as f64);
            }
            KeyEventPressed { key, modifiers } => {
                self.user_io_utils.is_shift_pressed = modifiers.shift;
                self.user_io_utils.is_ctrl_pressed = modifiers.control;
                self.user_io_utils.key_pressed.insert(key);
            }
            KeyEventReleased { key, modifiers } => {
                self.user_io_utils.is_shift_pressed = modifiers.shift;
                self.user_io_utils.is_ctrl_pressed = modifiers.control;
                self.user_io_utils.key_pressed.remove(&key);
            }
        }

        false
    }
}

impl EventHandler<AppMessage> for SceneEventHandler {
    fn handle_event(&mut self, event: AppMessage) -> bool {
        let mut needs_render = match event.clone() {
            AppMessage::FileIOMsg { msg } => self.handle_menubar_event(msg),
            AppMessage::InputMsg { msg } => self.preprocess_userio_event(msg),
            _ => false,
        };

        use crate::ActiveTool::*;
        let result = match &mut self.active_tool {
            Selection(tool) => tool.handle_event(event, &mut self.user_io_utils),
            Sketch => ToolResult::Continue(false),
            Extrusion => ToolResult::Continue(false),
        };

        if let ToolResult::MoveTo(_, new_mode) = result {
            self.move_to_tool(new_mode);
        }

        true
    }

    fn update_renderer(&self, _renderer: &mut dyn TruckRenderer) {}
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // create the cad context
    // let cad_data = CadData::new();
    // use truck_modeling::builder;
    // let cad_data = CadData::from_solid(builder::tsweep(
    //     &builder::tsweep(
    //         &builder::tsweep(
    //             &builder::vertex(Point3::new(-1.0, 0.0, -1.0)),
    //             1.0 * Vector3::unit_z(),
    //         ),
    //         1.0 * Vector3::unit_x(),
    //     ),
    //     1.0 * Vector3::unit_y(),
    // ));

    let cad_data = CadData::new();

    let app_conf = AppConfig {
        window_width: 640,
        window_height: 480,
        debug_mode: false,
        cam_light_config: CamLightConfig {
            init_cam_pos: Point3::new(4.5, 4.5, 4.5),
            base_axis: Vector3::unit_y(),
            cam_perspective: ProjectionMethod::Perspective { fov: Rad(PI / 4.0) },
            near_clip: 0.1,
            far_clip: 40.0,
            background_color: wgpu::Color {
                r: 230.0 / 256.0,
                g: 230.0 / 256.0,
                b: 230.0 / 256.0,
                a: 0.8,
            },
            pan_sensitivity: 0.005,
            rot_sensitivity: 0.005,
        },
    };

    let conf_manager = ConfigManager::new(app_conf);

    let cam_light_layout = CameraLightLayout {
        cam_orit: Matrix4::look_at_rh(
            conf_manager.read().cam_light_config.init_cam_pos,
            Point3::new(0.0, 0.0, 0.0),
            conf_manager.read().cam_light_config.base_axis,
        ),
        light_pos: conf_manager.read().cam_light_config.init_cam_pos,
        pivot: Point3::new(0.0, 0.0, 0.0),
        perspective: conf_manager.read().cam_light_config.cam_perspective,
    };

    let ctx = Arc::new(AppContext::new(conf_manager, cad_data, cam_light_layout));

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

                    let renderer = SceneRenderer::new(ctx.clone(), device_handler.clone());
                    let active_tool = ActiveTool::Selection(SelectionTool::new(ctx.clone()));

                    let event_handler = SceneEventHandler {
                        ctx: ctx.clone(),
                        active_tool,
                        user_io_utils: UserIOUtils::default(),
                    };

                    let channels = run_truck_kernel_with_slint(
                        device_handler.clone(),
                        Box::new(renderer),
                        Box::new(event_handler),
                        (
                            ctx.config.read().window_width,
                            ctx.config.read().window_height,
                        ),
                    );

                    *truck_channels_setup.borrow_mut() = Some(channels);

                    if let Some((_, control_message_sender)) =
                        truck_channels_setup.borrow().as_ref()
                    {
                        // setup_toolbar_controls(app_weak.clone(), control_message_sender.clone());
                        setup_baseops_controls(app_weak.clone(), control_message_sender.clone());
                        setup_menubar_controls(app_weak.clone(), control_message_sender.clone())
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
