use std::ops::Rem;
use std::rc::Rc;
use futures::Future;
use wgpu::ShaderStages;
use bytemuck;

use crate::renderer::webgpu::Utils;
use crate::timer::ScopedTimer;
use crate::GraphicsLevel;

use super::pipeline_loader::RenderPipelineFlatDescriptor;
use super::preprocessor::Preprocessor;
use super::shader_loader::{FragmentShaderVariant, VertexShaderVariant};
use super::webgpu::buffer::{Buffer, UniformBuffer};
use super::webgpu::utils::PipelineLayoutBuilder;
use super::webgpu::uniform::BindGroupInfo;
use super::{DemoLoadingFuture, DemoLoadingSimpleFuture, Dispose, ExternalState, IDemo, LoadingArgs, Progress, RenderArgs, SimpleFuture, Webgpu};

const VERTEX_SHADER_VARIANT: VertexShaderVariant = VertexShaderVariant::TriangleFullscreen;
// const FRAGMENT_SHADER_VARIANT: FragmentShaderVariant = FragmentShaderVariant::Uv;
const FRAGMENT_SHADER_VARIANT: FragmentShaderVariant = FragmentShaderVariant::FractalMandelbrot;

#[derive(Default)]
enum DemoLoadingStage {
   Ready = 0,
   #[default] CreateShaders,
   CreatePipelines,
   CreateUniforms,
   StartSwitchingGraphicsLevel,
   SwitchGraphicsLevel,
}

struct DemoLoadingProcess {
   stage: DemoLoadingStage,
   stage_percent: f32,
   graphics_level: GraphicsLevel,
   loading_args: LoadingArgs,
   render_pipelines: Option<FractalRenderPipelines>,
   vertex_shader: Option<Rc<wgpu::ShaderModule>>,
   fragment_shader_default: Option<Rc<wgpu::ShaderModule>>,
   fragment_shader_antialiasing: Option<Rc<wgpu::ShaderModule>>,
   uniform_groups: Vec<BindGroupInfo>,
   fractal_uniform_buffer: Option<UniformBuffer>,
   loaded_demo: Option<Demo>,
}

impl Dispose for DemoLoadingProcess {
   fn dispose(&mut self) {
      match self.stage {
         DemoLoadingStage::Ready => {
            // demo is fully loaded, its lifetime is now separate, 
            // shouldn't free its resources
         },
         _ => {
            self.render_pipelines.take();
            self.vertex_shader.take();
            self.fragment_shader_default.take();
            self.fragment_shader_antialiasing.take();
            self.stage = DemoLoadingStage::Ready;
            self.loaded_demo.take();
            log::warn!("Rust loading drop {} {}", std::module_path!(), self.stage_percent)
         },
      }
   }
}

impl Drop for DemoLoadingProcess {
   fn drop(&mut self) {
      self.dispose();
   }
}

impl Progress for DemoLoadingProcess {
    fn progress(&self) -> f32 {
        self.stage_percent
    }
}

impl SimpleFuture for DemoLoadingProcess {
   type Output = Box<dyn IDemo>;

