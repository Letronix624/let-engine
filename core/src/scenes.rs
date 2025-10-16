use std::ops::Range;

#[cfg(feature = "physics")]
use crate::objects::physics::{AddJointError, Physics, Shape, joints};
#[cfg(feature = "physics")]
use {rapier2d::parry::query::DefaultQueryDispatcher, slotmap::KeyData};

use crate::{
    backend::gpu::Loaded,
    camera::{CameraMode, CameraScaling},
    objects::{Color, Object, ObjectBuilder, ObjectId, Transform},
};
use glam::{Mat4, Vec2, Vec3Swizzles, vec2};
use slotmap::{SlotMap, new_key_type};

/// The whole scene seen with all it's layers.
pub struct Scene<T: Loaded = ()> {
    layers: SlotMap<LayerId, Layer>,
    root_layer_id: LayerId,

    layer_views: SlotMap<LayerViewId, LayerView<T>>,
    root_layer_view_id: LayerViewId,

    layer_tree_version: usize,

    objects: SlotMap<ObjectId, Object<T>>,

    #[cfg(feature = "physics")]
    dirty_objects: Vec<ObjectId>,
    #[cfg(feature = "physics")]
    physics_pipeline: crate::Mutex<PhysicsPipeline>,
}

impl<T: Loaded> Default for Scene<T> {
    fn default() -> Self {
        let mut layers = SlotMap::default();
        let mut layer_views = SlotMap::default();

        let root_layer_id = layers.insert_with_key(|id| Layer::new(id, None));
        let root_layer_view_id = layer_views.insert(LayerView::new(
            root_layer_id,
            DrawTarget::Window,
            Some(Color::BLACK),
        ));

        // Add root view to root layer.
        // No need to sort; This is the first insert
        layers[root_layer_id].views.push(root_layer_view_id);

        Self {
            layers,
            root_layer_id,
            layer_views,
            root_layer_view_id,
            layer_tree_version: 0,
            objects: SlotMap::default(),
            #[cfg(feature = "physics")]
            dirty_objects: Vec::new(),
            #[cfg(feature = "physics")]
            physics_pipeline: Default::default(),
        }
    }
}

impl<T: Loaded> Scene<T> {
    /// Returns the root layer of the scene.
    pub fn root_layer(&self) -> &Layer {
        // Root layer can not be removed, so it's safe to index
        &self.layers[self.root_layer_id]
    }

    pub fn root_layer_id(&self) -> LayerId {
        self.root_layer_id
    }

    pub fn root_layer_mut(&mut self) -> &mut Layer {
        // Root layer can not be removed, so it's safe to index
        &mut self.layers[self.root_layer_id]
    }

    /// Returns the only `LayerView` of the root layer.
    pub fn root_view(&self) -> &LayerView<T> {
        // There must always be a root view to render from
        &self.layer_views[self.root_layer_view_id]
    }

    pub fn root_view_id(&self) -> LayerViewId {
        self.root_layer_view_id
    }

    pub fn root_view_mut(&mut self) -> &mut LayerView<T> {
        // There must always be a root view to render from
        &mut self.layer_views[self.root_layer_view_id]
    }

    pub fn add_layer(&mut self, parent_id: LayerId) -> Option<LayerId> {
        let layer_id = self
            .layers
            .insert_with_key(|id| Layer::new(id, Some(parent_id)));

        let parent = self.layers.get_mut(parent_id)?;

        parent.layers.push(layer_id);

        self.layer_tree_version += 1;

        Some(layer_id)
    }

    pub fn layer(&self, id: LayerId) -> Option<&Layer> {
        self.layers.get(id)
    }

    pub fn layers_count(&self) -> usize {
        self.layers.len()
    }

    pub fn layer_mut(&mut self, id: LayerId) -> Option<&mut Layer> {
        self.layers.get_mut(id)
    }

