use anyhow::Result;
use parking_lot::{Mutex, RwLock};
use std::{
    sync::{atomic::AtomicBool, Arc, OnceLock},
    time::Duration,
};
use vulkano::{
    command_buffer::{
        CommandBuffer, CommandBufferBeginInfo, CommandBufferInheritanceInfo, CommandBufferLevel,
        CommandBufferUsage, RecordingCommandBuffer, RenderPassBeginInfo, SubpassBeginInfo,
        SubpassContents,
    },
    descriptor_set::{DescriptorSet, WriteDescriptorSet},
    pipeline::{graphics::viewport::Viewport, Pipeline},
    render_pass::Framebuffer,
    swapchain::{
        acquire_next_image, Surface, Swapchain, SwapchainAcquireFuture, SwapchainCreateInfo,
        SwapchainPresentInfo,
    },
    sync::{self, GpuFuture},
    Validated, VulkanError as VulkanoError,
};
use winit::event_loop::EventLoop;

use crate::{
    camera::CameraSettings,
    objects::{scenes::SCENE, Instance, Node, Object, VisualObject},
    resources::{
        data::{InstanceData, ModelViewProj, ObjectFrag},
        resources,
        vulkan::{
            swapchain::create_swapchain_and_images, window::create_window,
            window_size_dependent_setup,
        },
        Loader, Model,
    },
    utils::ortho_maker,
    window::{Window, WindowBuilder},
};

//use cgmath::{Deg, Matrix3, Matrix4, Ortho, Point3, Rad, Vector3};
use glam::{
    f32::{Mat4, Quat, Vec3},
    vec2,
};

pub static VIEWPORT: RwLock<Viewport> = RwLock::new(Viewport {
    offset: [0.0; 2],
    extent: [0.0; 2],
    depth_range: 0.0..=1.0,
});

/// Responsible for drawing on the surface.
pub struct Draw {
    pub surface: Arc<Surface>,
    pub window: Arc<Window>,
    pub swapchain: Arc<Swapchain>,
    pub framebuffers: Vec<Arc<Framebuffer>>,
    pub previous_frame_end: Option<Box<dyn GpuFuture>>,
    graphics: Arc<Graphics>,
    dimensions: [u32; 2],
}

impl Draw {
    pub fn setup(
        window_builder: WindowBuilder,
        event_loop: &EventLoop<()>,
        graphics: Arc<Graphics>,
    ) -> Result<Self> {
        let vulkan = resources()?.vulkan().clone();
        let loader = resources()?.loader().lock();
        let (surface, window) =
            create_window(event_loop, &resources()?.vulkan().instance, window_builder)?;

        let (swapchain, images) = create_swapchain_and_images(&vulkan.device, &surface, &graphics)?;

        let mut viewport = Viewport {
            offset: [0.0; 2],
            extent: [0.0; 2],
            depth_range: 0.0..=1.0,
        };

        let framebuffers =
            window_size_dependent_setup(&images, vulkan.render_pass.clone(), &mut viewport)?;

        *VIEWPORT.write() = viewport;

        let uploads = RecordingCommandBuffer::new(
            loader.command_buffer_allocator.clone(),
            vulkan.queue.queue_family_index(),
            CommandBufferLevel::Primary,
            CommandBufferBeginInfo {
                usage: CommandBufferUsage::OneTimeSubmit,
                ..Default::default()
            },
        )?;

        let previous_frame_end = Some(uploads.end()?.execute(vulkan.queue.clone())?.boxed());

        let dimensions = [0; 2];

        Ok(Self {
            surface,
            window,
            swapchain,
            framebuffers,
            previous_frame_end,
            graphics,
            dimensions,
        })
    }

    pub fn window(&self) -> &Arc<Window> {
        &self.window
    }

    /// Recreates the swapchain in case it is out of date if someone for example changed the scene size or window dimensions.
    fn recreate_swapchain(&mut self, loader: &mut Loader) -> Result<()> {
        if self
            .graphics
            .recreate_swapchain
            .load(std::sync::atomic::Ordering::Acquire)
        {
            let (new_swapchain, new_images) = match self.swapchain.recreate(SwapchainCreateInfo {
                image_extent: self.dimensions,
                present_mode: self.graphics.present_mode().into(),
                ..self.swapchain.create_info()
            }) {
                Ok(r) => r,
                Err(e) => {
                    return Err(e.into());
                }
            };

            self.swapchain = new_swapchain;
            self.framebuffers = window_size_dependent_setup(
                &new_images,
                resources()?.vulkan().render_pass.clone(),
                &mut VIEWPORT.write(),
            )
            .map_err(VulkanError::Other)?;
            loader.pipelines.clear();
            self.graphics
                .recreate_swapchain
                .store(false, std::sync::atomic::Ordering::Release);
        };
        Ok(())
    }

