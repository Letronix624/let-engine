use super::*;
use crate::{backend::graphics::Loaded, camera::*};
use anyhow::Result;
use crossbeam::atomic::AtomicCell;
use glam::UVec2;
use indexmap::IndexSet;

use crate::{HashMap, Mutex};
use std::sync::{
    atomic::{AtomicBool, AtomicU64, Ordering},
    Arc,
};

/// The whole scene seen with all it's layers.
pub struct Scene<T: Loaded> {
    root_layer: Arc<Layer<T>>,
    ordered_views: Mutex<Vec<Arc<LayerView<T>>>>,
    root_view: Arc<LayerView<T>>,
    #[cfg(feature = "physics")]
    physics_pipeline: Mutex<PhysicsPipeline>,
}

impl<T: Loaded> std::fmt::Debug for Scene<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // TODO: Debug actual scene
        write!(f, "Root layer")?;
        write!(f, "Views")?;
        write!(f, "Root View")?;
        write!(f, "Physics Pipeline")
    }
}

impl<T: Loaded> Default for Scene<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Loaded> Scene<T> {
    pub fn new() -> Self {
        let (root_layer, root_view) = Layer::new_root();
        Self {
            root_layer,
            ordered_views: Mutex::new(vec![root_view.clone()]),
            root_view,
            #[cfg(feature = "physics")]
            physics_pipeline: Default::default(),
        }
    }

    /// Returns the root layer of the scene.
    pub fn root_layer(&self) -> &Arc<Layer<T>> {
        &self.root_layer
    }

    /// Returns the only `LayerView` of the root layer.
    pub fn root_view(&self) -> &Arc<LayerView<T>> {
        // last one should never ever be empty.
        &self.root_view
    }

    /// Returns a mutex of all layer views of the whole scene.
    pub fn views(&self) -> &Mutex<Vec<Arc<LayerView<T>>>> {
        &self.ordered_views
    }

    /// Reorders the ordered views list.
    ///
    /// This method should not be used by the user but by the event loop integration using this engine.
    pub fn update(&self) {
        let mut views = self.ordered_views.lock();

        // fill new_views with new copies of the references before dropping the references
        // to avoid dropping the last arc of the root
        let mut new_views = Vec::with_capacity(views.capacity());

        self.root_layer.post_order_views(&mut new_views);

        *views = new_views;
    }

    /// Updates the scene physics and layers.
    #[cfg(feature = "physics")]
    pub fn physics_iteration(&self, physics: bool) -> Result<()> {
        let mut pipeline = self.physics_pipeline.lock();
        self.root_layer.physics_iteration(&mut pipeline, physics)?;

        Ok(())
    }
}

/// A layer struct holding it's own object hierarchy, camera and physics iteration.
#[derive(Debug)]
pub struct Layer<T: Loaded = ()> {
    pub(crate) objects: Mutex<Vec<NObject<T>>>,
    pub(crate) objects_map: Mutex<ObjectsMap<T>>,
    #[cfg(feature = "physics")]
    rigid_body_roots: Mutex<ObjectsMap<T>>,
    latest_object: AtomicU64,
    #[cfg(feature = "physics")]
    physics: Mutex<Physics>,
    #[cfg(feature = "physics")]
    physics_enabled: std::sync::atomic::AtomicBool,
    layers: Mutex<IndexSet<Arc<Layer<T>>>>,
    self_weak: Weak<Layer<T>>,
    parent_layer: Weak<Layer<T>>,
    views: Mutex<Vec<Weak<LayerView<T>>>>,
}