    pub fn remove_layer(&mut self, id: LayerId) {
        let Some(layer) = self.layers.remove(id) else {
            return;
        };

        self.layer_tree_version += 1;

        // Remove layer from parent if there is one
        if let Some(parent_id) = layer.parent_id
            && let Some(parent) = self.layers.get_mut(parent_id)
            && let Ok(id) = parent.layers.binary_search(&id)
        {
            parent.layers.remove(id);
        };

        // recursively get all ids to be removed
        let mut layers: Vec<LayerId> = layer.layers.into_iter().collect();
        let mut layer_views: Vec<LayerViewId> = layer.views.into_iter().collect();
        let mut objects: Vec<ObjectId> = layer.objects.into_iter().collect();

        while let Some(id) = layers.pop() {
            let Some(layer) = self.layers.remove(id) else {
                continue;
            };
            layers.extend(layer.layers.into_iter());
            layer_views.extend(layer.views.into_iter());
            objects.extend(layer.objects.into_iter());
        }

        for view_id in layer_views {
            self.layer_views.remove(view_id);
        }

        for object_id in objects {
            self.remove_object(object_id);
        }
    }

    pub fn layer_tree_version(&self) -> usize {
        self.layer_tree_version
    }

    /// Returns a new viewpoint to this scene.
    ///
    /// Returns `None` in case the layer ID is invalid.
    ///
    /// You can not have multiple views of the root layer.
    ///
    /// # Arguments
    /// - `layer_id`: The ID of the layer in which this view views.
    /// - `camera`: The transform of the camera where size equals zoom.
    /// - `scaling`: The method of scaling the image to the aspect ratio.
    /// - `draw_target`: The target which the image gets drawn onto.
    /// - `clear_color`: If some, the color with which the image gets cleared;
    ///   if none, the image will not be cleared.
    pub fn add_view(
        &mut self,
        layer_id: LayerId,
        camera: Transform,
        scaling: CameraScaling,
        draw_target: DrawTarget<T>,
        clear_color: Option<Color>,
    ) -> Option<LayerViewId> {
        let layer = self.layers.get_mut(layer_id)?;

        let view = LayerView {
            transform: camera,
            scaling,
            ..LayerView::new(layer_id, draw_target, clear_color)
        };

        let key = self.layer_views.insert(view);

        if let Err(index) = layer.views.binary_search(&key) {
            layer.views.insert(index, key);
        }

        self.layer_tree_version += 1;

        Some(key)
    }