    /// Makes a primary and secondary command buffer already inside a render pass.
    fn make_command_buffer(
        &self,
        image_num: usize,
        clear_color: [f32; 4],
        loader: &Loader,
    ) -> Result<(RecordingCommandBuffer, RecordingCommandBuffer), VulkanError> {
        let vulkan = resources()
            .map_err(|e| VulkanError::Other(e.into()))?
            .vulkan()
            .clone();
        let mut builder = RecordingCommandBuffer::new(
            loader.command_buffer_allocator.clone(),
            vulkan.queue.queue_family_index(),
            CommandBufferLevel::Primary,
            CommandBufferBeginInfo {
                usage: CommandBufferUsage::OneTimeSubmit,
                ..Default::default()
            },
        )
        .map_err(Validated::unwrap)
        .map_err(VulkanError::Validated)?;

        // Makes a commandbuffer that takes multiple secondary buffers.
        builder
            .begin_render_pass(
                RenderPassBeginInfo {
                    clear_values: vec![Some(clear_color.into())],
                    ..RenderPassBeginInfo::framebuffer(self.framebuffers[image_num].clone())
                },
                SubpassBeginInfo {
                    contents: SubpassContents::SecondaryCommandBuffers,
                    ..Default::default()
                },
            )
            .map_err(|e| VulkanError::Other(e.into()))?;

        let mut secondary_builder = RecordingCommandBuffer::new(
            loader.command_buffer_allocator.clone(),
            vulkan.queue.queue_family_index(),
            CommandBufferLevel::Secondary,
            CommandBufferBeginInfo {
                usage: CommandBufferUsage::OneTimeSubmit,
                inheritance_info: Some(CommandBufferInheritanceInfo {
                    render_pass: Some(vulkan.subpass.clone().into()),
                    ..Default::default()
                }),
                ..Default::default()
            },
        )
        .map_err(Validated::unwrap)
        .map_err(VulkanError::Validated)?;
        secondary_builder
            .set_viewport(0, [VIEWPORT.read().clone()].into_iter().collect())
            .map_err(|e| VulkanError::Other(e.into()))?;

        Ok((builder, secondary_builder))
    }

    fn make_mvp_matrix(
        object: &VisualObject,
        dimensions: [u32; 2],
        camera: &Object,
        camera_settings: CameraSettings,
    ) -> (Mat4, Mat4, Mat4) {
        let transform = object.appearance.get_transform().combine(object.transform);
        let scaling = Vec3::new(transform.size[0], transform.size[1], 0.0);
        let rotation = Quat::from_rotation_z(transform.rotation);
        let translation = Vec3::new(transform.position[0], transform.position[1], 0.0);

        // Model matrix
        let model = Mat4::from_scale_rotation_translation(scaling, rotation, translation);

        // View matrix
        let rotation = Mat4::from_rotation_z(camera.transform.rotation);

        let zoom = 1.0 / camera_settings.zoom;

        // Projection matrix
        let proj = ortho_maker(
            camera_settings.mode,
            camera.transform.position,
            zoom,
            vec2(dimensions[0] as f32, dimensions[1] as f32),
        );

        let view = Mat4::look_at_rh(
            Vec3::from([
                camera.transform.position[0],
                camera.transform.position[1],
                1.0,
            ]),
            Vec3::from([
                camera.transform.position[0],
                camera.transform.position[1],
                0.0,
            ]),
            Vec3::Y,
        ) * rotation;
        (model, view, proj)
    }

