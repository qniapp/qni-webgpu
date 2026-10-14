use std::sync::atomic::{AtomicBool, Ordering};

use eframe::{egui, egui_wgpu, wgpu};

use super::super::resources::StateVectorResources;

pub(crate) struct FrameCallback<T> {
    callback: T,
    prepared: AtomicBool,
}

impl<T> FrameCallback<T> {
    pub(crate) fn new(callback: T) -> Self {
        Self {
            callback,
            prepared: AtomicBool::new(false),
        }
    }

    fn begin_prepare(&self, resources_ready: bool) -> bool {
        resources_ready && !self.prepared.swap(true, Ordering::AcqRel)
    }
}

impl<T: egui_wgpu::CallbackTrait> egui_wgpu::CallbackTrait for FrameCallback<T> {
    fn prepare(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        screen_descriptor: &egui_wgpu::ScreenDescriptor,
        egui_encoder: &mut wgpu::CommandEncoder,
        callback_resources: &mut egui_wgpu::CallbackResources,
    ) -> Vec<wgpu::CommandBuffer> {
        if !self.begin_prepare(callback_resources.get::<StateVectorResources>().is_some()) {
            return Vec::new();
        }
        self.callback.prepare(
            device,
            queue,
            screen_descriptor,
            egui_encoder,
            callback_resources,
        )
    }

    fn paint(
        &self,
        info: egui::PaintCallbackInfo,
        render_pass: &mut wgpu::RenderPass<'static>,
        callback_resources: &egui_wgpu::CallbackResources,
    ) {
        self.callback.paint(info, render_pass, callback_resources);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prepares_once_when_resources_are_ready() {
        let callback = FrameCallback::new(());
        assert_eq!(
            [callback.begin_prepare(true), callback.begin_prepare(true)],
            [true, false]
        );
    }

    #[test]
    fn missing_resources_do_not_consume_prepare() {
        let callback = FrameCallback::new(());
        assert_eq!(
            [callback.begin_prepare(false), callback.begin_prepare(true)],
            [false, true]
        );
    }
}