    pub fn views_iter(&self) -> slotmap::basic::Iter<'_, LayerViewId, LayerView<T>> {
        self.layer_views.iter()
    }

    pub fn views_count(&self) -> usize {
        self.layer_views.len()
    }

    pub fn view(&self, id: LayerViewId) -> Option<&LayerView<T>> {
        self.layer_views.get(id)
    }

    pub fn view_mut(&mut self, id: LayerViewId) -> Option<&mut LayerView<T>> {
        self.layer_views.get_mut(id)
    }

    pub fn remove_view(&mut self, view_id: LayerViewId) {
        let Some(view) = self.layer_views.remove(view_id) else {
            return;
        };

        let layer = self.layers.get_mut(view.layer_id()).unwrap();
        if let Ok(index) = layer.views.binary_search(&view_id) {
            layer.views.remove(index);
        }

        self.layer_tree_version += 1;
    }

    pub fn add_object(&mut self, layer_id: LayerId, builder: ObjectBuilder<T>) -> Option<ObjectId> {
        let layer = self.layers.get_mut(layer_id)?;

        let object = Object {
            transform: builder.transform,
            appearance: builder.appearance,
            children: Vec::new(),
            parent_id: None,
            layer_id,
            #[cfg(feature = "physics")]
            physics: builder.physics,
        };

        let object_id = self.objects.insert(object);

        #[cfg(feature = "physics")]
        self.dirty_objects.push(object_id);

        if let Err(index) = layer.objects.binary_search(&object_id) {
            layer.objects.insert(index, object_id);
        }

        Some(object_id)
    }

    pub fn add_object_with_parent(
        &mut self,
        parent_id: ObjectId,
        builder: ObjectBuilder<T>,
    ) -> Option<ObjectId> {
        let layer = {
            let parent_object = self.objects.get(parent_id)?;
            self.layers.get_mut(parent_object.layer_id)?
        };

        let object = Object {
            transform: builder.transform,
            appearance: builder.appearance,
            children: Vec::new(),
            parent_id: Some(parent_id),
            layer_id: layer.id,
            #[cfg(feature = "physics")]
            physics: builder.physics,
        };

        let object_id = self.objects.insert(object);

        #[cfg(feature = "physics")]
        self.dirty_objects.push(object_id);

        let parent = self.objects.get_mut(parent_id).unwrap();
        if let Err(index) = parent.children.binary_search(&object_id) {
            parent.children.insert(index, object_id);
        }
        if let Err(index) = layer.objects.binary_search(&object_id) {
            layer.objects.insert(index, object_id);
        }

        Some(object_id)
    }

    pub fn object(&self, id: ObjectId) -> Option<&Object<T>> {
        self.objects.get(id)
    }

    /// Returns the public transform, the transform of the object with all parents applied.
    pub fn object_public_transfrom(&self, id: ObjectId) -> Option<Transform> {
        let object = self.objects.get(id)?;
        let mut public_transform = object.transform;
        let mut parent_id = object.parent_id;
        while let Some(id) = parent_id {
            let parent = self
                .objects
                .get(id)
                .expect("The parent of an object should exist in the objects map");

            public_transform = parent.transform * public_transform;
            parent_id = parent.parent_id;
        }

        Some(public_transform)
    }

    pub fn object_mut(&mut self, id: ObjectId) -> Option<&mut Object<T>> {
        let object = self.objects.get_mut(id)?;
        #[cfg(feature = "physics")]
        self.dirty_objects.push(id);
        Some(object)
    }

    pub fn remove_object(&mut self, id: ObjectId) {
        let Some(object) = self.objects.remove(id) else {
            return;
        };

        let Some(layer) = self.layers.get_mut(object.layer_id) else {
            return;
        };
        if let Ok(index) = layer.objects.binary_search(&id) {
            layer.objects.remove(index);
        }
        #[cfg(feature = "physics")]
        object.physics.remove(&mut layer.physics);

        // Remove yourself from parent
        if let Some(parent_id) = object.parent_id
            && let Some(parent) = self.objects.get_mut(parent_id)
            && let Ok(index) = parent.children.binary_search(&id)
        {
            parent.children.remove(index);
        }

        // Create removal stack
        let mut objects: Vec<ObjectId> = object.children.into_iter().collect();
        // Remove all objects that descend from this object
        while let Some(id) = objects.pop() {
            let Some(object) = self.objects.remove(id) else {
                continue;
            };
            objects.extend(object.children.into_iter());
        }
    }

    /// Adds a joint between object 1 and 2.
    ///
    /// Objects must be from the same layer
    #[cfg(feature = "physics")]
    pub fn add_joint(
        &mut self,
        object1: ObjectId,
        object2: ObjectId,
        data: impl Into<joints::GenericJoint>,
        wake_up: bool,
    ) -> Result<ImpulseJointHandle, AddJointError> {
        let object1 = self.objects.get(object1).ok_or(AddJointError::NoObject)?;
        let object2 = self.objects.get(object2).ok_or(AddJointError::NoObject)?;

        if object1.layer_id != object2.layer_id {
            return Err(AddJointError::DifferentLayers);
        }

        let layer = self
            .layers
            .get_mut(object1.layer_id)
            .expect("Object should be in a valid layer");

        if let (Some(handle1), Some(handle2)) =
            (object1.rigidbody_handle(), object2.rigidbody_handle())
        {
            Ok(layer
                .physics
                .impulse_joint_set
                .insert(handle1, handle2, data.into().data, wake_up))
        } else {
            Err(AddJointError::NoRigidBody)
        }
    }

    /// Updates the scene physics and layers.
    #[cfg(feature = "physics")]
    pub fn physics_iteration(&mut self) -> anyhow::Result<()> {
        let mut pipeline = self.physics_pipeline.lock();

        // Update physics location of all updated objects
        while let Some(object_id) = self.dirty_objects.pop() {
            let Some(object) = self.objects.get(object_id) else {
                continue;
            };

            let public_transform = self.object_public_transfrom(object_id).unwrap();

            let Some(layer) = self.layers.get_mut(object.layer_id) else {
                continue;
            };

            let object = self.objects.get_mut(object_id).unwrap();

            object
                .physics
                .update(public_transform, object_id, &mut layer.physics);
        }

        for layer in self.layers.values_mut() {
            if layer.physics_enabled {
                layer.physics.step(&mut pipeline); // Rapier-side physics iteration run.
                for object_id in layer.objects.iter() {
                    let Some(object) = self.objects.get_mut(*object_id) else {
                        continue;
                    };
                    if let Some(handle) = object.rigidbody_handle() {
                        let rigid_body = layer.physics.rigid_body_set.get(handle).unwrap();
                        let pos = *rigid_body.translation();
                        let t = &mut object.transform;
                        t.position.x = pos.x;
                        t.position.y = pos.y;
                        t.rotation = glam::Quat::from_rotation_z(rigid_body.rotation().angle());
                    }
                }
            }
        }

        Ok(())
    }
}