    /// Draws the Game Scene on the given command buffer.
    fn write_secondary_command_buffer(
        &self,
        command_buffer: &mut RecordingCommandBuffer,
        loader: &mut Loader,
    ) -> Result<()> {
        for layer in SCENE.layers().iter() {
            let mut order: Vec<VisualObject> = Vec::with_capacity(layer.objects_map.lock().len());
            let mut instances: Vec<Instance> = vec![];

            Node::order_position(&mut order, &layer.root.lock());

            for object in order {
                let appearance = &object.appearance;

                let Some(model) = appearance.get_model() else {
                    continue;
                };

                let vulkan = resources()?.vulkan();
                let shapes = resources()?.shapes().clone();

                let model_data = match model {
                    Model::Custom(data) => data,
                    Model::Square => &shapes.square,
                    Model::Triangle => &shapes.triangle,
                };

                // Skip drawing the object if the object is not marked visible or has no vertices.
                if appearance.is_instanced() {
                    // appearance.instance.drawing.
                    appearance.instance.draw(&mut instances);
                    let mut data = appearance.instance.instance_data.lock();
                    let (model, view, proj) = Self::make_mvp_matrix(
                        &object,
                        self.dimensions,
                        &layer.camera.lock().lock().object,
                        layer.camera_settings(),
                    );
                    let instance_data = InstanceData {
                        model,
                        view,
                        proj,
                        color: (*appearance.get_color()).into(),
                        layer: appearance.layer().unwrap_or(0),
                    };
                    data.push(instance_data);
                    continue;
                };

                let mut descriptors = vec![];

                // The pipeline of the current object. Takes the default one if there is none.
                let pipeline = if let Some(material) = appearance.get_material() {
                    if let Some(texture) = material.texture() {
                        descriptors.push(texture.set().clone());
                    }
                    if let Some(descriptor) = &material.descriptor {
                        descriptors.push(descriptor.clone());
                    }
                    material
                        .get_pipeline_or_recreate(loader)
                        .map_err(VulkanError::Other)?
                } else {
                    vulkan
                        .default_material
                        .get_pipeline_or_recreate(loader)
                        .map_err(VulkanError::Other)?
                };

                // MVP matrix for the object
                let objectvert_sub_buffer = loader
                    .object_buffer_allocator
                    .allocate_sized()
                    .map_err(|error| VulkanError::Other(error.into()))?;
                // Simple color and texture data for the fragment shader.
                let objectfrag_sub_buffer = loader
                    .object_buffer_allocator
                    .allocate_sized()
                    .map_err(|error| VulkanError::Other(error.into()))?;

                let (model, view, proj) = Self::make_mvp_matrix(
                    &object,
                    self.dimensions,
                    &layer.camera.lock().lock().object,
                    layer.camera_settings(),
                );

                *objectvert_sub_buffer
                    .write()
                    .map_err(|error| VulkanError::Other(error.into()))? =
                    ModelViewProj { model, view, proj };
                *objectfrag_sub_buffer
                    .write()
                    .map_err(|error| VulkanError::Other(error.into()))? = ObjectFrag {
                    color: (*appearance.get_color()).into(),
                    texture_id: if let Some(material) = appearance.get_material() {
                        material.layer()
                    } else {
                        0
                    },
                };

                descriptors.insert(
                    0,
                    DescriptorSet::new(
                        loader.descriptor_set_allocator.clone(),
                        pipeline
                            .layout()
                            .set_layouts()
                            .first()
                            .ok_or(VulkanError::ShaderError)?
                            .clone(),
                        [
                            WriteDescriptorSet::buffer(0, objectvert_sub_buffer.clone()),
                            WriteDescriptorSet::buffer(1, objectfrag_sub_buffer.clone()),
                        ],
                        [],
                    )
                    .map_err(Validated::unwrap)
                    .map_err(VulkanError::Validated)?,
                );

                let command_buffer = command_buffer
                    .bind_pipeline_graphics(pipeline.clone())
                    .map_err(|e| VulkanError::Other(e.into()))?
                    .bind_descriptor_sets(
                        vulkano::pipeline::PipelineBindPoint::Graphics,
                        pipeline.layout().clone(),
                        0,
                        descriptors,
                    )
                    .map_err(|e| VulkanError::Other(e.into()))?
                    .bind_vertex_buffers(0, model_data.vertex_buffer())
                    .map_err(|e| VulkanError::Other(e.into()))?
                    .bind_index_buffer(model_data.index_buffer())
                    .map_err(|e| VulkanError::Other(e.into()))?;
                unsafe {
                    command_buffer
                        .draw_indexed(model_data.size() as u32, 1, 0, 0, 0)
                        .map_err(|e| VulkanError::Other(e.into()))?;
                }
            }
            for instance in instances {
                let Some(model) = instance.model.as_ref() else {
                    continue;
                };

                let mut data = instance.instance_data.lock();
                let instance_buffer = loader
                    .instance_buffer_allocator
                    .allocate_slice::<InstanceData>(data.len() as u64)
                    .map_err(|e| VulkanError::Other(e.into()))?;
                instance_buffer
                    .write()
                    .map_err(|e| VulkanError::Other(e.into()))?
                    .copy_from_slice(&data);

                let mut descriptors = vec![];
                let vulkan = resources()?.vulkan();

                // The pipeline of the current object. Takes the default one if there is none.
                let pipeline = if let Some(material) = &instance.material {
                    if let Some(texture) = material.texture() {
                        descriptors.push(texture.set().clone());
                    }
                    if let Some(descriptor) = &material.descriptor {
                        descriptors.push(descriptor.clone());
                    }
                    material
                        .get_pipeline_or_recreate(loader)
                        .map_err(VulkanError::Other)?
                } else {
                    vulkan
                        .default_instance_material
                        .get_pipeline_or_recreate(loader)
                        .map_err(VulkanError::Other)?
                };

                let shapes = resources()?.shapes().clone();
                let model = match &model {
                    Model::Custom(data) => data,
                    Model::Square => &shapes.square,
                    Model::Triangle => &shapes.triangle,
                };

                let command_buffer = command_buffer
                    .bind_pipeline_graphics(pipeline.clone())
                    .map_err(|e| VulkanError::Other(e.into()))?
                    .bind_descriptor_sets(
                        vulkano::pipeline::PipelineBindPoint::Graphics,
                        pipeline.layout().clone(),
                        0,
                        descriptors,
                    )
                    .map_err(|e| VulkanError::Other(e.into()))?
                    .bind_vertex_buffers(0, (model.vertex_buffer(), instance_buffer))
                    .map_err(|e| VulkanError::Other(e.into()))?
                    .bind_index_buffer(model.index_buffer())
                    .map_err(|e| VulkanError::Other(e.into()))?;
                unsafe {
                    command_buffer
                        .draw_indexed(model.size() as u32, data.len() as u32, 0, 0, 0)
                        .map_err(|e| VulkanError::Other(e.into()))?;
                }
                instance.finish_drawing();
                data.clear();
            }
        }
        Ok(())
    }

