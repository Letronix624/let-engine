use std::collections::{BTreeMap, BTreeSet};

use crate::{
    backend::graphics::Loaded,
    resources::{buffer::Location, data::Data, model::Vertex},
};

/// Builder struct to the [`Appearance`] struct.
///
/// Fields `material` and `model` must be initialized for an appearance to be able to be built.
#[derive(Clone)]
pub struct AppearanceBuilder<T: Loaded> {
    /// Initial transform
    pub transform: Transform,

    /// The initial material of the appearance.
    ///
    /// This field must be initialized for the build to succeed.
    pub material: Option<T::Material>,

    /// The initial model of the appearance.
    ///
    /// This field must be initialized for the build to succeed.
    pub model: Option<T::DrawableModel>,

    /// The buffers and textures and their location in the shaders.
    pub descriptors: BTreeMap<Location, Descriptor<T>>,

    /// Initial visibility of the appearance.
    pub visible: bool,
}

impl<T: Loaded> Default for AppearanceBuilder<T> {
    fn default() -> Self {
        Self {
            transform: Transform::default(),
            material: None,
            model: None,
            descriptors: BTreeMap::new(),
            visible: true,
        }
    }
}

impl<T: Loaded> AppearanceBuilder<T> {
    /// Sets if this object is visible and returns self.
    pub fn visible(mut self, visible: bool) -> Self {
        self.visible = visible;
        self
    }

    /// Sets the transformation and returns self.
    pub fn transform(mut self, transform: Transform) -> Self {
        self.transform = transform;
        self
    }

    /// Sets the material and returns self.
    ///
    /// This field must be initialized for the build to succeed.
    pub fn material(mut self, material: T::Material) -> Self {
        self.material = Some(material);
        self
    }

    /// Sets the model and returns self.
    ///
    /// This field must be initialized for the build to succeed.
    pub fn model<V: Vertex>(mut self, model: T::Model<V>) -> Self {
        self.model = Some(T::draw_model(model));
        self
    }

    /// Inserts all buffer locations and values.
    ///
    /// This field must be set and has to match up with the format of the shaders provided by
    /// the material before building.
    #[allow(clippy::type_complexity)]
    pub fn descriptors(mut self, descriptors: &[(Location, Descriptor<T>)]) -> Self {
        self.descriptors = descriptors.iter().cloned().collect();
        self
    }

    /// Builds this struct into an `Appearance`.
    pub fn build(self) -> Result<Appearance<T>, AppearanceBuilderError<T>> {
        let Some(material) = self.material else {
            return Err(AppearanceBuilderError::Uninitialized);
        };

        let Some(model) = self.model else {
            return Err(AppearanceBuilderError::Uninitialized);
        };

        // Validate skipping sets
        {
            let mut existing_sets: BTreeSet<u32> = BTreeSet::new();

            // Collect all unique set indices
            for location in self.descriptors.keys() {
                existing_sets.insert(location.set);
            }

            let mut missing_sets = Vec::new();
            let mut expected_set = 0;

            for &set in &existing_sets {
                while expected_set < set {
                    missing_sets.push(expected_set);
                    expected_set += 1;
                }
                expected_set = set + 1;
            }

            if !missing_sets.is_empty() {
                return Err(AppearanceBuilderError::MissingSets(missing_sets));
            }
        }

        // Validate backend
        T::initialize_appearance(&material, &model, &self.descriptors)
            .map_err(AppearanceBuilderError::<T>::InvalidCombination)?;

        Ok(Appearance {
            visible: self.visible,
            transform: self.transform,
            material,
            model,
            descriptors: self.descriptors,
        })
    }
}

/// Errors returned when building an `AppearanceBuilder` into an `Appearance`.
#[derive(Error, Debug, Clone)]
pub enum AppearanceBuilderError<T: Loaded> {
    /// Gets returned when trying to build an `AppearanceBuilder` with the field `material` or `model` uninitialized.
    #[error("Failed to build Appearance: Uninitialized fields.")]
    Uninitialized,

