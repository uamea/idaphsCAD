use crate::*;
use messages::ControlMessage;
use slint::wgpu_27::wgpu;
use std::sync::{Arc, Mutex};
use truck_platform::DeviceHandler;

pub trait TruckRenderer<T>: Send + Sync
where
    T: Clone + Send,
{
    fn render_fn(&mut self, view: &wgpu::TextureView);
    fn handle_event(&mut self, event: T);

    fn resize(&mut self, width: u32, height: u32);
}

pub fn get_device_handler(instance: &wgpu::Instance) -> DeviceHandler {
    let adapter = smol::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        #[cfg(not(feature = "webgl"))]
        power_preference: wgpu::PowerPreference::HighPerformance,
        #[cfg(feature = "webgl")]
        power_preference: PowerPreference::LowPower,
        compatible_surface: None,
        force_fallback_adapter: false,
    }))
    .expect("Failed to find an appropriate adapter");

    let (device, queue) = smol::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        required_features: Default::default(),
        memory_hints: wgpu::MemoryHints::Performance,
        #[cfg(not(feature = "webgl"))]
        required_limits: wgpu::Limits::downlevel_defaults().using_resolution(adapter.limits()),
        #[cfg(feature = "webgl")]
        required_limits: Limits::downlevel_webgl2_defaults(),
        label: None,
        trace: wgpu::Trace::Off,
        experimental_features: Default::default(),
    }))
    .expect("Failed to create device");

    DeviceHandler::new(adapter, device, queue)
}

pub fn run_truck_kernel_with_slint<T>(
    device_handler: DeviceHandler,
    mut truck_renderer: Arc<Mutex<dyn TruckRenderer<T>>>,
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

    let front_buffer = create_texture("Front Buffer", 640, 480);
    let back_buffer = create_texture("Back Buffer", 640, 480);
    let inflight_buffer = create_texture("Back Buffer", 640, 480);

    let mut buffer_width = 640;
    let mut buffer_height = 480;

    let _truck_thread = std::thread::spawn(move || {
        loop {
            let msg = match control_message_receiver.recv_blocking() {
                Ok(m) => m,
                Err(_) => break,
            };

            match msg {
                ControlMessage::ReleaseFrontBufferTexture { mut texture } => {
                    if texture.width() != buffer_width || texture.height() != buffer_height {
                        texture = create_texture("Recreated Buffer", buffer_width, buffer_height);
                    }
                    let view = texture.create_view(&wgpu::TextureViewDescriptor {
                        label: Some("slint front buffer texture view"),
                        format: None,
                        dimension: None,
                        ..Default::default()
                    });

                    let mut renderer = truck_renderer.lock().unwrap();
                    renderer.render_fn(&view);

                    device_handler.queue().submit(std::iter::empty());

                    let _ = truck_front_buffer_sender.send_blocking(texture);
                }
                ControlMessage::ResizeBuffers { width, height } => {
                    buffer_width = width;
                    buffer_height = height;

                    let mut renderer = truck_renderer.lock().unwrap();
                    renderer.resize(width, height);
                }
                ControlMessage::AppMsg(msg) => {
                    let mut renderer = truck_renderer.lock().unwrap();
                    renderer.handle_event(msg);
                }
            }
        }
    });

    control_message_sender
        .send_blocking(ControlMessage::ReleaseFrontBufferTexture {
            texture: back_buffer,
        })
        .unwrap_or_else(|_| panic!("Failed to send initial back buffer texture to truck thread"));
    control_message_sender
        .send_blocking(ControlMessage::ReleaseFrontBufferTexture {
            texture: inflight_buffer,
        })
        .unwrap_or_else(|_| {
            panic!("Failed to send initial inflight buffer texture to truck thread")
        });
    control_message_sender
        .send_blocking(ControlMessage::ReleaseFrontBufferTexture {
            texture: front_buffer,
        })
        .unwrap_or_else(|_| panic!("Failed to send initial buffer size to truck thread"));

    (truck_front_buffer_receiver, control_message_sender)
}