impl<T: Loaded> Layer<T> {
    /// Creates a new layer.
    pub(crate) fn new(parent_layer: Weak<Layer<T>>) -> Arc<Self> {
        Arc::new_cyclic(|weak| Self {
            objects: Mutex::new(Vec::new()),
            objects_map: Mutex::new(HashMap::default()),
            #[cfg(feature = "physics")]
            rigid_body_roots: Mutex::new(HashMap::default()),
            latest_object: 0.into(),
            #[cfg(feature = "physics")]
            physics: Mutex::new(Physics::new()),
            #[cfg(feature = "physics")]
            physics_enabled: std::sync::atomic::AtomicBool::new(true),
            layers: Default::default(),
            self_weak: weak.clone(),
            parent_layer,
            views: Default::default(),
        })
    }

    /// Creates a new root layer, where the layer view gets directly rendered on the window surface.
    fn new_root() -> (Arc<Self>, Arc<LayerView<T>>) {
        let layer = Arc::new_cyclic(|weak| Self {
            objects: Mutex::new(Vec::new()),
            objects_map: Mutex::new(HashMap::default()),
            #[cfg(feature = "physics")]
            rigid_body_roots: Mutex::new(HashMap::default()),
            latest_object: 0.into(),
            #[cfg(feature = "physics")]
            physics: Mutex::new(Physics::new()),
            #[cfg(feature = "physics")]
            physics_enabled: std::sync::atomic::AtomicBool::new(true),
            layers: Default::default(),
            parent_layer: weak.clone(),
            self_weak: weak.clone(),
            views: Default::default(),
        });

        let view = Arc::new(LayerView {
            parent: layer.self_weak.upgrade().unwrap(),
            camera: AtomicCell::new(Default::default()),
            draw: true.into(),
            extent: UVec2 { x: 0, y: 0 }.into(),
            scaling: Default::default(),
        });

        let weak = Arc::downgrade(&view);

        layer.views.lock().push(weak);

        (layer, view)
    }

    /// Post order traverses the whole layer structure and places the views into `views`.
    fn post_order_views(&self, views: &mut Vec<Arc<LayerView<T>>>) {
        let layers = self.layers.lock();
        for layer in layers.iter() {
            layer.post_order_views(views);
        }

        let mut local_views = self.views.lock();
        // clean unused views
        local_views.retain(|view| {
            if view.strong_count() <= 1 {
                return false;
            }
            views.push(view.upgrade().unwrap());

            true
        })
    }

    /// Initializes a new layer to only use in this layer.
    pub fn new_layer(&self) -> Arc<Layer<T>> {
        let layer = Layer::new(self.self_weak.clone());
        self.layers.lock().insert(layer.clone());

        layer
    }

    /// Returns an IndexSet of all layers present in this layer.
    pub fn layers(&self) -> IndexSet<Arc<Layer<T>>> {
        self.layers.lock().clone()
    }

    /// Returns a layer in this layer by index in case it exists.
    pub fn layer(&self, index: usize) -> Option<Arc<Layer<T>>> {
        self.layers.lock().get_index(index).cloned()
    }

    /// Returns a new viewpoint to this scene.
    ///
    /// Returns `None` in case this gets called on the root layer.
    ///
    /// You can not have multiple views of the root layer.
    pub fn new_view(
        &self,
        scene: &Scene<T>,
        camera: Camera,
        extent: UVec2,
        scaling: CameraScaling,
    ) -> Option<Arc<LayerView<T>>> {
        if self.self_weak.ptr_eq(&self.parent_layer) {
            return None;
        }

        let view = Arc::new(LayerView {
            parent: self.self_weak.upgrade().unwrap(),
            camera: AtomicCell::new(camera),
            draw: true.into(),
            extent: extent.into(),
            scaling: scaling.into(),
        });

        // Add view just to have one reference more before updating.
        scene.ordered_views.lock().push(view.clone());

        let weak = Arc::downgrade(&view);

        self.views.lock().push(weak);

        // reorder and update the scene views so they
        // get rendered in the right order.
        scene.update();

        Some(view)
    }

    /// Checks if the layer contains this object.
    pub fn contains_object(&self, object_id: &usize) -> bool {
        self.objects_map.lock().contains_key(object_id)
    }