    /// Gets returned when the combination of material, model and buffers is invalid for the graphics backend.
    #[error("Failed to build Appearance: {0}")]
    InvalidCombination(T::AppearanceCreationError),

    /// Gets returned when descriptor set indices are not present in a chromatically ascending manner.
    #[error("Missing descriptor set")]
    MissingSets(Vec<u32>),
}

/// Holds everything about the appearance of objects like
/// textures, vetex/index data and material.
pub struct Appearance<T: Loaded> {
    descriptors: BTreeMap<Location, Descriptor<T>>,
    transform: Transform,

    material: T::Material,
    model: T::DrawableModel,
    visible: bool,
    // instance: Option<T::Buffer>,
}

impl Default for Appearance<()> {
    fn default() -> Self {
        Self {
            descriptors: BTreeMap::new(),
            transform: Transform::default(),
            material: (),
            model: (),
            visible: false,
        }
    }
}

impl<T: Loaded> Clone for Appearance<T> {
    fn clone(&self) -> Self {
        Self {
            visible: self.visible,
            transform: self.transform,
            material: self.material.clone(),
            model: self.model.clone(),
            // instance: self.instance.clone(),
            descriptors: self.descriptors.clone(),
        }
    }
}

/// Types a descriptor can be in the Appearance.
#[derive(Debug, PartialEq)]
pub enum Descriptor<T: Loaded> {
    Texture(T::Texture),
    Buffer(T::DrawableBuffer),
    Mvp,
}

impl<T: Loaded> Descriptor<T> {
    pub fn buffer<B: Data>(buffer: T::Buffer<B>) -> Self {
        Self::Buffer(T::draw_buffer(buffer))
    }
}

impl<T: Loaded> Clone for Descriptor<T> {
    fn clone(&self) -> Self {
        match self {
            Self::Texture(texture) => Self::Texture(texture.clone()),
            Self::Buffer(buffer) => Self::Buffer(buffer.clone()),
            Self::Mvp => Self::Mvp,
        }
    }
}

use anyhow::Result;
use paste::paste;
use thiserror::Error;

use super::Transform;

/// Just a macro that removes boilerplate getters and setters to be easily added with just one macro.
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

impl<T: Loaded> Appearance<T> {
    // /// Creates a new default Appearance with the given Material and Model.
    // pub fn new(material: T::Material, model: T::Model) -> Self {
    //     Self {
    //         visible: true,
    //         transform: Transform::default(),
    //         mvp_config: MvpConfig::default(),
    //         material,
    //         model,
    //         // instance: None,
    //         buffers: HashMap::default(),
    //     }
    // }

    // pub fn new_instanced(
    //     material: T::Material,
    //     model: T::Model,
    //     instance: Option<T::Buffer>,
    // ) -> Self {
    //     Self {
    //         visible: true,
    //         transform: Transform::default(),
    //         mvp_config: MVPConfig::default(),
    //         material,
    //         model,
    //         instance,
    //         buffers: HashMap::default(),
    //     }
    // }

    getters_and_setters!(visible, "the visibility", bool);
    getters_and_setters!(transform, "the transform", Transform);
    getters_and_setters!(model, "the model", T::DrawableModel);
    getters_and_setters!(material, "the material", T::Material);

    /// Returns a reference to the HashMap of descriptors in this appearance.
    #[inline]
    pub fn descriptors(&self) -> &BTreeMap<Location, Descriptor<T>> {
        &self.descriptors
    }

    /// Returns a reference to a descriptor at specified location in case it exists.
    #[inline]
    pub fn get_descriptor(&self, location: &Location) -> Option<&Descriptor<T>> {
        self.descriptors.get(location)
    }

    /// Returns a mutable reference to a descriptor at specified location in case it exists.
    #[inline]
    pub fn get_descriptor_mut(&mut self, location: &Location) -> Option<&mut Descriptor<T>> {
        self.descriptors.get_mut(location)
    }

    // /// Returns true if this object is instanced.
    // pub fn is_instanced(&self) -> bool {
    //     self.instance.is_some()
    // }
}
