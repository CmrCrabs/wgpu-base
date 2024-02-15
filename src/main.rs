use std::time::Instant;
use std::{mem, slice, io::BufReader};
use std::fs::File;
use image::GenericImageView;
use obj::{Obj, TexturedVertex};
use winit::{event::*, event_loop::EventLoop, window::WindowBuilder};
use wgpu::{util::DeviceExt, BindGroupLayoutDescriptor, BindGroupLayoutEntry};
use glam::{Vec3, Vec2, Mat4};
use log::LevelFilter;

type Result<T = (), E = Box<dyn std::error::Error>> = std::result::Result<T, E>;
const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Bgra8UnormSrgb;

#[repr(C, align(16))]
struct Vertex {
  position: Vec3,
  tex_coords: Vec2,
}

struct Camera {
  eye: Vec3,
  target: Vec3,
  up: Vec3,
  aspect: f32,
  fovy: f32,
  znear: f32,
  zfar: f32,
}

fn main() -> Result {
  // WINDOW
  env_logger::builder().filter_level(LevelFilter::Info).init();

  let event_loop = EventLoop::new()?;
  let window = WindowBuilder::new().with_title(":3").build(&event_loop)?;

  let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::default());
  let surface = unsafe { instance.create_surface(&window) }?;
  let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
    power_preference: wgpu::PowerPreference::default(),
    force_fallback_adapter: false,
    compatible_surface: Some(&surface),
  }))
  .unwrap();

  let (device, queue) = pollster::block_on(adapter.request_device(
    &wgpu::DeviceDescriptor {
      features: wgpu::Features::empty(),
      limits: wgpu::Limits::default(),
      label: None,
    },
    None,
  ))
  .unwrap();

  // TIME
  let start_time = Instant::now();

  let time_buf = device.create_buffer(&wgpu::BufferDescriptor {
    size: 4,
    mapped_at_creation: false,
    usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
    label: None,
  });
  let time_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
    entries: &[wgpu::BindGroupLayoutEntry {
      binding: 0,
      visibility: wgpu::ShaderStages::VERTEX,
      ty: wgpu::BindingType::Buffer {
        ty: wgpu::BufferBindingType::Uniform,
        has_dynamic_offset: false,
        min_binding_size: None,
      },
      count: None,
    }],
    label: None,
  });
  let time_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
    layout: &time_bind_group_layout,
    entries: &[wgpu::BindGroupEntry {
      binding: 0,
      resource: time_buf.as_entire_binding(),
    }],
    label: None,
  });

  // CAMERA
  let mut camera = Camera {
    eye: (0.0, 1.0, 30.0).into(),
    target: (0.0, 0.0, 0.0).into(),
    up: Vec3::new(0.0, 1.0, 0.0),
    aspect: window.inner_size().width as f32 / window.inner_size().height as f32,
    fovy: 45.0,
    znear: 0.1,
    zfar: 100.0,
  };

  // CAMERA UNIFORM
  let view = Mat4::look_at_rh(camera.eye, camera.target, camera.up);
  let proj = Mat4::perspective_rh(
    camera.fovy.to_radians(),
    camera.aspect,
    camera.znear,
    camera.zfar,
  );
  let camera_uniform = proj * view;

  let camera_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
    contents: cast_slice(&[camera_uniform]),
    usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
    label: None,
  });

  let camera_bind_group_layout =
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
      entries: &[wgpu::BindGroupLayoutEntry {
        binding: 0,
        visibility: wgpu::ShaderStages::VERTEX,
        ty: wgpu::BindingType::Buffer {
          ty: wgpu::BufferBindingType::Uniform,
          has_dynamic_offset: false,
          min_binding_size: None,
        },
        count: None,
      }],
      label: None,
    });

  let camera_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
    layout: &camera_bind_group_layout,
    entries: &[wgpu::BindGroupEntry {
      binding: 0,
      resource: camera_buffer.as_entire_binding(),
    }],
    label: None,
  });

  //OBJ
  let input = BufReader::new(File::open("assets/dinosaur.obj")?);
  let obj: Obj<TexturedVertex, u32> = obj::load_obj(input)?;
  let vertices = &obj
    .vertices
    .iter()
    .map(|v| Vertex {
      position: v.position.into(),
      tex_coords: Vec2::new(v.texture[0], 1.0 - v.texture[1]),
    })
    .collect::<Vec<_>>();

  // OBJ TEXTURE
  let diffuse_bytes = include_bytes!("../assets/DinosaurTexture.png");
  let diffuse_image = image::load_from_memory(diffuse_bytes).unwrap();
  let diffuse_rgba = diffuse_image.to_rgba32f();
  let dimensions = diffuse_image.dimensions();
  let texture_size = wgpu::Extent3d {
    width: dimensions.0,
    height: dimensions.1,
    depth_or_array_layers: 1,
  };

  // TEXTURE
  let texture = device.create_texture_with_data(
    &queue,
    &wgpu::TextureDescriptor {
      size: texture_size,
      mip_level_count: 1,
      sample_count: 1,
      dimension: wgpu::TextureDimension::D2,
      format: wgpu::TextureFormat::Rgba32Float,
      usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
      label: None,
      view_formats: &[],
    },
    cast_slice(&diffuse_rgba.as_raw()),
  );

  let diffuse_texture_view = texture.create_view(&wgpu::TextureViewDescriptor::default());
  let diffuse_sampler = device.create_sampler(&wgpu::SamplerDescriptor::default());

  let texture_bind_group_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
    entries: &[
      BindGroupLayoutEntry {
        binding: 0,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Texture {
          sample_type: wgpu::TextureSampleType::Float { filterable: false },
          view_dimension: wgpu::TextureViewDimension::D2,
          multisampled: false,
        },
        count: None,
      },
      BindGroupLayoutEntry {
        binding: 1,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::NonFiltering),
        count: None,
      },
    ],
    label: None,
  });

  let diffuse_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
    layout: &texture_bind_group_layout,
    entries: &[
      wgpu::BindGroupEntry {
        binding: 0,
        resource: wgpu::BindingResource::TextureView(&diffuse_texture_view),
      },
      wgpu::BindGroupEntry {
        binding: 1,
        resource: wgpu::BindingResource::Sampler(&diffuse_sampler),
      },
    ],
    label: None,
  });

  let vtx_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
    contents: cast_slice(&vertices),
    usage: wgpu::BufferUsages::VERTEX,
    label: None,
  });

  let idx_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
    contents: cast_slice(&obj.indices),
    usage: wgpu::BufferUsages::INDEX,
    label: None,
  });

  // DEPTH BUFFER
  const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
  let size = wgpu::Extent3d {
    width: window.inner_size().width,
    height: window.inner_size().height,
    depth_or_array_layers: 1,
  };
  let depth_texture = device.create_texture(&wgpu::TextureDescriptor {
    label: None,
    size,
    mip_level_count: 1,
    sample_count: 1,
    dimension: wgpu::TextureDimension::D2,
    format: DEPTH_FORMAT,
    usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
    view_formats: &[],
  });
  let mut depth_view = depth_texture.create_view(&wgpu::TextureViewDescriptor::default());

  // SHADERS
  let shader = device.create_shader_module(wgpu::include_spirv!(env!("shaders.spv")));

  // PIPELINE
  let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
    bind_group_layouts: &[
      &texture_bind_group_layout,
      &camera_bind_group_layout,
      &time_bind_group_layout,
    ],
    push_constant_ranges: &[],
    label: None,
  });

  let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
    layout: Some(&pipeline_layout),
    vertex: wgpu::VertexState {
      module: &shader,
      entry_point: "main_vs",
      buffers: &[wgpu::VertexBufferLayout {
        array_stride: mem::size_of::<Vertex>() as _,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x2],
      }],
    },
    fragment: Some(wgpu::FragmentState {
      module: &shader,
      entry_point: "main_fs",
      targets: &[Some(wgpu::ColorTargetState {
        format: FORMAT,
        blend: Some(wgpu::BlendState::ALPHA_BLENDING),
        write_mask: wgpu::ColorWrites::ALL,
      })],
    }),
    primitive: wgpu::PrimitiveState::default(),
    depth_stencil: Some(wgpu::DepthStencilState {
      format: DEPTH_FORMAT,
      depth_write_enabled: true,
      depth_compare: wgpu::CompareFunction::Less,
      stencil: wgpu::StencilState::default(),
      bias: wgpu::DepthBiasState::default(),
    }),
    multisample: wgpu::MultisampleState::default(),
    multiview: None,
    label: None,
  });

  // EVENT LOOP
  event_loop.run(move |event, elwt| match event {
    Event::WindowEvent { event, .. } => match event {
      WindowEvent::CloseRequested => elwt.exit(),

      WindowEvent::RedrawRequested => {
        let duration = start_time.elapsed();
        queue.write_buffer(&time_buf, 0, cast_slice(&[duration.as_secs_f32()]));
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());

        let surface = surface.get_current_texture().unwrap();
        let surface_view = surface
          .texture
          .create_view(&wgpu::TextureViewDescriptor::default());

        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
          color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view: &surface_view,
            resolve_target: None,
            ops: wgpu::Operations {
              load: wgpu::LoadOp::Clear(wgpu::Color {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
              }),
              store: wgpu::StoreOp::Store,
            },
          })],

          depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
            view: &depth_view,
            depth_ops: Some(wgpu::Operations {
              load: wgpu::LoadOp::Clear(1.0),
              store: wgpu::StoreOp::Store,
            }),
            stencil_ops: None,
          }),
          timestamp_writes: None,
          occlusion_query_set: None,
          label: None,
        });

        render_pass.set_pipeline(&pipeline);
        render_pass.set_bind_group(0, &diffuse_bind_group, &[]);
        render_pass.set_bind_group(1, &camera_bind_group, &[]);
        render_pass.set_bind_group(2, &time_bind_group, &[]);
        render_pass.set_vertex_buffer(0, vtx_buf.slice(..));
        render_pass.set_index_buffer(idx_buf.slice(..), wgpu::IndexFormat::Uint32);

        render_pass.draw_indexed(0..(obj.indices.len() as _), 0, 0..1);
        drop(render_pass);
        queue.submit([encoder.finish()]);
        surface.present();
      }

      WindowEvent::Resized(size) => {
        let config = wgpu::SurfaceConfiguration {
          usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
          format: FORMAT,
          width: size.width,
          height: size.height,
          present_mode: wgpu::PresentMode::Fifo,
          alpha_mode: wgpu::CompositeAlphaMode::Opaque,
          view_formats: vec![],
        };
        surface.configure(&device, &config);

        //FIXING FOV
        camera.aspect = window.inner_size().width as f32 / window.inner_size().height as f32;
        let proj = Mat4::perspective_rh(
          camera.fovy.to_radians(),
          camera.aspect,
          camera.znear,
          camera.zfar,
        );
        let camera_uniform = proj * view;
        queue.write_buffer(&camera_buffer, 0, cast_slice(&[camera_uniform]));

        // FIXING DEPTH BUFFER
        let size = wgpu::Extent3d {
          width: window.inner_size().width,
          height: window.inner_size().height,
          depth_or_array_layers: 1,
        };
        let depth_texture = device.create_texture(&wgpu::TextureDescriptor {
          label: None,
          size,
          mip_level_count: 1,
          sample_count: 1,
          dimension: wgpu::TextureDimension::D2,
          format: DEPTH_FORMAT,
          usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
          view_formats: &[],
        });
        depth_view = depth_texture.create_view(&wgpu::TextureViewDescriptor::default());
      }
      _ => {}
    },
    Event::AboutToWait => window.request_redraw(),
    _ => {}
  })?;
  Ok(())
}

fn cast_slice<T>(fake: &[T]) -> &[u8] {
  unsafe { slice::from_raw_parts(fake.as_ptr() as _, mem::size_of_val(fake)) }
}
