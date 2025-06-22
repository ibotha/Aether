use bytemuck::{Pod, Zeroable};
use image::{DynamicImage, GenericImageView};
use tracing::instrument;
use wgpu::{
    BindGroup, BindGroupDescriptor, BindGroupEntry, BindGroupLayoutDescriptor,
    BindGroupLayoutEntry, BufferBinding, BufferUsages, CommandEncoder, ComputePipeline,
    ComputePipelineDescriptor, Device, Extent3d, PipelineCompilationOptions, Queue, Texture,
    TextureDescriptor, TextureUsages, TextureViewDescriptor, include_wgsl,
    util::{BufferInitDescriptor, DeviceExt},
    wgt::TextureDataOrder,
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
    block_size: u32,
    iterations: u32,
    filter_size: i32,
    batch: [u32; 2],
    pipeline: ComputePipeline,
    compute_constants: BindGroup,
    initial_compute_data: BindGroup,
    vertical_compute_data: BindGroup,
    horizontal_compute_data: BindGroup,
    image_texture: Texture,
    pub textures: [Texture; 2],
}

impl DotDetector {
    pub fn new(queue: &Queue, device: &Device, src: &DynamicImage) -> Self {
        const TILE_DIM: u32 = 128;
        let filter_size = 15i32;
        let iterations = 2;
        let dimensions = src.dimensions();
        let src_rgba = src.to_rgba8();
        let image_texture = device.create_texture_with_data(
            queue,
            &TextureDescriptor {
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                label: Some("InitialTexture"),
                mip_level_count: 1,
                size: Extent3d {
                    width: dimensions.0,
                    height: dimensions.1,
                    depth_or_array_layers: 1,
                },
                usage: TextureUsages::TEXTURE_BINDING
                    | TextureUsages::COPY_DST
                    | TextureUsages::COPY_SRC,
                sample_count: 1,
                view_formats: &[],
            },
            TextureDataOrder::LayerMajor,
            &src_rgba,
        );

        let textures = [0, 1].map(|_| {
            return device.create_texture(&TextureDescriptor {
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                label: Some("InitialTexture"),
                mip_level_count: 1,
                size: Extent3d {
                    width: dimensions.0,
                    height: dimensions.1,
                    depth_or_array_layers: 1,
                },
                sample_count: 1,
                view_formats: &[],
                usage: TextureUsages::COPY_DST
                    | TextureUsages::STORAGE_BINDING
                    | TextureUsages::TEXTURE_BINDING
                    | TextureUsages::RENDER_ATTACHMENT,
            });
        });

        // A buffer with 0 in it. Binding this buffer is used to set `flip` to 0
        let buffer0 = device.create_buffer_init(&BufferInitDescriptor {
            label: Some("Buffer1"),
            usage: BufferUsages::UNIFORM,
            contents: bytemuck::cast_slice(&[0f32]),
        });
        // A buffer with 1 in it. Binding this buffer is used to set `flip` to 1
        let buffer1 = device.create_buffer_init(&BufferInitDescriptor {
            label: Some("Buffer1"),
            usage: BufferUsages::UNIFORM,
            contents: bytemuck::cast_slice(&[1f32]),
        });

        let compute_constants_layout =
            device.create_bind_group_layout(&BindGroupLayoutDescriptor {
                label: Some("Compute Constants Layout"),
                entries: &[
                    BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::NonFiltering),
                        count: None,
                    },
                    BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                ],
            });

        let blur_pass_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("Blur Pass Layout"),
            entries: &[
                // Source
                BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                // Destination
                BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::StorageTexture {
                        access: wgpu::StorageTextureAccess::WriteOnly,
                        format: wgpu::TextureFormat::Rgba8Unorm,
                        view_dimension: wgpu::TextureViewDimension::D2,
                    },
                    count: None,
                },
                // Flip
                BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        let block_size = TILE_DIM - filter_size as u32;

        let blur_params_buffer = device.create_buffer_init(&BufferInitDescriptor {
            label: Some("Params"),
            contents: bytemuck::cast_slice(&[Params {
                filter_dim: filter_size + 1,
                block_dim: block_size,
            }]),
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
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
        let compute_constants = device.create_bind_group(&BindGroupDescriptor {
            label: Some("Compute Constants"),
            layout: &compute_constants_layout,
            entries: &[
                BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Buffer(BufferBinding {
                        buffer: &blur_params_buffer,
                        offset: 0,
                        size: None,
                    }),
                },
            ],
        });

        let initial_compute_data = device.create_bind_group(&BindGroupDescriptor {
            label: Some("initial_compute_data"),
            layout: &blur_pass_layout,
            entries: &[
                BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(
                        &image_texture.create_view(&TextureViewDescriptor::default()),
                    ),
                },
                BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(
                        &textures[0].create_view(&TextureViewDescriptor::default()),
                    ),
                },
                BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::Buffer(BufferBinding {
                        buffer: &buffer0,
                        offset: 0,
                        size: None,
                    }),
                },
            ],
        });

        let vertical_compute_data = device.create_bind_group(&BindGroupDescriptor {
            label: Some("vertical_compute_data"),
            layout: &blur_pass_layout,
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
                    resource: wgpu::BindingResource::Buffer(BufferBinding {
                        buffer: &buffer1,
                        offset: 0,
                        size: None,
                    }),
                },
            ],
        });

        let horizontal_compute_data = device.create_bind_group(&BindGroupDescriptor {
            label: Some("horizontal_compute_data"),
            layout: &blur_pass_layout,
            entries: &[
                BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(
                        &textures[1].create_view(&TextureViewDescriptor::default()),
                    ),
                },
                BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(
                        &textures[0].create_view(&TextureViewDescriptor::default()),
                    ),
                },
                BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::Buffer(BufferBinding {
                        buffer: &buffer0,
                        offset: 0,
                        size: None,
                    }),
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Render Pipeline Layout"),
            bind_group_layouts: &[&compute_constants_layout, &blur_pass_layout],
            push_constant_ranges: &[],
        });
        let shader = device.create_shader_module(include_wgsl!("../shaders/canny.wgsl"));

        let pipeline = device.create_compute_pipeline(&ComputePipelineDescriptor {
            label: Some("Blur Pipeline"),
            layout: Some(&pipeline_layout),
            cache: None,
            entry_point: Some("main"),
            module: &shader,
            compilation_options: PipelineCompilationOptions::default(),
        });

        Self {
            dimensions: src.dimensions(),
            block_size,
            iterations,
            filter_size,
            batch: [4, 4],
            compute_constants,
            initial_compute_data,
            horizontal_compute_data,
            vertical_compute_data,
            pipeline,
            image_texture,
            textures,
        }
    }

    #[instrument(skip(self, encoder))]
    pub fn encode(&mut self, encoder: &mut CommandEncoder) {
        let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("Render Pass"),
            timestamp_writes: None,
        });
        compute_pass.set_pipeline(&self.pipeline);
        compute_pass.set_bind_group(0, &self.compute_constants, &[]);

        compute_pass.set_bind_group(1, &self.initial_compute_data, &[]);
        compute_pass.dispatch_workgroups(
            (self.dimensions.0 as f32 / self.block_size as f32).ceil() as u32,
            (self.dimensions.1 as f32 / self.batch[1] as f32).ceil() as u32,
            1,
        );

        compute_pass.set_bind_group(1, &self.vertical_compute_data, &[]);
        compute_pass.dispatch_workgroups(
            (self.dimensions.1 as f32 / self.block_size as f32).ceil() as u32,
            (self.dimensions.0 as f32 / self.batch[1] as f32).ceil() as u32,
            1,
        );

        for _ in 0..self.iterations {
            compute_pass.set_bind_group(1, &self.horizontal_compute_data, &[]);
            compute_pass.dispatch_workgroups(
                (self.dimensions.0 as f32 / self.block_size as f32).ceil() as u32,
                (self.dimensions.1 as f32 / self.batch[1] as f32).ceil() as u32,
                1,
            );

            compute_pass.set_bind_group(1, &self.vertical_compute_data, &[]);
            compute_pass.dispatch_workgroups(
                (self.dimensions.1 as f32 / self.block_size as f32).ceil() as u32,
                (self.dimensions.0 as f32 / self.batch[1] as f32).ceil() as u32,
                1,
            );
        }
    }
}
