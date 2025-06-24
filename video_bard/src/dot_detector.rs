use std::num::NonZero;

use bytemuck::{Pod, Zeroable};
use tracing::instrument;
use wgpu::{
    BindGroup, BindGroupDescriptor, BindGroupEntry, BindGroupLayoutDescriptor,
    BindGroupLayoutEntry, BufferBinding, BufferUsages, CommandEncoder, ComputePipeline,
    ComputePipelineDescriptor, Device, Extent3d, Origin3d, PipelineCompilationOptions,
    TexelCopyTextureInfo, Texture, TextureDescriptor, TextureUsages, TextureViewDescriptor,
    include_wgsl,
    util::{BufferInitDescriptor, DeviceExt},
    wgt::BufferDescriptor,
};

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
struct Params {
    filter_dim: i32,
    block_dim: u32,
}

#[derive(Debug)]
pub struct DotDetector {
    dimensions: (u32, u32),
    pipeline: ComputePipeline,
    constants_bind_group: BindGroup,
    inner_buffer_bind_group: BindGroup,
    pub textures: [Texture; 6],
}

impl DotDetector {
    pub fn new(device: &Device, src: &Texture) -> Self {
        let dimensions = (src.width(), src.height());

        // We need a few images to swap back and forth between to do the different processing passes in.
        let textures = [0, 1, 2, 3, 4, 5].map(|_| {
            return device.create_texture(&TextureDescriptor {
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                label: Some("Processing Buffer"),
                mip_level_count: 1,
                size: Extent3d {
                    width: dimensions.0,
                    height: dimensions.1,
                    depth_or_array_layers: 1,
                },
                sample_count: 1,
                view_formats: &[],
                usage: TextureUsages::COPY_DST
                    | TextureUsages::COPY_SRC
                    | TextureUsages::STORAGE_BINDING
                    | TextureUsages::TEXTURE_BINDING
                    | TextureUsages::RENDER_ATTACHMENT,
            });
        });

        let constants_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("canny constants layout"),
            entries: &[
                // Non-filtering sampler
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    // This should match the filterable field of the
                    // corresponding Texture entry above.
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::NonFiltering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    // This should match the filterable field of the
                    // corresponding Texture entry above.
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: Some(NonZero::new(8).unwrap()),
                    },
                    count: None,
                },
            ],
        });

        let inner_buffers_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("inner buffers layout"),
            entries: &[
                // B0
                BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::StorageTexture {
                        access: wgpu::StorageTextureAccess::ReadWrite,
                        format: wgpu::TextureFormat::Rgba8Unorm,
                        view_dimension: wgpu::TextureViewDimension::D2,
                    },
                    count: None,
                },
                // B1
                BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::StorageTexture {
                        access: wgpu::StorageTextureAccess::ReadWrite,
                        format: wgpu::TextureFormat::Rgba8Unorm,
                        view_dimension: wgpu::TextureViewDimension::D2,
                    },
                    count: None,
                },
                // B1
                BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::StorageTexture {
                        access: wgpu::StorageTextureAccess::ReadWrite,
                        format: wgpu::TextureFormat::Rgba8Unorm,
                        view_dimension: wgpu::TextureViewDimension::D2,
                    },
                    count: None,
                },
                // B1
                BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::StorageTexture {
                        access: wgpu::StorageTextureAccess::ReadWrite,
                        format: wgpu::TextureFormat::Rgba8Unorm,
                        view_dimension: wgpu::TextureViewDimension::D2,
                    },
                    count: None,
                },
                // B1
                BindGroupLayoutEntry {
                    binding: 5,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::StorageTexture {
                        access: wgpu::StorageTextureAccess::ReadWrite,
                        format: wgpu::TextureFormat::Rgba8Unorm,
                        view_dimension: wgpu::TextureViewDimension::D2,
                    },
                    count: None,
                },
                // B1
                BindGroupLayoutEntry {
                    binding: 6,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::StorageTexture {
                        access: wgpu::StorageTextureAccess::ReadWrite,
                        format: wgpu::TextureFormat::Rgba8Unorm,
                        view_dimension: wgpu::TextureViewDimension::D2,
                    },
                    count: None,
                },
            ],
        });

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        let settings_buffer = device.create_buffer_init(&BufferInitDescriptor {
            label: Some("canny_settings_buffer"),
            contents: bytemuck::cast_slice(&[dimensions.0 as f32, dimensions.1 as f32]),
            usage: BufferUsages::COPY_DST | BufferUsages::UNIFORM,
        });
        let constants_bind_group = device.create_bind_group(&BindGroupDescriptor {
            label: Some("canny_constants_data"),
            layout: &constants_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Buffer(BufferBinding {
                        buffer: &settings_buffer,
                        offset: 0,
                        size: None,
                    }),
                },
            ],
        });

        let inner_buffer_bind_group = device.create_bind_group(&BindGroupDescriptor {
            label: Some("internal_canny_data"),
            layout: &inner_buffers_layout,
            entries: &[
                BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(
                        &textures[0].create_view(&TextureViewDescriptor::default()),
                    ),
                },
                BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(
                        &textures[1].create_view(&TextureViewDescriptor::default()),
                    ),
                },
                BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(
                        &textures[2].create_view(&TextureViewDescriptor::default()),
                    ),
                },
                BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::TextureView(
                        &textures[3].create_view(&TextureViewDescriptor::default()),
                    ),
                },
                BindGroupEntry {
                    binding: 5,
                    resource: wgpu::BindingResource::TextureView(
                        &textures[4].create_view(&TextureViewDescriptor::default()),
                    ),
                },
                BindGroupEntry {
                    binding: 6,
                    resource: wgpu::BindingResource::TextureView(
                        &textures[5].create_view(&TextureViewDescriptor::default()),
                    ),
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Canny Pipeline Layout"),
            bind_group_layouts: &[&constants_layout, &inner_buffers_layout],
            push_constant_ranges: &[],
        });
        let shader = device.create_shader_module(include_wgsl!("../shaders/canny.wgsl"));

        let pipeline = device.create_compute_pipeline(&ComputePipelineDescriptor {
            label: Some("Canny Pipeline"),
            layout: Some(&pipeline_layout),
            cache: None,
            entry_point: Some("main"),
            module: &shader,
            compilation_options: PipelineCompilationOptions::default(),
        });

        Self {
            dimensions,
            constants_bind_group,
            inner_buffer_bind_group,
            pipeline,
            textures,
        }
    }

    #[instrument(skip(self, encoder))]
    pub fn encode(
        &mut self,
        encoder: &mut CommandEncoder,
        src: &Texture,
        dst: &Texture,
        tex_index: Option<usize>,
    ) {
        encoder.copy_texture_to_texture(
            TexelCopyTextureInfo {
                texture: &src,
                aspect: wgpu::TextureAspect::All,
                mip_level: 0,
                origin: Origin3d { x: 0, y: 0, z: 0 },
            },
            TexelCopyTextureInfo {
                texture: &self.textures[0],
                aspect: wgpu::TextureAspect::All,
                mip_level: 0,
                origin: Origin3d { x: 0, y: 0, z: 0 },
            },
            src.size(),
        );
        {
            let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("Render Pass"),
                timestamp_writes: None,
            });
            compute_pass.set_pipeline(&self.pipeline);
            compute_pass.set_bind_group(0, &self.constants_bind_group, &[]);
            compute_pass.set_bind_group(1, &self.inner_buffer_bind_group, &[]);
            compute_pass.dispatch_workgroups(
                (self.dimensions.0 as f32 / 128 as f32).ceil() as u32,
                (self.dimensions.1 as f32 / 4 as f32).ceil() as u32,
                1,
            );
        }
        encoder.copy_texture_to_texture(
            TexelCopyTextureInfo {
                texture: &src,
                aspect: wgpu::TextureAspect::All,
                mip_level: 0,
                origin: Origin3d { x: 0, y: 0, z: 0 },
            },
            TexelCopyTextureInfo {
                texture: &self.textures[0],
                aspect: wgpu::TextureAspect::All,
                mip_level: 0,
                origin: Origin3d { x: 0, y: 0, z: 0 },
            },
            src.size(),
        );
        encoder.copy_texture_to_texture(
            TexelCopyTextureInfo {
                texture: &self.textures[match tex_index {
                    None => 0,
                    Some(i) => i,
                }],
                aspect: wgpu::TextureAspect::All,
                mip_level: 0,
                origin: Origin3d { x: 0, y: 0, z: 0 },
            },
            TexelCopyTextureInfo {
                texture: &dst,
                aspect: wgpu::TextureAspect::All,
                mip_level: 0,
                origin: Origin3d { x: 0, y: 0, z: 0 },
            },
            dst.size(),
        );
    }
}