   fn simple_poll(mut self: std::pin::Pin<&mut Self>, _cx: &mut std::task::Context) -> std::task::Poll<Self::Output> {
      use DemoLoadingStage::*;
      match self.stage {
         CreateShaders => {
            if self.vertex_shader.is_none() {
               self.compile_shader_vert();
            } else if self.fragment_shader_default.is_none() {
               self.compile_shader_frag_default();
            } else if self.fragment_shader_antialiasing.is_none() {
               self.compile_shader_frag_aa();
            } else {
               self.stage = CreateUniforms;
            }
            self.stage_percent += 0.1;
            std::task::Poll::Pending
         },
         CreateUniforms => {
            self.make_bind_groups();
            self.stage_percent += 0.1;
            self.stage = CreatePipelines;
            std::task::Poll::Pending
         }
         CreatePipelines => {
            self.build_pipelines();
            self.stage_percent += 0.1;
            self.stage = StartSwitchingGraphicsLevel;
            std::task::Poll::Pending
         }
         StartSwitchingGraphicsLevel => {
            self.switch_graphics_level();
            self.stage_percent = 0.75;
            self.stage = SwitchGraphicsLevel;
            std::task::Poll::Pending
         }
         SwitchGraphicsLevel => {
            let webgpu = self.loading_args.webgpu.clone();
            match self.loaded_demo.as_mut().unwrap().poll_switching_graphics_level(webgpu.as_ref()) {
               Ok(std::task::Poll::Pending)  => {
                  self.stage_percent = 0.75 + 0.25 * self.loaded_demo.as_ref().unwrap().progress_switching_graphics_level();
                  self.stage = SwitchGraphicsLevel;
                  std::task::Poll::Pending
               }
               Ok(std::task::Poll::Ready(())) => {
                  self.stage_percent = 1.0;
                  self.stage = Ready;
                  std::task::Poll::Ready(Box::new(
                     self.loaded_demo.take().unwrap()
                  ))
               },
               Err(e) => {
                  eprintln!("Error when switching graphics level: {}: {}", std::module_path!(), e);
                  std::task::Poll::Pending // hopefully will work next frame
               }
            }
         }
         Ready => unreachable!("Should not poll the task again after std::task::Poll::Ready was polled"),
      }
   }
}

impl Future for DemoLoadingProcess {
   type Output = Box<dyn IDemo>;
   
   fn poll(mut self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<Self::Output> {
      match self.as_mut().simple_poll(cx) {
         std::task::Poll::Pending => {
            cx.waker().wake_by_ref();
            std::task::Poll::Pending
         }
         poll => poll,
      }
   }
}

impl DemoLoadingSimpleFuture for DemoLoadingProcess {}

impl DemoLoadingFuture for DemoLoadingProcess {}


impl DemoLoadingProcess {
   fn new(loading_args: LoadingArgs, graphics_level: GraphicsLevel) -> Self {
      Self {
         stage: Default::default(),
         stage_percent: 0.0,
         graphics_level,
         loading_args,
         render_pipelines: Default::default(),
         vertex_shader: Default::default(),
         fragment_shader_default: Default::default(),
         fragment_shader_antialiasing: Default::default(),
         uniform_groups: Default::default(),
         fractal_uniform_buffer: Default::default(),
         loaded_demo: Default::default(),
      }
   }

   #[allow(unused)]
   fn compile_shaders(&mut self) {
      self.compile_shader_vert();
      self.compile_shader_frag_default();
      self.compile_shader_frag_aa();
   }

   fn rebuild_pipelines(&mut self) {
      self.compile_shaders();
      self.make_bind_groups();
      self.build_pipelines();
   }

   fn compile_shader_vert(&mut self) {
      let _t = ScopedTimer::new("compile_shader_vert");
      let vertex_shader = self.loading_args.get_vertex_shader(VERTEX_SHADER_VARIANT, None);
      self.vertex_shader = Some(vertex_shader);
   }

   fn compile_shader_frag_default(&mut self) {
      let _t = ScopedTimer::new("compile_shader_frag_default");
      let mut preprocessor = Preprocessor::new();
      let fragment_shader_default = self.loading_args.get_fragment_shader(FRAGMENT_SHADER_VARIANT, Some(&mut preprocessor));
      self.fragment_shader_default = Some(fragment_shader_default);
   }

   fn compile_shader_frag_aa(&mut self) {
      let _t = ScopedTimer::new("compile_shader_frag_aa");
      let mut preprocessor = Preprocessor::new();
      preprocessor.define("USE_ANTIALIASING", "1");
      let fragment_shader_antialiasing = self.loading_args.get_fragment_shader(FRAGMENT_SHADER_VARIANT, Some(&mut preprocessor));
      self.fragment_shader_antialiasing = Some(fragment_shader_antialiasing);
   }