    /// Increments the object ID counter by one and returns it.
    pub(crate) fn increment_id(&self) -> usize {
        self.latest_object.fetch_add(1, Ordering::AcqRel) as usize
    }

    pub(crate) fn add_object(&self, id: usize, object: &NObject<T>) {
        self.objects_map.lock().insert(id, object.clone());
    }

    /// Returns the number of objects in total initialized into this layer.
    pub fn number_of_objects(&self) -> usize {
        self.objects_map.lock().len()
    }

    /// Returns all children as nodes.
    ///
    /// Should be used by the graphics backend for drawing.
    pub fn children(&self) -> &Mutex<Vec<Arc<Mutex<Node<T>>>>> {
        &self.objects
    }

    /// Moves an object on the given index in it's parents children order.
    pub(crate) fn move_to(&self, object: &Object<T>, index: usize) -> Result<(), ObjectError> {
        let node = object.as_node()?;
        let count = Self::count_children(&node);

        if count < index {
            return Err(ObjectError::Move(format!(
                "This object can not be moved to {index}. You can not go above {count}"
            )));
        } else {
            Self::move_object_to(&node, index);
        }
        Ok(())
    }

    /// Moves an object one up in it's parents children order.
    pub(crate) fn move_up(&self, object: &Object<T>) -> Result<(), ObjectError> {
        let node = object.as_node()?;
        let object = &node.lock().object;
        if let Some(parent) = object.parent_node() {
            let children = &parent.lock().children;
            let index = Self::find_child_index(children, &node).unwrap();
            if index != 0 {
                let dst_node = children.get(index - 1).unwrap().clone();
                Self::swap_objects(&node, &dst_node);
            } else {
                return Err(ObjectError::Move(
                    "Object already on the top of the current layer.".to_string(),
                ));
            }
        } else {
            let objects = self.objects.lock();

            let index = Self::find_child_index(&objects, &node).unwrap();
            if index != 0 {
                let dst_node = objects.get(index - 1).unwrap().clone();
                Self::swap_objects(&node, &dst_node);
            } else {
                return Err(ObjectError::Move(
                    "Object already on the top of the current layer.".to_string(),
                ));
            }
        }

        Ok(())
    }

    /// Moves an object one down in it's parents children order.
    pub(crate) fn move_down(&self, object: &Object<T>) -> Result<(), ObjectError> {
        let node = object.as_node()?;
        if let Some(parent) = node.lock().object.parent_node() {
            let children = &parent.lock().children;
            let index = Self::find_child_index(children, &node).unwrap();

            let dst_node = parent
                .lock()
                .children
                .get(index + 1)
                .ok_or(ObjectError::Move(format!(
                    "Object already at the bottom of the layer: {index}"
                )))?
                .clone();
            Self::swap_objects(&node, &dst_node);
        } else {
            let objects = &self.objects.lock();

            let index = Self::find_child_index(objects, &node).unwrap();

            let dst_node = objects
                .get(index + 1)
                .ok_or(ObjectError::Move(format!(
                    "Object already at the bottom of the layer: {index}"
                )))?
                .clone();
            Self::swap_objects(&node, &dst_node);
        }
        Ok(())
    }

    /// Moves an object all the way to the top of it's parents children list.
    pub(crate) fn move_to_top(&self, object: &Object<T>) -> Result<(), ObjectError> {
        let node = object.as_node()?;
        Self::move_object_to(&node, 0);
        Ok(())
    }

    /// Moves an object all the way to the bottom of it's parents children list.
    pub(crate) fn move_to_bottom(&self, object: &Object<T>) -> Result<(), ObjectError> {
        let node = object.as_node()?;
        let count = Self::count_children(&node) - 1;
        Self::move_object_to(&node, count);
        Ok(())
    }