    /// Creates and executes a future in which the command buffer gets executed.
    fn execute_command_buffer(
        &mut self,
        command_buffer: Arc<CommandBuffer>,
        acquire_future: SwapchainAcquireFuture,
        image_num: u32,
    ) -> Result<()> {
        let vulkan = resources()?.vulkan().clone();
        let future = self
            .previous_frame_end
            .take()
            .ok_or(VulkanError::FlushFutureError(
                "Failed to obtain previous frame".to_string(),
            ))?
            .join(acquire_future)
            .then_execute(vulkan.queue.clone(), command_buffer)
            .map_err(|e| VulkanError::Other(e.into()))?
            .then_swapchain_present(
                vulkan.queue.clone(),
                SwapchainPresentInfo::swapchain_image_index(self.swapchain.clone(), image_num),
            )
            .then_signal_fence_and_flush();

        match future.map_err(Validated::unwrap) {
            Ok(future) => {
                self.previous_frame_end = Some(future.boxed());
            }
            Err(VulkanoError::OutOfDate) => {
                self.mark_swapchain_outdated();
                self.previous_frame_end = Some(sync::now(vulkan.device.clone()).boxed());
            }
            Err(e) => {
                self.previous_frame_end = Some(sync::now(vulkan.device.clone()).boxed());
                return Err(VulkanError::FlushFutureError(e.to_string()).into());
            }
        }
        Ok(())
    }

    pub fn mark_swapchain_outdated(&self) {
        self.graphics
            .recreate_swapchain
            .store(true, std::sync::atomic::Ordering::Release);
    }

