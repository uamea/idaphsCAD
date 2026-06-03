// Prevent console window in addition to Slint window in Windows release builds when, e.g., starting the app via file manager. Ignored on other platforms. #![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod base_ops;
mod cad_data;
mod camera;
mod extrusion;
mod messages;
mod msg_macro;
mod scene;
mod selection;
mod sketch;
mod slint_truck_adapter;
mod toolbar;

use base_ops::setup_baseops_controls;
use cad_data::CadData;
use messages::{AppMessage, ControlMessage};
use scene::SceneRenderer;
use slint_truck_adapter::{run_truck_kernel_with_slint, TruckRenderer};
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use toolbar::setup_toolbar_controls;
use truck_meshalgo::prelude::*;
use truck_modeling::{Edge, Face, Solid, Vertex, builder};
use truck_platform::wgpu::{self};
use truck_platform::*;

slint::include_modules!();

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

                    let vertex: Vertex = builder::vertex(Point3::new(-1.0, 0.0, -1.0));
                    let edge: Edge = builder::tsweep(&vertex, 2.0 * Vector3::unit_z());
                    let face: Face = builder::tsweep(&edge, 2.0 * Vector3::unit_x());
                    let cube: Solid = builder::tsweep(&face, 2.0 * Vector3::unit_y());

                    let cad_data = Arc::new(Mutex::new(CadData::new(cube)));
                    let renderer = SceneRenderer::new(device_handler.clone(), cad_data);
                    
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