    /// Finds the index of the child in the parents children list.
    ///
    /// Returns `None` in case the child is not present in the object.
    fn find_child_index(parent_children: &[NObject<T>], object: &NObject<T>) -> Option<usize> {
        parent_children.iter().position(|x| Arc::ptr_eq(x, object))
    }

    /// Counts the amount of children the parent of the given object has.
    fn count_children(object: &NObject<T>) -> usize {
        let object = &object.lock().object;

        if let Some(parent) = object.parent_node() {
            let parent = parent.lock();

            parent.children.len()
        } else {
            let layer = object.layer();
            let objects = layer.objects.lock();
            objects.len()
        }
    }

    /// Moves an object on the given index in it's parents children order.
    fn move_object_to(src: &NObject<T>, dst: usize) {
        let object = &src.lock().object;
        if let Some(parent) = object.parent_node() {
            let mut parent = parent.lock();
            let objects = &mut parent.children;
            let index = objects.iter().position(|x| Arc::ptr_eq(x, src)).unwrap();
            let element = objects.remove(index);
            objects.insert(dst, element);
        } else {
            let layer = object.layer();
            let mut objects = layer.objects.lock();
            let index = objects.iter().position(|x| Arc::ptr_eq(x, src)).unwrap();
            let element = objects.remove(index);
            objects.insert(dst, element);
        };
    }

    fn swap_objects(src: &NObject<T>, dst: &NObject<T>) {
        let object = &src.lock().object;
        if let Some(parent) = object.parent_node() {
            let mut parent = parent.lock();
            let src_index = parent
                .children
                .iter()
                .position(|x| Arc::ptr_eq(x, src))
                .unwrap();
            let dst_index = parent
                .children
                .iter()
                .position(|x| Arc::ptr_eq(x, dst))
                .unwrap();
            parent.children.swap(src_index, dst_index);
        } else {
            let layer = object.layer();
            let mut objects = layer.objects.lock();

            let src_index = objects.iter().position(|x| Arc::ptr_eq(x, src)).unwrap();
            let dst_index = objects.iter().position(|x| Arc::ptr_eq(x, dst)).unwrap();

            objects.swap(src_index, dst_index);
        };
    }
}

impl<T: Loaded> Drop for Layer<T> {
    fn drop(&mut self) {
        let Some(parent) = self.parent_layer.upgrade() else {
            return;
        };

        let mut layers = parent.layers.lock();

        let layer = self.self_weak.upgrade().unwrap();
        layers.shift_remove(&layer);

        for object in layer.objects.lock().iter() {
            //delete all the children of the layer too.
            object.lock().remove_children(
                &mut layer.objects_map.lock(),
                #[cfg(feature = "physics")]
                &mut layer.rigid_body_roots.lock(),
            );
        }

        layers.shift_remove(&layer);
    }
}

#[cfg(feature = "physics")]
use rapier2d::prelude::*;

/// Physics
#[cfg_attr(docsrs, doc(cfg(feature = "physics")))]
#[cfg(feature = "physics")]
impl<T: Loaded> Layer<T> {
    /// Updates the scene physics and layers.
    pub fn physics_iteration(&self, pipeline: &mut PhysicsPipeline, physics: bool) -> Result<()> {
        let layers = self.layers.lock();

        if physics {
            self.step_physics(pipeline);
            for layer in layers.iter() {
                layer.physics_iteration(pipeline, physics)?;
            }
        }
        Ok(())
    }

    /// Used by the proc macro to initialize the physics for an object.
    pub(crate) fn physics(&self) -> &Mutex<Physics> {
        &self.physics
    }

    /// Returns all root objects of rigid bodies in this scene.
    pub(crate) fn rigid_body_roots(&self) -> &Mutex<ObjectsMap<T>> {
        &self.rigid_body_roots
    }