    /// Redraws the scene.
    pub fn redraw_event(
        &mut self,
        #[cfg(feature = "egui")] gui: &mut egui_winit_vulkano::Gui,
    ) -> Result<(), VulkanError> {
        let mut loader = resources()
            .map_err(|e| VulkanError::Other(e.into()))?
            .loader()
            .lock();

        let dimensions = self.window.inner_size();
        self.dimensions = [dimensions.x as u32, dimensions.y as u32];

        self.previous_frame_end
            .as_mut()
            .ok_or(VulkanError::Other(anyhow::Error::msg(
                "Could not obtain previous frame end.",
            )))?
            .cleanup_finished();

        if self.dimensions.contains(&0) {
            return Ok(());
        }

        Self::recreate_swapchain(self, &mut loader).map_err(VulkanError::Other)?;

        let (image_num, suboptimal, acquire_future) =
            match acquire_next_image(self.swapchain.clone(), None).map_err(Validated::unwrap) {
                Ok(r) => r,
                Err(VulkanoError::OutOfDate) => {
                    self.mark_swapchain_outdated();
                    return Err(VulkanError::SwapchainOutOfDate);
                }
                Err(e) => {
                    return Err(VulkanError::Validated(e));
                }
            };

        if suboptimal {
            self.mark_swapchain_outdated();
        }

        let (mut builder, mut secondary_builder) = Self::make_command_buffer(
            self,
            image_num as usize,
            self.window.clear_color().rgba(),
            &loader,
        )?;

        Self::write_secondary_command_buffer(self, &mut secondary_builder, &mut loader)
            .map_err(VulkanError::Other)?;

        builder
            .execute_commands(secondary_builder.end()?)
            .map_err(|e| VulkanError::Other(e.into()))?;

        #[cfg(feature = "egui")]
        {
            // Creates and draws the second command buffer in case of egui.
            let cb = gui.draw_on_subpass_image(self.dimensions);
            builder
                .execute_commands(cb)
                .map_err(|e| VulkanError::Other(e.into()))?;
        }
        builder
            .end_render_pass(Default::default())
            .map_err(|e| VulkanError::Other(e.into()))?;
        let command_buffer = builder.end()?;

        Self::execute_command_buffer(self, command_buffer, acquire_future, image_num)
            .map_err(VulkanError::Other)?;
        Ok(())
    }
}

/// Engine wide Graphics settings.
///
/// By default the present mode is determined by this order based on availability on the device:
///
/// 1. `Mailbox`
/// 2. `Immediate`
/// 3. `Fifo`
///
/// The framerate limit is `None`, so off.
///
/// Only alter settings after the game engine has been initialized. The initialisation of the game engine also
/// initializes the settings.
pub struct Graphics {
    /// An option that determines something called "VSync".
    pub(crate) present_mode: Mutex<PresentMode>,
    /// Time waited before each frame.
    framerate_limit: Mutex<Duration>,
    pub(crate) available_present_modes: OnceLock<Vec<PresentMode>>,
    pub(crate) recreate_swapchain: AtomicBool,
}

impl Graphics {
    /// Creates a new graphics settings instance.
    pub fn new(present_mode: PresentMode) -> Self {
        Self {
            present_mode: Mutex::new(present_mode),
            framerate_limit: Mutex::new(Duration::from_secs(0)),
            available_present_modes: OnceLock::new(),
            recreate_swapchain: false.into(),
        }
    }

    /// Returns the present mode of the game.
    pub fn present_mode(&self) -> PresentMode {
        *self.present_mode.lock()
    }

    /// Sets and applies the present mode of the game.
    ///
    /// Returns an error in case the present mode given is not supported by the device.
    ///
    /// Find out which present modes work using the [get_supported_present_modes](Graphics::get_supported_present_modes) function.
    pub fn set_present_mode(&self, mode: PresentMode) -> anyhow::Result<()> {
        if self.get_supported_present_modes().contains(&mode) {
            *self.present_mode.lock() = mode;
            self.recreate_swapchain
                .store(true, std::sync::atomic::Ordering::Release);
            Ok(())
        } else {
            Err(anyhow::Error::msg(format!(
                "This present mode \"{:?}\" is not available on this device.\nAvailable modes on this device are {:?}",
                mode, self.get_supported_present_modes()
            )))
        }
    }

    /// Returns waiting time between frames to wait.
    pub fn framerate_limit(&self) -> Duration {
        *self.framerate_limit.lock()
    }

    /// Sets the framerate limit as waiting time between frames.
    ///
    /// This should be able to be changed by the user in case they have a device with limited power capacity like a laptop with a battery.
    ///
    /// Setting the duration to no wait time at all will turn off the limit.
    pub fn set_framerate_limit(&self, limit: Duration) {
        *self.framerate_limit.lock() = limit;
    }

    /// Sets the cap for the max frames per second the game should be able to output.
    ///
    /// This method is the same as setting the `set_framerate_limit` of this setting to `1.0 / cap` in seconds.
    ///
    /// Turns off the framerate cap if 0 was given.
    pub fn set_fps_cap(&self, cap: u64) {
        if cap == 0 {
            self.set_framerate_limit(Duration::from_secs(cap));
            return;
        }
        self.set_framerate_limit(Duration::from_secs_f64(1.0 / cap as f64));
    }

