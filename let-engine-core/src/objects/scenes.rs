use super::*;
use crate::camera::*;
use anyhow::Result;
use crossbeam::atomic::AtomicCell;
use indexmap::IndexSet;

use parking_lot::Mutex;
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc, LazyLock,
    },
};

/// The engine wide scene holding all objects in layers.
pub static SCENE: LazyLock<crate::objects::scenes::Scene> =
    LazyLock::new(crate::objects::scenes::Scene::new);

/// The whole scene seen with all it's layers.
pub struct Scene {
    root_layer: Arc<Layer>,
    ordered_views: Mutex<Vec<Arc<LayerView>>>,
    // Keep to avoid dropping
    root_view: Arc<LayerView>,
    #[cfg(feature = "physics")]
    physics_pipeline: Mutex<PhysicsPipeline>,
}

impl Scene {
    fn new() -> Self {
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
    pub fn root_layer(&self) -> &Arc<Layer> {
        &self.root_layer
    }

    /// Returns the only `LayerView` of the root layer.
    pub fn root_view(&self) -> Arc<LayerView> {
        // last one should never ever be empty.
        self.root_view.clone()
    }

    pub(crate) fn views(&self) -> &Mutex<Vec<Arc<LayerView>>> {
        &self.ordered_views
    }

    /// Reorders the ordered views list
    pub(crate) fn update(&self) {
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
pub struct Layer {
    pub(crate) root: NObject,
    pub(crate) objects_map: Mutex<ObjectsMap>,
    #[cfg(feature = "physics")]
    rigid_body_roots: Mutex<ObjectsMap>,
    latest_object: AtomicU64,
    #[cfg(feature = "physics")]
    physics: Mutex<Physics>,
    #[cfg(feature = "physics")]
    physics_enabled: std::sync::atomic::AtomicBool,
    layers: Mutex<IndexSet<Arc<Layer>>>,
    self_weak: Weak<Layer>,
    parent_layer: Weak<Layer>,
    views: Mutex<Vec<Weak<LayerView>>>,
}

impl Layer {
    /// Creates a new layer.
    pub(crate) fn new(parent_layer: Weak<Layer>) -> Arc<Self> {
        let root = Arc::new_cyclic(|weak| {
            Mutex::new(Node {
                object: Object::root(weak.clone()),
                #[cfg(feature = "physics")]
                rigid_body_parent: None,
                children: vec![],
            })
        });
        let mut objects_map = HashMap::new();
        objects_map.insert(0, root.clone());
        Arc::new_cyclic(|weak| Self {
            root: root.clone(),
            objects_map: Mutex::new(objects_map),
            #[cfg(feature = "physics")]
            rigid_body_roots: Mutex::new(HashMap::new()),
            latest_object: AtomicU64::new(1),
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

    fn new_root() -> (Arc<Self>, Arc<LayerView>) {
        let root = Arc::new_cyclic(|weak| {
            Mutex::new(Node {
                object: Object::root(weak.clone()),
                #[cfg(feature = "physics")]
                rigid_body_parent: None,
                children: vec![],
            })
        });
        let mut objects_map = HashMap::new();
        objects_map.insert(0, root.clone());
        let layer = Arc::new_cyclic(|weak| Self {
            root: root.clone(),
            objects_map: Mutex::new(objects_map),
            #[cfg(feature = "physics")]
            rigid_body_roots: Mutex::new(HashMap::new()),
            latest_object: AtomicU64::new(1),
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
        });

        let weak = Arc::downgrade(&view);

        layer.views.lock().push(weak);

        (layer, view)
    }

    /// Post order traverses the whole layer structure and places the views into `views`.
    fn post_order_views(&self, views: &mut Vec<Arc<LayerView>>) {
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
    pub fn new_layer(&self) -> Arc<Layer> {
        let layer = Layer::new(self.self_weak.clone());
        self.layers.lock().insert(layer.clone());

        layer
    }

    /// Returns a new viewpoint to this scene.
    ///
    /// Returns `None` in case this gets called on the root layer.
    ///
    /// You can not have multiple views of the root layer.
    pub fn new_view(&self, camera: Camera) -> Option<Arc<LayerView>> {
        if self.self_weak.ptr_eq(&self.parent_layer) {
            return None;
        }

        let view = Arc::new(LayerView {
            parent: self.self_weak.upgrade().unwrap(),
            camera: AtomicCell::new(camera),
            draw: true.into(),
        });

        // Add view just to have one reference more before updating.
        SCENE.ordered_views.lock().push(view.clone());

        let weak = Arc::downgrade(&view);

        self.views.lock().push(weak);

        // reorder and update the scene views so they
        // get rendered in the right order.
        SCENE.update();

        Some(view)
    }

    /// Returns an IndexSet of all layers present in this layer.
    pub fn layers(&self) -> IndexSet<Arc<Layer>> {
        self.layers.lock().clone()
    }

    /// Returns a layer in this layer by index in case it exists.
    pub fn layer(&self, index: usize) -> Option<Arc<Layer>> {
        self.layers.lock().get_index(index).cloned()
    }

    /// Checks if the layer contains this object.
    pub fn contains_object(&self, object_id: &usize) -> bool {
        self.objects_map.lock().contains_key(object_id)
    }

    /// Increments the object ID counter by one and returns it.
    pub(crate) fn increment_id(&self) -> usize {
        self.latest_object.fetch_add(1, Ordering::AcqRel) as usize
    }

    pub(crate) fn add_object(&self, id: usize, object: &NObject) {
        self.objects_map.lock().insert(id, object.clone());
    }

    /// Moves an object on the given index in it's parents children order.
    pub(crate) fn move_to(&self, object: &Object, index: usize) -> Result<(), ObjectError> {
        let node = object.as_node()?;
        let count = Self::count_children(&node).ok_or(ObjectError::NoParent)?;

        if count < index {
            return Err(ObjectError::Move(format!(
                "This object can not be moved to {index}. You can not go above {count}"
            )));
        } else {
            Self::move_object_to(node, index);
        }
        Ok(())
    }

    /// Moves an object one up in it's parents children order.
    pub(crate) fn move_up(&self, object: &Object) -> Result<(), ObjectError> {
        let node = object.as_node()?;
        if Arc::ptr_eq(&node, &self.root) {
            return Err(ObjectError::NoParent);
        }
        let parent = node.lock().object.parent_node();
        let index = Self::find_child_index(&parent, &node).ok_or(ObjectError::NoParent)?;
        if index == 0 {
            return Err(ObjectError::Move(
                "Object already on the top of the current layer.".to_string(),
            ));
        } else {
            let dst_node = parent.lock().children.get(index - 1).unwrap().clone();
            Self::swap_objects(node, dst_node);
        }
        Ok(())
    }

    /// Moves an object one down in it's parents children order.
    pub(crate) fn move_down(&self, object: &Object) -> Result<(), ObjectError> {
        let node = object.as_node()?;
        if Arc::ptr_eq(&node, &self.root) {
            return Err(ObjectError::NoParent);
        }
        let parent = node.lock().object.parent_node();
        let index = Self::find_child_index(&parent, &node).ok_or(ObjectError::NoParent)?;

        let dst_node = parent
            .lock()
            .children
            .get(index + 1)
            .ok_or(ObjectError::Move(format!(
                "Object already at the bottom of the layer: {index}"
            )))?
            .clone();
        Self::swap_objects(node, dst_node);
        Ok(())
    }

    /// Moves an object all the way to the top of it's parents children list.
    pub(crate) fn move_to_top(&self, object: &Object) -> Result<(), ObjectError> {
        let node = object.as_node()?;
        Self::move_object_to(node, 0);
        Ok(())
    }

    /// Moves an object all the way to the bottom of it's parents children list.
    pub(crate) fn move_to_bottom(&self, object: &Object) -> Result<(), ObjectError> {
        let node = object.as_node()?;
        let count = Self::count_children(&node).ok_or(ObjectError::NoParent)? - 1;
        Self::move_object_to(node, count);
        Ok(())
    }

    /// Finds the index of the child in the parents children list.
    ///
    /// Returns `None` in case the child is not present in the object.
    fn find_child_index(parent: &NObject, object: &NObject) -> Option<usize> {
        let parent = parent.lock();
        parent
            .children
            .clone()
            .into_iter()
            .position(|x| Arc::ptr_eq(&x, object))
    }

    /// Counts the amount of children the parent of the given object has.
    ///
    /// Returns none in case there is no parent.
    fn count_children(object: &NObject) -> Option<usize> {
        let parent = object.lock().object.parent_node();
        let parent = parent.lock();
        Some(parent.children.len())
    }

    /// Moves an object on the given index in it's parents children order.
    fn move_object_to(src: NObject, dst: usize) {
        let parent = src.lock().object.parent_node();
        let mut parent = parent.lock();
        let index = parent
            .children
            .clone()
            .into_iter()
            .position(|x| Arc::ptr_eq(&x, &src))
            .unwrap();
        let element = parent.children.remove(index);
        parent.children.insert(dst, element);
    }

    fn swap_objects(src: NObject, dst: NObject) {
        let parent = src.lock().object.parent_node();
        let mut parent = parent.lock();
        let src_index = parent
            .children
            .clone()
            .into_iter()
            .position(|x| Arc::ptr_eq(&x, &src))
            .unwrap();
        let dst_index = parent
            .children
            .clone()
            .into_iter()
            .position(|x| Arc::ptr_eq(&x, &dst))
            .unwrap();

        parent.children.swap(src_index, dst_index);
    }
}

impl Drop for Layer {
    fn drop(&mut self) {
        let node: NObject;
        let parent = self.parent_layer.upgrade().unwrap();
        let mut layers = parent.layers.lock();

        let layer = self.self_weak.upgrade().unwrap();
        if layers.shift_remove(&layer) {
            node = layer.root.clone();
        } else {
            return;
        }
        let mut objectguard = node.lock();

        //delete all the children of the layer too.
        objectguard.remove_children(
            &mut layer.objects_map.lock(),
            #[cfg(feature = "physics")]
            &mut layer.rigid_body_roots.lock(),
        );
        layers.shift_remove(&layer);
    }
}

#[cfg(feature = "physics")]
use rapier2d::prelude::*;

/// Physics
#[cfg_attr(docsrs, doc(cfg(feature = "physics")))]
#[cfg(feature = "physics")]
impl Layer {
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
    pub(crate) fn rigid_body_roots(&self) -> &Mutex<ObjectsMap> {
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
        object1: &Object,
        object2: &Object,
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

impl PartialEq for Layer {
    fn eq(&self, other: &Self) -> bool {
        *self.root.lock() == *other.root.lock()
    }
}

impl Eq for Layer {}

impl std::hash::Hash for Layer {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        Arc::as_ptr(&self.root).hash(state);
    }
}

/// `LayerView` represents a view or camera into a specific `Layer` in the game engine's hierarchical
/// layer system. A `LayerView` is used to render a particular `Layer` as a texture or directly
/// to the screen in the case of the root layer.
///
/// To delete a LayerView, drop the last reference to it.
pub struct LayerView {
    parent: Arc<Layer>,
    camera: AtomicCell<Camera>,
    draw: AtomicBool,
}

impl LayerView {
    /// Gets the camera.
    pub fn camera(&self) -> Camera {
        self.camera.load()
    }

    /// Sets the camera.
    pub fn set_camera(&self, camera: Camera) {
        self.camera.store(camera)
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
    pub fn layer(&self) -> &Arc<Layer> {
        &self.parent
    }

    /// Returns the position of a given side with given window dimensions to world space.
    ///
    /// x -1.0 to 1.0 for left to right
    ///
    /// y -1.0 to 1.0 for up to down
    #[cfg(feature = "client")]
    pub fn side_to_world(&self, direction: Vec2) -> Vec2 {
        // Change this to remove dimensions.

        use crate::window::WINDOW;

        let camera = self.camera.load();
        let dimensions = if let Some(window) = WINDOW.get() {
            window.inner_size()
        } else {
            vec2(1000.0, 1000.0)
        };

        let dimensions = camera.scaling.scale(dimensions);
        let zoom = 1.0 / camera.transform.size;
        vec2(
            direction[0] * (dimensions.x * zoom.x) + camera.transform.position.x * 2.0,
            -direction[1] * (dimensions.y * zoom.y) + camera.transform.position.y * 2.0,
        )
    }
}