    /// Returns the nearest collider id from a specific location.
    pub fn query_nearest_collider_at(&self, position: Vec2) -> Option<usize> {
        let mut physics = self.physics.lock();
        physics.update_query_pipeline();

        let point = mint::Point2::from(position);
        let result = physics.query_pipeline.project_point(
            &physics.rigid_body_set,
            &physics.collider_set,
            &point.into(),
            true,
            QueryFilter::default(),
        );

        if let Some((handle, _)) = result {
            Some(physics.collider_set.get(handle).unwrap().user_data as usize)
        } else {
            None
        }
    }

    /// Returns id of the first collider intersecting with given ray.
    pub fn cast_ray(
        &self,
        position: Vec2,
        direction: Vec2,
        time_of_impact: Real,
        solid: bool,
    ) -> Option<usize> {
        let mut physics = self.physics.lock();
        physics.update_query_pipeline();

        let point = mint::Point2::from(position);
        let direction = mint::Vector2::from(direction);
        let result = physics.query_pipeline.cast_ray(
            &physics.rigid_body_set,
            &physics.collider_set,
            &Ray::new(point.into(), direction.into()),
            time_of_impact,
            solid,
            QueryFilter::default(),
        );

        if let Some((handle, _)) = result {
            Some(physics.collider_set.get(handle).unwrap().user_data as usize)
        } else {
            None
        }
    }
    /// Returns id of the first collider intersecting with given ray and returns a normal.
    pub fn cast_ray_and_get_normal(
        &self,
        position: Vec2,
        direction: Vec2,
        time_of_impact: Real,
        solid: bool,
    ) -> Option<(usize, Vec2)> {
        let mut physics = self.physics.lock();
        physics.update_query_pipeline();

        let point = mint::Point2::from(position);
        let direction = mint::Vector2::from(direction);
        let result = physics.query_pipeline.cast_ray_and_get_normal(
            &physics.rigid_body_set,
            &physics.collider_set,
            &Ray::new(point.into(), direction.into()),
            time_of_impact,
            solid,
            QueryFilter::default(),
        );

        if let Some((handle, intersection)) = result {
            let inter = intersection.normal;
            Some((
                physics.collider_set.get(handle).unwrap().user_data as usize,
                vec2(inter.x, inter.y),
            ))
        } else {
            None
        }
    }

    /// Returns id of the first collider intersecting with given ray.
    pub fn intersections_with_ray(
        &self,
        position: Vec2,
        direction: Vec2,
        time_of_impact: Real,
        solid: bool,
    ) -> Vec<usize> {
        let mut physics = self.physics.lock();
        physics.update_query_pipeline();

        let mut intersections = vec![];
        let bodies = &physics.rigid_body_set;
        let colliders = &physics.collider_set;
        let filter = QueryFilter::default();
        let mut callback = |handle| {
            intersections.push(physics.collider_set.get(handle).unwrap().user_data as usize);
            true
        };

        let point = mint::Point2::from(position);
        if direction.eq(&vec2(0.0, 0.0)) {
            physics.query_pipeline.intersections_with_point(
                bodies,
                colliders,
                &point.into(),
                filter,
                callback,
            );
        } else {
            let direction = mint::Vector2::from(direction);
            physics.query_pipeline.intersections_with_ray(
                bodies,
                colliders,
                &Ray::new(point.into(), direction.into()),
                time_of_impact,
                solid,
                filter,
                |handle, _| callback(handle),
            );
        };
        intersections
    }

    /// Cast a shape and return the first collider intersecting with it.
    pub fn intersection_with_shape(
        &self,
        shape: physics::Shape,
        position: (Vec2, f32),
    ) -> Option<usize> {
        let mut physics = self.physics.lock();
        physics.update_query_pipeline();

        let vec = mint::Vector2::from(position.0);
        let iso = nalgebra::Isometry2::new(vec.into(), position.1);
        let result = physics.query_pipeline.intersection_with_shape(
            &physics.rigid_body_set,
            &physics.collider_set,
            &iso,
            shape.0.as_ref(),
            QueryFilter::default(),
        );
        result.map(|handle| physics.collider_set.get(handle).unwrap().user_data as usize)
    }

