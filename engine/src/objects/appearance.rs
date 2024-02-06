use crate::error::textures::*;
use crate::prelude::*;
use parking_lot::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// Holds everything about the appearance of objects like
/// textures, vetex/index data, color and material.
#[derive(Debug, Clone, PartialEq)]
pub struct Appearance {
    visible: bool,
    transform: Transform,
    color: Color,

    instanced: bool,
    pub(crate) instance: Instance,
}
impl Eq for Appearance {}

use paste::paste;

// Just a macro that removes boilerplate getters and setters to be easily added with just one macro.
macro_rules! getters_and_setters {
    ($field:ident, $title:expr, $type:ty) => {
        #[doc=concat!("Sets ", $title, " of this appearance and returns self.")]
        #[inline]
        pub fn $field(mut self, $field: impl Into<$type>) -> Self {
            self.$field = $field.into();
            self
        }
        paste! {
            #[doc=concat!("Sets ", $title, " of this appearance.")]
            #[inline]
            pub fn [<set_ $field>](&mut self, $field: impl Into<$type>) {
                self.$field = $field.into();
            }
        }
        paste! {
            #[doc=concat!("Gets ", $title," of this appearance.")]
            #[inline]
            pub fn [<get_ $field>](&self) -> &$type {
                &self.$field
            }
        }
        paste! {
            #[doc=concat!("Gets a mutable reference to ", $title," of this appearance.")]
            #[inline]
            pub fn [<get_ $field _mut>](&mut self) -> &mut $type {
                &mut self.$field
            }
        }
    };
}

impl Appearance {
    /// Makes a default appearance.
    pub fn new() -> Self {
        Self {
            ..Default::default()
        }
    }
    /// Makes an instanced appearance allowing for better performance using the same appearance instance multiple times.
    pub fn new_instanced(model: Model, material: Option<Material>) -> Self {
        Self {
            instanced: true,
            instance: Instance {
                model,
                material,
                ..Default::default()
            },
            ..Default::default()
        }
    }

    /// Scales the object appearance according to the texture applied. Works best in Expand camera mode for best quality.
    pub fn auto_scale(&mut self) -> Result<(), TextureError> {
        let dimensions;
        if let Some(material) = &self.instance.material {
            dimensions = if let Some(texture) = &material.texture {
                texture.dimensions()
            } else {
                return Err(TextureError::NoTexture);
            };
        } else {
            return Err(TextureError::NoTexture);
        };

        self.transform.size = vec2(dimensions.0 as f32 * 0.001, dimensions.1 as f32 * 0.001);

        Ok(())
    }

    getters_and_setters!(visible, "the visibility", bool);
    getters_and_setters!(transform, "the transform", Transform);
    getters_and_setters!(color, "the color", Color);

    /// Returns the model of the appearance.
    pub fn get_model(&self) -> &Model {
        &self.instance.model
    }

    /// Returns the mutable instance of a model in case the appearance is not instanced.
    pub fn get_model_mut(&mut self) -> Option<&mut Model> {
        (!self.instanced).then_some(&mut self.instance.model)
    }

    /// Only sets the model if this appearance is not instanced.
    pub fn set_model(&mut self, model: Model) {
        (!self.instanced).then(|| self.instance.model = model);
    }

    /// Only sets the model if this appearance in not instanced.
    pub fn model(mut self, model: Model) -> Self {
        (!self.instanced).then(|| self.instance.model = model);
        self
    }

    /// Returns the material of the appearance.
    pub fn get_material(&self) -> Option<&Material> {
        self.instance.material.as_ref()
    }

    /// Returns the mutable instance of a material in case the appearance is not instanced.
    pub fn get_material_mut(&mut self) -> Option<Option<&mut Material>> {
        (!self.instanced).then_some(self.instance.material.as_mut())
    }

    /// Only sets the material if this appearance is not instanced.
    pub fn set_material(&mut self, material: Option<Material>) {
        (!self.instanced).then(|| self.instance.material = material);
    }

    /// Only sets the material if this appearance in not instanced.
    pub fn material(mut self, material: Option<Material>) -> Self {
        (!self.instanced).then(|| self.instance.material = material);
        self
    }

    /// Returns true if this object is instanced.
    pub fn is_instanced(&self) -> bool {
        self.instanced
    }

    /// Sets the layer of the texture in case it has a textured material with layers.
    pub fn set_layer(&mut self, id: u32) -> Result<(), TextureError> {
        self.instance
            .material
            .as_mut()
            .ok_or(TextureError::NoTexture)?
            .set_layer(id)
    }

    /// Returns the layer of the texture in case there is a material.
    pub fn layer(&self) -> Option<u32> {
        Some(self.instance.material.as_ref()?.layer)
    }

    /// Goes to the next frame of the textured material.
    ///
    /// Returns an error if it reached the limit.
    pub fn next_frame(&mut self) -> Result<(), TextureError> {
        self.instance
            .material
            .as_mut()
            .ok_or(TextureError::NoTexture)?
            .next_frame()
    }

    /// Goes back a frame of the textured material.
    ///
    /// Returns an error if the layer is already on 0.
    pub fn last_frame(&mut self) -> Result<(), TextureError> {
        self.instance
            .material
            .as_mut()
            .ok_or(TextureError::NoTexture)?
            .last_frame()
    }
}

impl Default for Appearance {
    fn default() -> Self {
        Self {
            visible: true,
            transform: Transform::default(),
            color: Color::WHITE,
            instanced: false,
            instance: Instance::default(),
        }
    }
}

/// An instance that can be reused many times without performance impact.
#[derive(Clone, Debug, Default)]
pub(crate) struct Instance {
    pub material: Option<Material>,
    pub model: Model,

    pub drawing: Arc<AtomicBool>,
    pub instance_data: Arc<Mutex<Vec<InstanceData>>>,
}

impl Instance {
    pub fn draw(&self, instances: &mut Vec<Self>) {
        if !self.drawing.load(Ordering::Acquire) {
            instances.push(self.clone());
            self.drawing.store(true, Ordering::Release);
        }
    }
    pub fn finish_drawing(&self) {
        self.drawing.store(false, Ordering::Release);
    }
}

impl PartialEq for Instance {
    fn eq(&self, other: &Self) -> bool {
        self.material == other.material && self.model == other.model
    }
}