   fn make_bind_groups(&mut self) {
      let _t = ScopedTimer::new("create_uniforms");

      let fractal_buffer = Buffer::new_uniform::<FractalUniformData>(
         &self.loading_args.webgpu.device,
         wgpu::BufferUsages::COPY_DST,
         Some("Fractal Bind Buffer"));
      let fractal_uniform_group = BindGroupInfo::builder()
         .with_uniform_buffer(0, ShaderStages::FRAGMENT, &fractal_buffer.buffer)
         .build(&self.loading_args.webgpu.device, Some("Fractal Bind Group"), None);
      self.fractal_uniform_buffer = Some(fractal_buffer);

      self.uniform_groups = vec![fractal_uniform_group];
   }

   fn build_pipelines(&mut self) {
      let _t = ScopedTimer::new("create_pipelines");
      let mut builder = PipelineLayoutBuilder::new();
      let premade = self.loading_args.premade.borrow();
      builder = builder.with(&premade.global_uniform.bind_group_info);
      for group in self.uniform_groups.iter() {
         builder = builder.with(group);
      }
      let pipeline_layout_descr = builder.build_descriptor(Some("Render Pipeline Layout")).clone();
      let vs = self.vertex_shader.take().unwrap();
      let fs = self.fragment_shader_default.take().unwrap();
      let fs_aa = self.fragment_shader_antialiasing.take().unwrap();
      self.render_pipelines = Some(FractalRenderPipelines{
         default: self.build_render_pipeline("Render Pipeline - Default",
            &pipeline_layout_descr, &vs, &fs),
         antialiasing: self.build_render_pipeline("Render Pipeline - AA",
            &pipeline_layout_descr, &vs, &fs_aa),
      });
   }

   fn build_render_pipeline(&self, label: &str, layout_descriptor: &wgpu::PipelineLayoutDescriptor, vs: &wgpu::ShaderModule, fs: &wgpu::ShaderModule) -> Rc<wgpu::RenderPipeline> {
      let _t = ScopedTimer::new("create_render_pipeline");
      let render_pipeline_layout = self.loading_args.webgpu.device.create_pipeline_layout(&layout_descriptor);
      self.loading_args.get_pipeline(&RenderPipelineFlatDescriptor::new(
         // &self.uniform_groups,
         layout_descriptor,
         &wgpu::RenderPipelineDescriptor {
            label: Some(label),
            layout: Some(&render_pipeline_layout),
            vertex: wgpu::VertexState {
                  module: vs,
                  entry_point: "vs_main",
                  buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                  module: fs,
                  entry_point: "fs_main",
                  targets: &[Some(wgpu::ColorTargetState {
                     format: self.loading_args.color_texture_format,
                     blend: Some(wgpu::BlendState::REPLACE),
                     write_mask: wgpu::ColorWrites::ALL,
                  })],
            }),
            primitive: Utils::default_primitive_state(),
            depth_stencil: None, // 1.
            multisample: wgpu::MultisampleState {
               count: 1,
               mask: !0,
               alpha_to_coverage_enabled: false,
            },
            multiview: None,
         },
      ))
   }

   fn switch_graphics_level(&mut self) {
      let default_fractal_zoom = 2.0;
      let mut loaded_demo = Demo {
         current_graphics_level: self.graphics_level,
         render_pipelines: self.render_pipelines.take().unwrap(),
         use_antialiasing: false,
         pending_graphics_level_switch: None,
         default_fractal_zoom,
         fractal_uniform_data: FractalUniformData {
            fractal_center: [-1.1900443, 0.3043895],
            fractal_zoom: default_fractal_zoom,
            num_iterations: 1000,
            color_bias: [3.4, 3.1, 2.5],
            color_power: 0.22,
         },
         fractal_buffer_offset: 0,
         fractal_uniform_buffer: self.fractal_uniform_buffer.take().unwrap(),
         uniform_groups: vec![],
      };
      std::mem::swap(&mut loaded_demo.uniform_groups, &mut self.uniform_groups);
      self.loaded_demo = Some(loaded_demo);
      let loading_args = self.loading_args.clone();
      self.loaded_demo.as_mut().unwrap()
            .start_switching_graphics_level(loading_args, self.graphics_level)
            .expect("WebGPU surface error");
   }
}

