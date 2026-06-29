use crate::*;
use kernel::{EventHandler, TruckKernelContext, TruckRenderer};
use messages::ControlMessage;
use slint::wgpu_27::wgpu;
use truck_platform::DeviceHandler;

/// adapter function to run the truck kernel in a separate thread and communicate with it via
/// channels.
pub fn run_truck_kernel_with_slint<T>(
    device_handler: DeviceHandler,
    truck_renderer: Box<dyn TruckRenderer>,
    event_handler: Box<dyn EventHandler<T>>,
    initial_buffer_size: (u32, u32),
) -> (
    smol::channel::Receiver<wgpu::Texture>,
    smol::channel::Sender<ControlMessage<T>>,
)
where
    T: Clone + Send + 'static,
{
    let (control_message_sender, control_message_receiver) =
        smol::channel::bounded::<ControlMessage<T>>(2);
    let (truck_front_buffer_sender, truck_front_buffer_receiver) =
        smol::channel::bounded::<wgpu::Texture>(2);

    let wgpu_device = device_handler.device().clone();

    let create_texture = move |label, width, height| {
        wgpu_device.create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        })
    };

    let front_buffer = create_texture("Front Buffer", initial_buffer_size.0, initial_buffer_size.1);
    let back_buffer = create_texture("Back Buffer", initial_buffer_size.0, initial_buffer_size.1);
    let inflight_buffer = create_texture(
        "Inflight Buffer",
        initial_buffer_size.0,
        initial_buffer_size.1,
    );

    let mut ctx = TruckKernelContext {
        renderer: truck_renderer,
        scene: event_handler,
        buffer_width: initial_buffer_size.0,
        buffer_height: initial_buffer_size.1,
    };

    let _truck_thread = std::thread::spawn(move || {
        let mut texture_pool: Vec<wgpu::Texture> = Vec::with_capacity(3);
        let mut needs_render = false;

        loop {
            let first_msg = match control_message_receiver.recv_blocking() {
                Ok(m) => m,
                Err(_) => break,
            };

            let mut process_message =
                |msg: ControlMessage<T>, pool: &mut Vec<wgpu::Texture>| -> bool {
                    match msg {
                        ControlMessage::AppMsg(msg) => ctx.scene.handle_event(msg),
                        ControlMessage::ResizeBuffers { width, height } => {
                            if ctx.buffer_width == width && ctx.buffer_height == height {
                                return false;
                            }
                            ctx.buffer_width = width;
                            ctx.buffer_height = height;
                            ctx.renderer.resize(width, height);
                            true
                        }
                        // needs_render is false here because the texture is just being released
                        // back to the pool, not rendered
                        ControlMessage::ReleaseFrontBufferTexture { texture } => {
                            pool.push(texture);
                            false
                        }
                        ControlMessage::KickOff => {
                            // needs_render is true here because we want to kick off the rendering process
                            true
                        }
                    }
                };

            // handle the first message
            needs_render |= process_message(first_msg, &mut texture_pool);

            // handle the rest of the messages in the queue without blocking
            while let Ok(msg) = control_message_receiver.try_recv() {
                needs_render |= process_message(msg, &mut texture_pool);
            }

            if needs_render && !texture_pool.is_empty() {
                let mut texture = texture_pool.pop().unwrap();
                if texture.width() != ctx.buffer_width || texture.height() != ctx.buffer_height {
                    texture =
                        create_texture("Recreated Buffer", ctx.buffer_width, ctx.buffer_height);
                }

                let view = texture.create_view(&wgpu::TextureViewDescriptor {
                    label: Some("slint front buffer texture view"),
                    format: None,
                    dimension: None,
                    ..Default::default()
                });

                ctx.scene.update_renderer(&mut *ctx.renderer);
                ctx.renderer.render_fn(&view);
                device_handler.queue().submit(std::iter::empty());

                // send the texture to slint front buffer
                let _ = truck_front_buffer_sender.send_blocking(texture);

                // reset the flag
                needs_render = false;
            }
        }
    });

    // send the first three textures to the truck kernel so it can use them for rendering
    control_message_sender
        .send_blocking(ControlMessage::ReleaseFrontBufferTexture {
            texture: back_buffer,
        })
        .unwrap_or_else(|_| panic!("Failed to send initial back buffer"));

    control_message_sender
        .send_blocking(ControlMessage::ReleaseFrontBufferTexture {
            texture: inflight_buffer,
        })
        .unwrap_or_else(|_| panic!("Failed to send initial inflight buffer"));

    control_message_sender
        .send_blocking(ControlMessage::ReleaseFrontBufferTexture {
            texture: front_buffer,
        })
        .unwrap_or_else(|_| panic!("Failed to send initial front buffer"));

    // hack: send a ResizeBuffers message to the truck kernel to kick off the rendering process
    control_message_sender
        .send_blocking(ControlMessage::KickOff)
        .unwrap_or_else(|_| panic!("Failed to send initial message for kickoff"));

    (truck_front_buffer_receiver, control_message_sender)
}