impl<T: Loaded> std::ops::Index<LayerId> for Scene<T> {
    type Output = Layer;

    fn index(&self, index: LayerId) -> &Self::Output {
        self.layer(index)
            .unwrap_or_else(|| panic!("Invalid layer index."))
    }
}

impl<T: Loaded> std::ops::Index<LayerViewId> for Scene<T> {
    type Output = LayerView<T>;

    fn index(&self, index: LayerViewId) -> &Self::Output {
        self.view(index)
            .unwrap_or_else(|| panic!("Invalid view index."))
    }
}

impl<T: Loaded> std::ops::Index<ObjectId> for Scene<T> {
    type Output = Object<T>;

    fn index(&self, index: ObjectId) -> &Self::Output {
        self.object(index)
            .unwrap_or_else(|| panic!("Invalid object index."))
    }
}

impl<T: Loaded> std::ops::IndexMut<LayerId> for Scene<T> {
    fn index_mut(&mut self, index: LayerId) -> &mut Self::Output {
        self.layer_mut(index)
            .unwrap_or_else(|| panic!("Invalid layer index."))
    }
}

impl<T: Loaded> std::ops::IndexMut<LayerViewId> for Scene<T> {
    fn index_mut(&mut self, index: LayerViewId) -> &mut Self::Output {
        self.view_mut(index)
            .unwrap_or_else(|| panic!("Invalid view index."))
    }
}

impl<T: Loaded> std::ops::IndexMut<ObjectId> for Scene<T> {
    fn index_mut(&mut self, index: ObjectId) -> &mut Self::Output {
        self.object_mut(index)
            .unwrap_or_else(|| panic!("Invalid object index."))
    }
}

/// A layer struct holding it's own object hierarchy, camera and physics iteration.
#[derive(Debug)]
pub struct Layer {
    objects: Vec<ObjectId>,
    views: Vec<LayerViewId>,
    layers: Vec<LayerId>,
    id: LayerId,
    parent_id: Option<LayerId>,
    #[cfg(feature = "physics")]
    physics: Physics,
    #[cfg(feature = "physics")]
    physics_enabled: bool,
}

new_key_type! { pub struct LayerId; }

