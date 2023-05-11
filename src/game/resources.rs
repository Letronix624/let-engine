use super::{materials, AObject, Font, Labelifier, Vulkan};
use crate::{error::textures::*, texture::*};
use image::{load_from_memory_with_format, DynamicImage, ImageFormat as IFormat};
use parking_lot::Mutex;
use std::sync::Arc;
use vulkano::buffer::BufferContents;
use vulkano::descriptor_set::persistent::PersistentDescriptorSet;
use vulkano::descriptor_set::WriteDescriptorSet;
use winit::window::Window;

mod loader;
pub use loader::Loader;

#[derive(Clone)]
pub struct Resources {
    pub vulkan: Vulkan,
    pub loader: Arc<Mutex<Loader>>,
    labelifier: Arc<Mutex<Labelifier>>,
}

impl Resources {
    //initialisation
    pub fn new(
        vulkan: Vulkan,
        loader: Arc<Mutex<Loader>>,
        labelifier: Arc<Mutex<Labelifier>>,
    ) -> Self {
        Self {
            vulkan,
            loader,
            labelifier,
        }
    }
    //redraw
    pub fn update(&self) {
        // swap with layers
        let mut loader = self.loader.lock();
        let mut labelifier = self.labelifier.lock();
        labelifier.update(&self.vulkan, &mut loader);
    }

    //loading
    pub fn load_font(&self, font: &[u8]) -> Arc<GameFont> {
        let mut labelifier = self.labelifier.lock();
        labelifier.load_font(font)
    }

    pub fn load_texture_from_raw(
        &self,
        texture: Vec<u8>,
        format: Format,
        dimensions: (u32, u32),
        layers: u32,
        settings: TextureSettings,
    ) -> Arc<Texture> {
        let mut loader = self.loader.lock();
        Arc::new(Texture {
            data: texture.clone(),
            dimensions,
            layers,
            set: loader.load_texture(&self.vulkan, texture, dimensions, layers, format, settings),
        })
    }