    /// Cast a shape and return the first collider intersecting with it.
    pub fn intersections_with_shape(
        &self,
        shape: physics::Shape,
        position: (Vec2, f32),
    ) -> Vec<usize> {
        let mut physics = self.physics.lock();
        physics.update_query_pipeline();

        let mut intersections = vec![];
        let callback = |handle| {
            intersections.push(physics.collider_set.get(handle).unwrap().user_data as usize);
            true
        };

        let vec = mint::Vector2::from(position.0);
        let iso = nalgebra::Isometry2::new(vec.into(), position.1);
        physics.query_pipeline.intersections_with_shape(
            &physics.rigid_body_set,
            &physics.collider_set,
            &iso,
            shape.0.as_ref(),
            QueryFilter::default(),
            callback,
        );
        intersections
    }
    pub(crate) fn step_physics(&self, physics_pipeline: &mut PhysicsPipeline) {
        if self.physics_enabled.load(Ordering::Acquire) {
            let mut map = self.rigid_body_roots.lock();

            let mut physics = self.physics.lock();
            physics.step(physics_pipeline); // Rapier-side physics iteration run.
            for (_, object) in map.iter_mut() {
                let mut node = object.lock();
                let rigid_body = physics
                    .rigid_body_set
                    .get(node.object.rigidbody_handle().unwrap())
                    .unwrap();
                let pos = *rigid_body.translation();
                node.object
                    .set_isometry(vec2(pos.x, pos.y), rigid_body.rotation().angle());
            }
        }
    }

    /// Gets the gravity parameter.
    pub fn gravity(&self) -> Vec2 {
        let vec = self.physics.lock().gravity;

        vec2(vec.x, vec.y)
    }

    /// Sets the gravity parameter.
    pub fn set_gravity(&self, gravity: Vec2) {
        let vec = mint::Vector2::from(gravity);
        self.physics.lock().gravity = vec.into();
    }

    /// Returns if physics is enabled.
    pub fn physics_enabled(&self) -> bool {
        self.physics_enabled.load(Ordering::Acquire)
    }

    /// Set physics to be enabled or disabled.
    pub fn set_physics_enabled(&self, enabled: bool) {
        self.physics_enabled.store(enabled, Ordering::Release)
    }

    /// Takes the physics simulation parameters.
    pub fn physics_parameters(&self) -> IntegrationParameters {
        self.physics.lock().integration_parameters
    }

    /// Sets the physics simulation parameters.
    pub fn set_physics_parameters(&self, parameters: IntegrationParameters) {
        self.physics.lock().integration_parameters = parameters;
    }

    /// Adds a joint between object 1 and 2.
    pub fn add_joint(
        &self,
        object1: &Object<T>,
        object2: &Object<T>,
        data: impl Into<joints::GenericJoint>,
        wake_up: bool,
    ) -> Result<ImpulseJointHandle, NoRigidBodyError> {
        if let (Some(handle1), Some(handle2)) =
            (object1.rigidbody_handle(), object2.rigidbody_handle())
        {
            Ok(self.physics.lock().impulse_joint_set.insert(
                handle1,
                handle2,
                data.into().data,
                wake_up,
            ))
        } else {
            Err(NoRigidBodyError)
        }
    }

    /// Returns if the joint exists.
    pub fn joint(&self, handle: ImpulseJointHandle) -> Option<joints::GenericJoint> {
        self.physics
            .lock()
            .impulse_joint_set
            .get(handle)
            .map(|joint| joints::GenericJoint { data: joint.data })
    }

    /// Updates a joint.
    pub fn set_joint(
        &self,
        data: impl Into<joints::GenericJoint>,
        handle: ImpulseJointHandle,
    ) -> Result<(), joints::NoJointError> {
        if let Some(joint) = self.physics.lock().impulse_joint_set.get_mut(handle) {
            joint.data = data.into().data;
            Ok(())
        } else {
            Err(joints::NoJointError)
        }
    }