impl Layer {
    /// Creates a new layer.
    fn new(id: LayerId, parent_id: Option<LayerId>) -> Self {
        Self {
            objects: Vec::new(),
            views: Vec::new(),
            layers: Vec::new(),
            id,
            parent_id,

            #[cfg(feature = "physics")]
            physics: Physics::new(),
            #[cfg(feature = "physics")]
            physics_enabled: true,
        }
    }

    pub fn id(&self) -> LayerId {
        self.id
    }

    pub fn parent_id(&self) -> Option<LayerId> {
        self.parent_id
    }

    pub fn object_ids_iter(&self) -> impl Iterator<Item = ObjectId> {
        self.objects.iter().copied()
    }

    /// Checks if the layer contains this object.
    pub fn contains_object(&self, object_id: &ObjectId) -> bool {
        self.objects.contains(object_id)
    }

    /// Returns the number of objects in total initialized into this layer.
    pub fn objects_count(&self) -> usize {
        self.objects.len()
    }

    pub fn view_count(&self) -> usize {
        self.views.len()
    }

    pub fn layer_count(&self) -> usize {
        self.layers.len()
    }

    pub fn view_ids_iter(&self) -> impl Iterator<Item = LayerViewId> {
        self.views.iter().copied()
    }

    pub fn layer_ids_iter(&self) -> impl Iterator<Item = LayerId> {
        self.layers.iter().copied()
    }
}

#[cfg(feature = "physics")]
use rapier2d::prelude::*;

/// Physics
#[cfg_attr(docsrs, doc(cfg(feature = "physics")))]
#[cfg(feature = "physics")]
impl Layer {
    /// Returns the nearest collider id from a specific location.
    pub fn query_nearest_collider_at(&self, position: Vec2) -> Option<ObjectId> {
        let query_pipeline = self.physics.broad_phase.as_query_pipeline(
            &DefaultQueryDispatcher,
            &self.physics.rigid_body_set,
            &self.physics.collider_set,
            QueryFilter::new(),
        );

        // TODO: Allow user to set max distance
        query_pipeline
            .project_point(&position.into(), 1000., true)
            .map(|(handle, _)| {
                ObjectId::from(KeyData::from_ffi(
                    self.physics.collider_set.get(handle).unwrap().user_data as u64,
                ))
            })
    }

    /// Returns id of the first collider intersecting with given ray.
    pub fn cast_ray(
        &self,
        position: Vec2,
        direction: Vec2,
        time_of_impact: Real,
        solid: bool,
    ) -> Option<ObjectId> {
        let query_pipeline = self.physics.broad_phase.as_query_pipeline(
            &DefaultQueryDispatcher,
            &self.physics.rigid_body_set,
            &self.physics.collider_set,
            QueryFilter::new(),
        );

        query_pipeline
            .cast_ray(
                &Ray::new(position.into(), direction.into()),
                time_of_impact,
                solid,
            )
            .map(|(handle, _)| {
                ObjectId::from(KeyData::from_ffi(
                    self.physics.collider_set.get(handle).unwrap().user_data as u64,
                ))
            })
    }
    /// Returns id of the first collider intersecting with given ray and returns a normal.
    pub fn cast_ray_and_get_normal(
        &self,
        position: Vec2,
        direction: Vec2,
        time_of_impact: Real,
        solid: bool,
    ) -> Option<(ObjectId, Vec2)> {
        let query_pipeline = self.physics.broad_phase.as_query_pipeline(
            &DefaultQueryDispatcher,
            &self.physics.rigid_body_set,
            &self.physics.collider_set,
            QueryFilter::new(),
        );

        query_pipeline
            .cast_ray_and_get_normal(
                &Ray::new(position.into(), direction.into()),
                time_of_impact,
                solid,
            )
            .map(|(handle, intersection)| {
                let inter = intersection.normal;
                (
                    ObjectId::from(KeyData::from_ffi(
                        self.physics.collider_set.get(handle).unwrap().user_data as u64,
                    )),
                    vec2(inter.x, inter.y),
                )
            })
    }