struct FractalRenderPipelines {
   default: Rc<wgpu::RenderPipeline>,
   antialiasing: Rc<wgpu::RenderPipeline>,
}
pub struct Demo {
   current_graphics_level: GraphicsLevel,
   render_pipelines: FractalRenderPipelines,
   use_antialiasing: bool,
   pending_graphics_level_switch: Option<GraphicsSwitchingProcess>,
   default_fractal_zoom: f32,
   uniform_groups: Vec<BindGroupInfo>,
   fractal_uniform_data: FractalUniformData,
   fractal_uniform_buffer: UniformBuffer,
   fractal_buffer_offset: u64,
}

#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct FractalUniformData {
   fractal_center: [f32; 2],
   fractal_zoom: f32,
   num_iterations: i32,
   color_bias: [f32; 3],
   color_power: f32,
}

impl IDemo for Demo {
   fn tick(&mut self, input: &ExternalState) {
      const DEMO_LENGTH_SECONDS: f64 = 45.0;
      let zoom_scale = (-0.3*input.time_now_sec().rem(DEMO_LENGTH_SECONDS)).exp() as f32;
      if input.debug_mode() < Some(2) {
         self.fractal_uniform_data.fractal_zoom = self.default_fractal_zoom * zoom_scale;
      }
   }

   fn render(&mut self, args: RenderArgs) -> Result<(), wgpu::SurfaceError> {
      {
         self.fractal_uniform_buffer.write(&args.webgpu.queue, self.fractal_buffer_offset, &[self.fractal_uniform_data]);
      }

      let color = Some(Utils::surface_view(args.backbuffer));
      let mut encoder = args.webgpu.device.create_command_encoder(
         &wgpu::CommandEncoderDescriptor { label: Some("Render Encoder"), });

      {
         let mut render_pass = Utils::default_renderpass(&mut encoder, &color, &None);
         render_pass.set_pipeline(
            if self.use_antialiasing { &self.render_pipelines.antialiasing }
            else { &self.render_pipelines.default }
         );
         const GLOBAL_UNIFORM_BIND_GROUP_INDEX: u32 = 0;
         const FRACTAL_UNIFORM_BIND_GROUP_INDEX: u32 = 1;
         render_pass.set_bind_group(GLOBAL_UNIFORM_BIND_GROUP_INDEX, &args.global_uniform.bind_group_info.bind_group, &[]);
         render_pass.set_bind_group(FRACTAL_UNIFORM_BIND_GROUP_INDEX, &self.uniform_groups[0].bind_group, &[]);
         render_pass.draw(0..3, 0..1); // self.num_rendered>vertices
      }
   
      // submit will accept anything that implements IntoIter
      args.webgpu.queue.submit(std::iter::once(encoder.finish()));
      Ok(())
   }

   #[cfg(any(feature = "imgui_win", feature = "imgui_web"))]
   fn render_imgui(&mut self, ui: &imgui::Ui, args: super::imgui_web::ImguiRenderArgs) {
      use imgui::*;
      let window = ui.window("Fractal Demo");
      window
         .size(args.size, Condition::FirstUseEver)
         .position(args.position, Condition::FirstUseEver)
         .always_auto_resize(true)
         .build(|| {
            imgui::Drag::new("Num iterations")
               .range(1, 2000)
               .speed(2.0)
               .build(ui,&mut self.fractal_uniform_data.num_iterations);
            let drag_speed = self.fractal_uniform_data.fractal_zoom * 0.05;
            imgui::Drag::new("Zoom")
               .range(1e-16, 2.0)
               .speed(drag_speed)
               .flags(SliderFlags::LOGARITHMIC)
               .build(ui,&mut self.fractal_uniform_data.fractal_zoom);
            imgui::Drag::new("Center")
               .range(-2.0, 2.0)
               .speed(drag_speed)
               .build_array(ui, &mut self.fractal_uniform_data.fractal_center);
            imgui::Drag::new("Color bias")
               .range(-5.0, 5.0)
               .speed(0.01)
               .build_array(ui, &mut self.fractal_uniform_data.color_bias);
            imgui::Drag::new("Color power")
               .range(-0.15, 1.0)
               .speed(0.005)
               .build(ui,&mut self.fractal_uniform_data.color_power);
            ui.checkbox("Antialiasing", &mut self.use_antialiasing);
            ui.separator();
         });
   }

