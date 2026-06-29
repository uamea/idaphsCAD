use slint::wgpu_27::wgpu;

pub struct TruckKernelContext<T> {
    pub renderer: Box<dyn TruckRenderer>,
    pub scene: Box<dyn EventHandler<T>>,
    pub buffer_width: u32,
    pub buffer_height: u32,
}

pub trait TruckRenderer: Send + Sync {
    fn render_fn(&mut self, view: &wgpu::TextureView);
    fn resize(&mut self, width: u32, height: u32);
}

pub trait EventHandler<T>: Send + Sync
where
    T: Clone + Send,
{
    fn handle_event(&mut self, msg: T) -> bool;
    fn update_renderer(&self, renderer: &mut dyn TruckRenderer);
}
