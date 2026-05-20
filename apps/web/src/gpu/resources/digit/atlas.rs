use eframe::wgpu;

use super::super::super::digit_atlas::{
    rasterize_digit_sdf_atlas, DIGIT_ATLAS_HEIGHT, DIGIT_ATLAS_WIDTH,
};

pub(super) struct DigitAtlas {
    pub(super) view: wgpu::TextureView,
    pub(super) sampler: wgpu::Sampler,
}

pub(super) fn create_digit_atlas(device: &wgpu::Device, queue: &wgpu::Queue) -> DigitAtlas {
    let atlas_data = rasterize_digit_sdf_atlas();
    let texture = wgpu::util::DeviceExt::create_texture_with_data(
        device,
        queue,
        &wgpu::TextureDescriptor {
            label: Some("measurement_digit_sdf_atlas"),
            size: wgpu::Extent3d {
                width: DIGIT_ATLAS_WIDTH,
                height: DIGIT_ATLAS_HEIGHT,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        },
        wgpu::util::TextureDataOrder::default(),
        &atlas_data,
    );
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("measurement_digit_sampler"),
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        ..Default::default()
    });

    DigitAtlas { view, sampler }
}