    /// Returns id of the first collider intersecting with given ray.
    pub fn intersections_with_ray(
        &self,
        position: Vec2,
        direction: Vec2,
        time_of_impact: Real,
        solid: bool,
    ) -> Vec<ObjectId> {
        let bodies = &self.physics.rigid_body_set;
        let colliders = &self.physics.collider_set;

        let query_pipeline = self.physics.broad_phase.as_query_pipeline(
            &DefaultQueryDispatcher,
            bodies,
            colliders,
            QueryFilter::new(),
        );

        if direction.eq(&vec2(0.0, 0.0)) {
            query_pipeline
                .intersect_point(position.into())
                .map(|x| ObjectId::from(KeyData::from_ffi(x.1.user_data as u64)))
                .collect()
        } else {
            query_pipeline
                .intersect_ray(
                    Ray::new(position.into(), direction.into()),
                    time_of_impact,
                    solid,
                )
                .map(|x| ObjectId::from(KeyData::from_ffi(x.1.user_data as u64)))
                .collect()
        }
    }

    /// Cast a shape and return the first collider intersecting with it.
    pub fn intersections_with_shape(&self, shape: Shape, position: (Vec2, f32)) -> Vec<ObjectId> {
        let query_pipeline = self.physics.broad_phase.as_query_pipeline(
            &DefaultQueryDispatcher,
            &self.physics.rigid_body_set,
            &self.physics.collider_set,
            QueryFilter::new(),
        );

        let iso = nalgebra::Isometry2::new(position.0.into(), position.1);
        query_pipeline
            .intersect_shape(iso, shape.0.as_ref())
            .map(|(_, collider)| ObjectId::from(KeyData::from_ffi(collider.user_data as u64)))
            .collect()
    }

    /// Gets the gravity parameter.
    pub fn gravity(&self) -> Vec2 {
        let vec = self.physics.gravity;

        vec2(vec.x, vec.y)
    }

    /// Sets the gravity parameter.
    pub fn set_gravity(&mut self, gravity: Vec2) {
        self.physics.gravity = gravity.into();
    }

    /// Returns if physics is enabled.
    pub fn physics_enabled(&self) -> bool {
        self.physics_enabled
    }

    /// Set physics to be enabled or disabled.
    pub fn set_physics_enabled(&mut self, enabled: bool) {
        self.physics_enabled = enabled;
    }

    /// Takes the physics simulation parameters.
    pub fn physics_parameters(&self) -> IntegrationParameters {
        self.physics.integration_parameters
    }

    /// Sets the physics simulation parameters.
    pub fn set_physics_parameters(&mut self, parameters: IntegrationParameters) {
        self.physics.integration_parameters = parameters;
    }

    /// Returns if the joint exists.
    pub fn joint(&self, handle: ImpulseJointHandle) -> Option<joints::GenericJoint> {
        self.physics
            .impulse_joint_set
            .get(handle)
            .map(|joint| joints::GenericJoint { data: joint.data })
    }

    /// Updates a joint.
    pub fn set_joint(
        &mut self,
        data: impl Into<joints::GenericJoint>,
        handle: ImpulseJointHandle,
    ) -> Result<(), joints::NoJointError> {
        if let Some(joint) = self.physics.impulse_joint_set.get_mut(handle, true) {
            joint.data = data.into().data;
            Ok(())
        } else {
            Err(joints::NoJointError)
        }
    }

    /// Removes a joint.
    pub fn remove_joint(&mut self, handle: ImpulseJointHandle, wake_up: bool) {
        self.physics.impulse_joint_set.remove(handle, wake_up);
    }
}