    /// Returns all the present modes this device supports.
    ///
    /// If the vec is empty the engine has not been initialized and the settings should not be changed at this state.
    pub fn get_supported_present_modes(&self) -> Vec<PresentMode> {
        self.available_present_modes
            .get()
            .cloned()
            .unwrap_or(vec![])
    }
}

/// The presentation action to take when presenting images to the window.
///
/// In game engine terms this affects "VSync".
///
/// `Immediate` mode is the only one that does not have "VSync".
///
/// When designing in game graphics settings this is the setting that gets changed when users select the VSync option.
///
/// The vsync options may include higher latency than the other ones.
///
/// It is not recommended dynamically switching between those during the game, as they may cause visual artifacts or noticable changes.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[non_exhaustive]
pub enum PresentMode {
    /// This one has no vsync and presents the image as soon as it is available.
    ///
    /// This may happen while the image is presenting, so it may cause tearing.
    ///
    /// This present mode has the lowest latency compared to every other mode, so this is the option for most fast paced games where latency matters.
    ///
    /// This present mode may not be available on every device.
    Immediate,
    /// This present mode has a waiting slot for the next image to be presented after the current one has finished presenting.
    /// This mode also does not block the drawing thread, drawing images, even when they will not get presented.
    ///
    /// This means there is no tearing and with just one waiting slot also not that much latency.
    ///
    /// This option is recommended if `Immediate` is not available and also for games that focus visual experience over latency, as this one does not have tearing.
    ///
    /// It may also not be available on every device.
    Mailbox,
    /// Means first in first out.
    ///
    /// This present mode is also known as "vsync on". It blocks the thread and only draws and presents images if the present buffer is finished drawing to the screen.
    ///
    /// It is guaranteed to be available on every device.
    Fifo,
}

impl From<PresentMode> for vulkano::swapchain::PresentMode {
    fn from(value: PresentMode) -> vulkano::swapchain::PresentMode {
        use vulkano::swapchain::PresentMode as Pm;
        match value {
            PresentMode::Immediate => Pm::Immediate,
            PresentMode::Mailbox => Pm::Mailbox,
            PresentMode::Fifo => Pm::Fifo,
        }
    }
}

impl From<vulkano::swapchain::PresentMode> for PresentMode {
    fn from(value: vulkano::swapchain::PresentMode) -> PresentMode {
        use vulkano::swapchain::PresentMode as Pm;
        match value {
            Pm::Immediate => PresentMode::Immediate,
            Pm::Mailbox => PresentMode::Mailbox,
            _ => PresentMode::Fifo,
        }
    }
}

// Redraw errors

use thiserror::Error;
use vulkano::shader::spirv::SpirvBytesNotMultipleOf4;

/// Errors that originate from Vulkan.
#[derive(Error, Debug)]
pub enum VulkanError {
    #[error("The swapchain is out of date and needs to be updated.")]
    SwapchainOutOfDate,
    #[error("Failed to flush future:\n{0}")]
    FlushFutureError(String),
    #[error("A Validated error:\n{0}")]
    Validated(VulkanoError),
    #[error("An unexpected error with the shaders occured.")]
    ShaderError,
    #[error("An unexpected error occured:\n{0}")]
    Other(anyhow::Error),
}

impl From<Validated<VulkanoError>> for VulkanError {
    fn from(value: Validated<VulkanoError>) -> Self {
        Self::Validated(value.unwrap())
    }
}

/// Errors that occur from the creation of Shaders.
#[derive(Error, Debug)]
pub enum ShaderError {
    #[error("The given entry point to those shaders is not present in the given shaders.")]
    ShaderEntryPoint,
    #[error("The provided bytes are not SpirV.")]
    InvalidSpirV,
    #[error("Something happened and the shader can not be made.: {0:?}")]
    Other(VulkanError),
}

impl From<Validated<VulkanoError>> for ShaderError {
    fn from(value: Validated<VulkanoError>) -> Self {
        Self::Other(value.into())
    }
}

impl From<SpirvBytesNotMultipleOf4> for ShaderError {
    fn from(_value: SpirvBytesNotMultipleOf4) -> Self {
        Self::InvalidSpirV
    }
}