    pub fn load_texture(
        &self,
        texture: &[u8],
        format: ImageFormat,
        layers: u32,
        settings: TextureSettings,
    ) -> Result<Arc<Texture>, InvalidFormatError> {
        let image_format = match format {
            ImageFormat::PNG => IFormat::Png,
            ImageFormat::JPG => IFormat::Jpeg,
            ImageFormat::BMP => IFormat::Bmp,
            ImageFormat::TIFF => IFormat::Tiff,
            ImageFormat::WebP => IFormat::WebP,
            ImageFormat::TGA => IFormat::Tga,
        };
        let image = match load_from_memory_with_format(texture, image_format) {
            Err(_) => return Err(InvalidFormatError),
            Ok(v) => v,
        };

        let mut dimensions: (u32, u32);

        let mut format = Format::RGBA8;

        let image: Vec<u8> = match image {
            DynamicImage::ImageLuma8(image) => {
                format = Format::R8;
                dimensions = image.dimensions();
                image.into_vec()
            }
            DynamicImage::ImageLumaA8(_) => {
                let image = image.to_rgba8();
                dimensions = image.dimensions();
                image.into_vec()
            }
            DynamicImage::ImageLuma16(_) => {
                let image = image.to_luma8();
                dimensions = image.dimensions();
                format = Format::R8;
                image.into_vec()
            }
            DynamicImage::ImageLumaA16(_) => {
                let image = image.to_rgba16();
                dimensions = image.dimensions();
                format = Format::RGBA16;
                u16tou8vec(image.into_vec())
            }
            DynamicImage::ImageRgb8(_) => {
                let image = image.to_rgba8();
                dimensions = image.dimensions();
                image.into_vec()
            }
            DynamicImage::ImageRgba8(image) => {
                dimensions = image.dimensions();
                image.into_vec()
            }
            DynamicImage::ImageRgb16(_) => {
                let image = image.to_rgba16();
                dimensions = image.dimensions();
                format = Format::RGBA16;
                u16tou8vec(image.into_vec())
            }
            DynamicImage::ImageRgba16(image) => {
                format = Format::RGBA16;
                dimensions = image.dimensions();
                u16tou8vec(image.into_vec())
            }
            DynamicImage::ImageRgb32F(_) => {
                let image = image.to_rgba16();
                dimensions = image.dimensions();
                format = Format::RGBA16;
                u16tou8vec(image.into_vec())
            }
            DynamicImage::ImageRgba32F(_) => {
                let image = image.to_rgba16();
                dimensions = image.dimensions();
                format = Format::RGBA16;
                u16tou8vec(image.into_vec())
            }
            _ => {
                let image = image.to_rgba8();
                dimensions = image.dimensions();
                image.into_vec()
            }
        };

        dimensions.1 /= layers;

        Ok(Self::load_texture_from_raw(
            self, image, format, dimensions, layers, settings,
        ))
    }
    //
    pub fn queue_to_label(
        &self,
        object: &AObject,
        font: &Arc<Font>,
        text: &str,
        scale: f32,
        align: [f32; 2],
    ) {
        let mut labelifier = self.labelifier.lock();
        labelifier.queue(object.clone(), font, text.to_string(), scale, align);
    }
    //shaders
    /// # Safety
    ///
    /// Any wrong shaders bytes can mess up the program in a way I didn't text before.
    pub unsafe fn new_shader_from_raw(
        // loading things all temporary. Will get sepparated to their own things soon.
        &self,
        vertex_bytes: &[u8],
        fragment_bytes: &[u8],
    ) -> materials::Shaders {
        unsafe { materials::Shaders::from_bytes(vertex_bytes, fragment_bytes, &self.vulkan) }
    }
    // fn new_shader ..requires the vulkano_shaders library function load() device
    pub fn new_descriptor_write<T: BufferContents>(&self, buf: T, set: u32) -> WriteDescriptorSet {
        let loader = self.loader.lock();
        loader.write_descriptor(buf, set)
    }
    pub fn new_material(
        &self,
        settings: materials::MaterialSettings,
        descriptor_bindings: Vec<WriteDescriptorSet>,
    ) -> materials::Material {
        let mut loader = self.loader.lock();
        loader.load_material(&self.vulkan, settings, descriptor_bindings)
    }
    /// Simplification of making a texture and putting it into a material.
    pub fn new_material_from_texture(
        &self,
        texture: &[u8],
        format: ImageFormat,
        layers: u32,
        settings: TextureSettings,
    ) -> Result<materials::Material, InvalidFormatError> {
        let texture = Self::load_texture(self, texture, format, layers, settings)?;

        Ok(Self::default_textured_material(self, &texture))
    }
    /// Simplification of making a texture and putting it into a material.
    pub fn new_material_from_raw_texture(
        &self,
        texture: Vec<u8>,
        format: Format,
        dimensions: (u32, u32),
        layers: u32,
        settings: TextureSettings,
    ) -> materials::Material {
        let texture =
            Self::load_texture_from_raw(self, texture, format, dimensions, layers, settings);
        Self::default_textured_material(self, &texture)
    }
    pub fn default_textured_material(&self, texture: &Arc<Texture>) -> materials::Material {
        let default = if texture.layers == 1 {
            self.vulkan.textured_material.clone()
        } else {
            self.vulkan.texture_array_material.clone()
        };
        materials::Material {
            texture: Some(Arc::clone(texture)),
            ..default
        }
    }

    pub fn get_window(&self) -> &Window {
        self.vulkan
            .surface
            .object()
            .unwrap()
            .downcast_ref::<Window>()
            .unwrap()
    }

    pub fn window_dimensions(&self) -> (u32, u32) {
        let dim = Self::get_window(self).inner_size();
        (dim.width, dim.height)
    }
}

#[derive(Clone)]
pub struct Texture {
    pub data: Vec<u8>,
    pub dimensions: (u32, u32),
    pub layers: u32,
    pub set: Arc<PersistentDescriptorSet>,
}

pub struct GameFont {
    pub font: rusttype::Font<'static>,
    pub fontid: usize,
}

pub struct Sound {
    pub data: Vec<u8>,
}

impl PartialEq for Texture {
    fn eq(&self, other: &Self) -> bool {
        self.data == other.data
            && self.dimensions == other.dimensions
            && Arc::ptr_eq(&self.set, &other.set)
    }
}

impl std::fmt::Debug for Texture {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Texture")
            .field("size", &self.data.len())
            .field("dimensions", &self.dimensions)
            .field("frames", &self.layers)
            .finish()
    }
}

/// Not done.
#[allow(dead_code)]
pub fn load_sound(sound: &[u8]) -> Arc<Sound> {
    Arc::new(Sound {
        data: sound.to_vec(),
    })
}

fn u16tou8vec(data: Vec<u16>) -> Vec<u8> {
    data.iter()
        .flat_map(|&u16_value| {
            let high_byte = ((u16_value >> 8) & 0xff) as u8;
            let low_byte = (u16_value & 0xff) as u8;
            vec![high_byte, low_byte]
        })
        .collect()
}