/// `LayerView` represents a view or camera into a specific `Layer` in the game engine's hierarchical
/// layer system. A `LayerView` is used to render a particular `Layer` as a texture or directly
/// to the screen in the case of the root layer.
///
/// To delete a LayerView, drop the last reference to it.
///
/// The [`Transform`] acts as a camera, where `size` determines the zoom in both axis.
///
/// The extent of this view is a screen or texture space UV rectangle.
pub struct LayerView<T: Loaded> {
    parent_id: LayerId,
    pub draw: bool,
    draw_target: DrawTarget<T>,
    clear_color: Option<Color>,

    pub transform: Transform,

    /// The extent of the viewport.
    ///
    /// A rect determining the location on the target, where to put the viewport in UV coordinates.
    pub extent: [Vec2; 2],

    /// The range determining the depth view distances.
    pub depth: Range<f32>,

    pub mode: CameraMode,

    /// The scissor of the viewport.
    ///
    /// A UV rect that is visible in the viewport. Everything outside this rect is discarded.
    pub scissor: [Vec2; 2],
    pub scaling: CameraScaling,
}

new_key_type! { pub struct LayerViewId; }

impl<T: Loaded> LayerView<T> {
    fn new(parent_id: LayerId, draw_target: DrawTarget<T>, clear_color: Option<Color>) -> Self {
        Self {
            parent_id,
            draw: true,
            draw_target,
            clear_color,
            transform: Transform::default(),
            extent: [Vec2::ZERO, Vec2::ONE],
            depth: 0.0..1.0,
            mode: CameraMode::default(),
            scissor: [Vec2::ZERO, Vec2::ONE],
            scaling: CameraScaling::default(),
        }
    }

    /// Returns the matrix of this view.
    #[inline]
    pub fn view_matrix(&self) -> Mat4 {
        self.transform.to_matrix().inverse()
    }

    /// Creates a projection matrix for the view.
    pub fn projection_matrix(&self, resolution: Vec2) -> Mat4 {
        let scaled = self.scaling.scale(resolution);
        match self.mode {
            CameraMode::Orthographic => Mat4::orthographic_rh(
                -scaled.x,
                scaled.x,
                -scaled.y,
                scaled.y,
                self.depth.start,
                -self.depth.end,
            ),
            CameraMode::Perspective { fov_y_rad } => Mat4::perspective_rh(
                fov_y_rad,
                scaled.max_element() / scaled.min_element(),
                self.depth.start,
                self.depth.end,
            ),
            CameraMode::Frustum => Mat4::frustum_rh(
                -scaled.x,
                scaled.x,
                -scaled.y,
                scaled.y,
                self.depth.start,
                self.depth.end,
            ),
        }
    }

    /// Returns the parent layer of this view.
    pub fn layer_id(&self) -> LayerId {
        self.parent_id
    }

    pub fn draw_target(&self) -> &DrawTarget<T> {
        &self.draw_target
    }

    pub fn clear_color(&self) -> Option<Color> {
        self.clear_color
    }

    /// Sets the clear color of this view in case it has been created with a clear color.
    pub fn set_clear_color(&mut self, color: Color) {
        if let Some(clear_color) = self.clear_color.as_mut() {
            *clear_color = color;
        }
    }

    /// Returns the position of a given screen space with given window dimensions to world space.
    ///
    /// This function takes coordinates, where (-1, -1) represents the top left corner of the screen
    /// and (1, 1) the bottom right.
    pub fn screen_to_world(&self, direction: Vec2, resolution: Vec2) -> Vec2 {
        let min = vec2(
            self.extent[0].x.min(self.extent[1].x),
            self.extent[0].y.min(self.extent[1].y),
        );
        let max = vec2(
            self.extent[0].x.max(self.extent[1].x),
            self.extent[0].y.max(self.extent[1].y),
        );

        let offset = direction + 1.0 - max - min;
        let extent = self.scaling.scale(1.0 / (max - min) * resolution) * self.transform.size.xy();
        let camera_offset = self.transform.position.xy();

        offset * extent + camera_offset
    }
}

#[derive(Clone, Copy)]
pub enum DrawTarget<T: Loaded> {
    Window,
    Texture(T::TextureId),
}
