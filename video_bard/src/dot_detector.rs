use bytemuck::{Pod, Zeroable};
use tracing::instrument;
use wgpu::{
    BindGroup, BindGroupDescriptor, BindGroupEntry, BindGroupLayoutDescriptor,
    BindGroupLayoutEntry, CommandEncoder, ComputePipeline, ComputePipelineDescriptor, Device,
    Extent3d, PipelineCompilationOptions, Queue, Texture, TextureDescriptor, TextureUsages,
    TextureViewDescriptor, include_wgsl,
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
    io_bind_group: BindGroup,
    inner_buffer_bind_group: BindGroup,
    pub textures: [Texture; 2],
}

impl DotDetector {
    pub fn new(device: &Device, src: &Texture, dst: &Texture) -> Self {
        let dimensions = (src.width(), src.height());

        // We need a few images to swap back and forth between to do the different processing passes in.
        let textures = [0, 1].map(|_| {
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
                    | TextureUsages::STORAGE_BINDING
                    | TextureUsages::TEXTURE_BINDING
                    | TextureUsages::RENDER_ATTACHMENT,
            });
        });

        // // A buffer with 0 in it. Binding this buffer is used to set `flip` to 0
        // let buffer0 = device.create_buffer_init(&BufferInitDescriptor {
        //     label: Some("Buffer0"),
        //     usage: BufferUsages::UNIFORM,
        //     contents: bytemuck::cast_slice(&[0f32]),
        // });
        // // A buffer with 1 in it. Binding this buffer is used to set `flip` to 1
        // let buffer1 = device.create_buffer_init(&BufferInitDescriptor {
        //     label: Some("Buffer1"),
        //     usage: BufferUsages::UNIFORM,
        //     contents: bytemuck::cast_slice(&[1f32]),
        // });

        // let compute_constants_layout =
        //     device.create_bind_group_layout(&BindGroupLayoutDescriptor {
        //         label: Some("Compute Constants Layout"),
        //         entries: &[
        //             BindGroupLayoutEntry {
        //                 binding: 1,
        //                 visibility: wgpu::ShaderStages::COMPUTE,
        //                 ty: wgpu::BindingType::Buffer {
        //                     ty: wgpu::BufferBindingType::Uniform,
        //                     has_dynamic_offset: false,
        //                     min_binding_size: None,
        //                 },
        //                 count: None,
        //             },
        //         ],
        //     });
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
            ],
        });
        let io_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("canny IO layout"),
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
            ],
        });

        let inner_buffers_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("inner buffers layout"),
            entries: &[
                // B0
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
                // B1
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
        let constants_bind_group = device.create_bind_group(&BindGroupDescriptor {
            label: Some("canny_constants_data"),
            layout: &constants_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Sampler(&sampler),
            }],
        });
        let io_bind_group = device.create_bind_group(&BindGroupDescriptor {
            label: Some("io_canny_data"),
            layout: &io_layout,
            entries: &[
                BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(
                        &src.create_view(&TextureViewDescriptor::default()),
                    ),
                },
                BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(
                        &dst.create_view(&TextureViewDescriptor::default()),
                    ),
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
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Canny Pipeline Layout"),
            bind_group_layouts: &[&constants_layout, &io_layout, &inner_buffers_layout],
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
            io_bind_group,
            inner_buffer_bind_group,
            pipeline,
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
        compute_pass.set_bind_group(0, &self.constants_bind_group, &[]);
        compute_pass.set_bind_group(1, &self.io_bind_group, &[]);
        compute_pass.set_bind_group(2, &self.inner_buffer_bind_group, &[]);
        compute_pass.dispatch_workgroups(
            (self.dimensions.0 as f32 / 128 as f32).ceil() as u32,
            (self.dimensions.1 as f32 / 4 as f32).ceil() as u32,
            1,
        );
    }
}
