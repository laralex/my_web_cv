pub mod buffer;
pub mod utils;
use std::{cell::RefCell, rc::Rc};

pub use utils::*;

use super::{pipeline_loader::{PipelineLoader, RenderPipelineFlatDescriptor}, preprocessor::Preprocessor, shader_loader::{FragmentShaderVariant, ShaderLoader, VertexShaderVariant}};
pub mod draw;
pub mod uniform;

const USE_SHADER_CACHE: bool = true;
const USE_PIPELINE_CACHE: bool = true;
pub struct Webgpu {
   // pub surface: wgpu::Surface<'static>,
   pub device: wgpu::Device,
   pub queue: wgpu::Queue,
   shader_loader: RefCell<ShaderLoader>,
   pipeline_loader: RefCell<PipelineLoader>,
}

impl Webgpu {
   #[cfg(feature = "web")]
   pub async fn new_with_canvas(power_preference: wgpu::PowerPreference) -> (web_sys::HtmlCanvasElement, Self, wgpu::Surface<'static>, wgpu::SurfaceConfiguration){
      use wasm_bindgen::JsCast;
      use crate::timer::ScopedTimer;

      let try_init_webgpu = |backend| async move {
         let _t = ScopedTimer::new("webgpu::try_init_webgpu");
         let document = web_sys::window().unwrap().document().unwrap();
         let canvas = document.create_element("canvas").unwrap();
         let canvas = canvas.dyn_into::<web_sys::HtmlCanvasElement>().unwrap();

         let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: backend, ..Default::default() });
         
         // # Safety
         // The surface needs to live as long as the window that created it.
         let surface = instance.create_surface(wgpu::SurfaceTarget::Canvas(canvas.clone()))
            .map_err(|e| web_sys::console::log_2(&"Failed to create wgpu surface: ".into(), &e.to_string().into()))
            .ok();
         if surface.is_none() {
            canvas.remove();
            return None;
         }
         
         let adapter = instance.request_adapter(
            &wgpu::RequestAdapterOptions {
               power_preference,
               compatible_surface: Some(&surface.as_ref().unwrap()),
               force_fallback_adapter: false,
            },
         ).await;
         if adapter.is_none() {
            canvas.remove();
            return None;
         }

         Some((canvas, instance, surface.unwrap(), adapter.unwrap()))
      };
      
      let mut webgpu_artifacts = None;
      let backends_to_try = &[
         // (wgpu::Backends::DX12, "DX12".to_owned()),
         (wgpu::Backends::BROWSER_WEBGPU, "WebGPU".to_owned()),
         // (wgpu::Backends::PRIMARY, "Vulkan/Metal/DX12/WebGPU".to_owned()),
         (wgpu::Backends::GL, "WebGL".to_owned()),
      ];
      for (backend, backend_name) in backends_to_try {
         webgpu_artifacts = try_init_webgpu(backend.clone()).await;
         if webgpu_artifacts.is_some() {
            web_sys::console::log_2(&"Created wgpu surface for backend:".into(), &backend_name.into());
            break;
         }
         web_sys::console::log_2(&"Failed to create wgpu surface for backend:".into(), &backend_name.into());
      }
      let _t = ScopedTimer::new("webgpu::new_with_canvas");
      let (canvas, _instance, surface, adapter) = webgpu_artifacts
         .expect("No wgpu backends are available. Can't start the application");
      let (width, height) = (canvas.width(), canvas.height()); // TODO: newly created, maybe pass size, or resize in init_fn

      let mut device_result = adapter.request_device(
         &Utils::default_device_descriptor(),
         None, // Trace path
      ).await;
      if let Err(e) = device_result {
         web_sys::console::log_2(
            &"Failed to get a webgpu device with default features".into(), &e.to_string().into());
         // fallback to more compatible features
         device_result = adapter.request_device(
            &Utils::downlevel_device_descriptor(),
            None,
         ).await
      }
      let (device, queue) = device_result.expect("Failed to request wgpu device");

      let surface_caps = surface.get_capabilities(&adapter);
      let surface_format = surface_caps.formats.iter()
         .copied()
         .filter(|f| f.is_srgb())
         .next()
         .unwrap_or(surface_caps.formats[0]);

      let config = wgpu::SurfaceConfiguration {
         usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
         width,
         height,
         view_formats: vec![],
         desired_maximum_frame_latency: 2,
         format: surface_format,
         present_mode: surface_caps.present_modes[0],
         alpha_mode: surface_caps.alpha_modes[0],
      };

      surface.configure(&device, &config);
      let shader_loader = RefCell::new(ShaderLoader::new(USE_SHADER_CACHE));
      let pipeline_loader = RefCell::new(PipelineLoader::new(USE_PIPELINE_CACHE));
      ( canvas, Self { device, queue, shader_loader, pipeline_loader }, surface, config )
   }

   #[allow(unused)]
   pub async fn new_offscreen() -> Self {
      let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
         backends: wgpu::Backends::GL,
         ..Default::default()
      });
      let adapter = instance
         .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::default(),
            compatible_surface: None,
            force_fallback_adapter: false,
         })
         .await
         .unwrap();
      let (device, queue) = adapter
         .request_device(&Utils::default_device_descriptor(), None)
         .await
         .unwrap();

      let shader_loader = RefCell::new(ShaderLoader::new(USE_SHADER_CACHE));
      let pipeline_loader = RefCell::new(PipelineLoader::new(USE_PIPELINE_CACHE));
      Self {device, queue, shader_loader, pipeline_loader}
   }

   pub fn get_vertex_shader(&self, variant: VertexShaderVariant, preprocessor: Option<&mut Preprocessor>) -> Rc<wgpu::ShaderModule> {
      self.shader_loader.borrow_mut().get_vertex_shader(&self.device, variant, preprocessor)
   }

   pub fn get_fragment_shader(&self, variant: FragmentShaderVariant, preprocessor: Option<&mut Preprocessor>) -> Rc<wgpu::ShaderModule> {
      self.shader_loader.borrow_mut().get_fragment_shader(&self.device, variant, preprocessor)
   }

   pub fn get_pipeline(&self, flat_descriptor: &RenderPipelineFlatDescriptor) -> Rc<wgpu::RenderPipeline> {
      self.pipeline_loader.borrow_mut().get_pipeline(&self.device, flat_descriptor)
   }
}