    /// Removes a joint.
    pub fn remove_joint(&self, handle: ImpulseJointHandle, wake_up: bool) {
        self.physics
            .lock()
            .impulse_joint_set
            .remove(handle, wake_up);
    }
}

impl<T: Loaded> PartialEq for Layer<T> {
    fn eq(&self, other: &Self) -> bool {
        self.self_weak.ptr_eq(&other.self_weak) && self.parent_layer.ptr_eq(&other.parent_layer)
    }
}

impl<T: Loaded> Eq for Layer<T> {}

impl<T: Loaded> std::hash::Hash for Layer<T> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.self_weak.as_ptr().hash(state);
        self.parent_layer.as_ptr().hash(state);
    }
}

/// `LayerView` represents a view or camera into a specific `Layer` in the game engine's hierarchical
/// layer system. A `LayerView` is used to render a particular `Layer` as a texture or directly
/// to the screen in the case of the root layer.
///
/// To delete a LayerView, drop the last reference to it.
///
/// In `camera`, the [`Transform`] acts as a camera, where `size` determines the zoom in both axis.
///
/// Setting the extent on the root view does not do anything.
#[derive(Debug)]
pub struct LayerView<T: Loaded> {
    parent: Arc<Layer<T>>,
    camera: AtomicCell<Camera>,
    draw: AtomicBool,
    extent: AtomicCell<UVec2>,
    scaling: AtomicCell<CameraScaling>,
}

impl<T: Loaded> LayerView<T> {
    /// Gets the camera.
    pub fn camera(&self) -> Camera {
        self.camera.load()
    }

    /// Sets the camera.
    pub fn set_camera(&self, camera: Camera) {
        self.camera.store(camera)
    }

    pub fn scaling(&self) -> CameraScaling {
        self.scaling.load()
    }

    pub fn set_scaling(&self, scaling: CameraScaling) {
        self.scaling.store(scaling);
    }

    pub fn extent(&self) -> UVec2 {
        self.extent.load()
    }

    pub fn set_extent(&self, extent: UVec2) {
        self.extent.store(extent);
    }

    /// Returns if this view gets drawn.
    pub fn draw(&self) -> bool {
        self.draw.load(Ordering::Acquire)
    }

    /// Sets if this view should be drawn in the next draw task.
    ///
    /// If this is true it will and the view will update, but when false
    /// the view will be stuck on the last drawn frame.
    pub fn set_draw(&self, draw: bool) {
        self.draw.store(draw, Ordering::Release)
    }

    /// Returns the parent layer of this view.
    pub fn layer(&self) -> &Arc<Layer<T>> {
        &self.parent
    }

    /// Returns the position of a given side with given window dimensions to world space.
    ///
    /// x -1.0 to 1.0 for left to right
    ///
    /// y -1.0 to 1.0 for up to down
    pub fn side_to_world(&self, direction: Vec2) -> Vec2 {
        // Change this to remove dimensions.

        let camera = self.camera.load();

        let dimensions = self.scaling().scale(self.extent.load().as_vec2());
        let zoom = 1.0 / camera.size;
        vec2(
            direction[0] * (dimensions.x * zoom.x) + camera.position.x * 2.0,
            -direction[1] * (dimensions.y * zoom.y) + camera.position.y * 2.0,
        )
    }

    /// Creates a projection matrix for the view.
    pub fn make_projection_matrix(&self) -> Mat4 {
        let scaled = self.scaling().scale(self.extent.load().as_vec2());
        // let scaled = scaled * 1.0 / self.transform.size;
        Mat4::orthographic_rh(-scaled.x, scaled.x, -scaled.y, scaled.y, -1.0, 1.0)
    }
}

// pub struct RootView<T: Loaded> {
//     view: LayerView,
// }