   fn rebuild_pipelines(&mut self, args: LoadingArgs) {
      let mut loader = DemoLoadingProcess::new(args,
         self.current_graphics_level);
      loader.rebuild_pipelines();
      self.render_pipelines = loader.render_pipelines.take().unwrap();
   }

   fn start_switching_graphics_level(&mut self, _loading_args: LoadingArgs, graphics_level: GraphicsLevel) -> Result<(), wgpu::SurfaceError> {
      // TODO: fix this, the graphics level is not yet finished switching
      self.current_graphics_level = graphics_level;
      log::warn!("Rust start_switching_graphics_level {} {}", std::module_path!(), graphics_level.as_ref());
      self.pending_graphics_level_switch = Some(GraphicsSwitchingProcess{
         progress: 0.0,
         graphics_level,
      });
      Ok(())
   }

   fn poll_switching_graphics_level(&mut self, webgpu: &Webgpu) -> Result<std::task::Poll<()>, wgpu::SurfaceError> {
      match self.pending_graphics_level_switch {
         Some(_) => Ok(GraphicsSwitchingProcess::poll(self, webgpu)),
         _ => Ok(std::task::Poll::Ready(())),
      }
   }

   fn progress_switching_graphics_level(&self) -> f32 {
      self.pending_graphics_level_switch.as_ref()
         .map_or(0.0, |s| s.progress())
   }

   fn drop_demo(&mut self, _webgpu: &Webgpu) {
      log::warn!("Rust demo drop custom {}", std::module_path!());
   }
}

impl Demo {
   pub fn start_loading(args: LoadingArgs, graphics_level: GraphicsLevel) -> Box<dyn DemoLoadingFuture> {
      Box::new(DemoLoadingProcess::new(args, graphics_level))
   }

   pub fn make_command_buffers(&mut self) {
      // precache encoder.begin_render_pass into internal textures here
      // can't precache render into backbuffer, because it requires a ref to TextureView
   }
}

pub struct GraphicsSwitchingProcess {
   progress: f32,
   graphics_level: GraphicsLevel,
}

impl Progress for GraphicsSwitchingProcess {
    fn progress(&self) -> f32 {
      self.progress
   }
}

impl GraphicsSwitchingProcess {
   pub fn poll(demo: &mut Demo, _webgpu: &Webgpu) -> std::task::Poll<()> {
      let self_ = demo.pending_graphics_level_switch.as_mut().unwrap();
      (demo.fractal_uniform_data.num_iterations, demo.use_antialiasing) = match self_.graphics_level {
        GraphicsLevel::Minimal => (75, false),
        GraphicsLevel::Low => (250, false),
        GraphicsLevel::Medium => (500, true),
        GraphicsLevel::High => (1000, true),
        GraphicsLevel::Ultra => (1500, true),
      };
      self_.progress = 1.0;
      demo.make_command_buffers();
      std::task::Poll::Ready(())
   }
}


#[cfg(test)]
mod tests {
   use std::cell::RefCell;
   use crate::renderer::Premade;
   use super::*;

    #[test]
   fn shaders_compile() {
      let webgpu = futures::executor::block_on(Webgpu::new_offscreen());
      let premade = Premade::new(&webgpu.device);
      let loading_args = LoadingArgs {
         webgpu: Rc::new(webgpu),
         color_texture_format: wgpu::TextureFormat::Rgba8Unorm,
         premade: Rc::new(RefCell::new(premade)),
      };
      let mut demo_loader = DemoLoadingProcess::new(loading_args, GraphicsLevel::Medium);
      demo_loader.compile_shaders();
   }
}