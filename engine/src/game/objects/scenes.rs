use crate::{
    camera::{CameraScaling, CameraSettings},
    error::objects::*,
    utils::scale,
};

use super::{physics::Shape, physics::*, NObject, Node, Object, ObjectsMap};
use crossbeam::atomic::AtomicCell;
use glam::{vec2, Vec2};
use hashbrown::HashMap;
use indexmap::{indexset, IndexSet};
use parking_lot::Mutex;
use rapier2d::prelude::*;
use std::sync::{
    atomic::{AtomicBool, AtomicU64, Ordering},
    Arc,
};

/// The whole scene seen with all it's layers.
#[derive(Clone)]
pub struct Scene {
    layers: Arc<Mutex<IndexSet<Layer>>>,
    physics_pipeline: Arc<Mutex<PhysicsPipeline>>,
}

impl Scene {
    /// Iterates through all physics.
    pub fn iterate_all_physics(&self) {
        let mut pipeline = self.physics_pipeline.lock();
        let layers = self.layers.lock();

        for layer in layers.iter() {
            layer.step_physics(&mut pipeline);
        }
    }

    /// Initializes a new layer into the scene.
    pub fn new_layer(&self) -> Layer {
        let object = Object::default();

        let node = Layer::new(Arc::new(Mutex::new(Node {
            object,
            parent: None,
            rigid_body_parent: None,
            children: vec![],
        })));
        self.layers.lock().insert(node.clone());

        node
    }

    /// Removes a layer from the scene.
    pub fn remove_layer(&self, layer: &mut Layer) -> Result<(), NoObjectError> {
        let node: NObject;
        let mut layers = self.layers.lock();
        if layers.remove(layer) {
            node = layer.root.clone();
        } else {
            return Err(NoObjectError);
        }
        let mut objectguard = node.lock();

        //delete all the children of the layer too.
        objectguard.remove_children(
            &mut layer.objects_map.lock(),
            &mut layer.rigid_body_roots.lock(),
        );
        //finish him!
        layers.remove(layer);

        Ok(())
    }

    /// Returns an IndexSet of all layers.
    pub fn get_layers(&self) -> IndexSet<Layer> {
        self.layers.lock().clone()
    }

    //Add support to serialize and deserialize scenes. load and undload.
    //Add those functions to game.
}
impl Default for Scene {
    fn default() -> Self {
        Self {
            layers: Arc::new(Mutex::new(indexset![])),
            physics_pipeline: Arc::new(Mutex::new(PhysicsPipeline::new())),
        }
    }
}

/// A layer struct holding it's own object hierarchy, camera and physics iteration.
#[derive(Clone)]
pub struct Layer {
    pub(crate) root: NObject,
    pub(crate) camera: Arc<Mutex<NObject>>,
    camera_settings: Arc<AtomicCell<CameraSettings>>,
    pub(crate) objects_map: Arc<Mutex<ObjectsMap>>,
    rigid_body_roots: Arc<Mutex<ObjectsMap>>,
    latest_object: Arc<AtomicU64>,
    physics: Arc<Mutex<Physics>>,
    physics_enabled: Arc<AtomicBool>,
}

impl Layer {
    /// Creates a new layer with the given root.
    pub fn new(root: NObject) -> Self {
        let mut objects_map = HashMap::new();
        objects_map.insert(0, root.clone());
        Self {
            root: root.clone(),
            camera: Arc::new(Mutex::new(root)),
            camera_settings: Arc::new(AtomicCell::new(CameraSettings::default())),
            objects_map: Arc::new(Mutex::new(objects_map)),
            rigid_body_roots: Arc::new(Mutex::new(HashMap::new())),
            latest_object: Arc::new(AtomicU64::new(1)),
            physics: Arc::new(Mutex::new(Physics::new())),
            physics_enabled: Arc::new(AtomicBool::new(true)),
        }
    }
    /// Used by the proc macro to initialize the physics for an object.
    pub(crate) fn physics(&self) -> &Arc<Mutex<Physics>> {
        &self.physics
    }
    pub(crate) fn rigid_body_roots(&self) -> &Arc<Mutex<ObjectsMap>> {
        &self.rigid_body_roots
    }
    /// Sets the camera of this layer.
    pub fn set_camera(&self, camera: &Object) {
        *self.camera.lock() = camera.as_node().expect("camera uninitialized");
    }
    pub(crate) fn camera_position(&self) -> Vec2 {
        self.camera.lock().lock().object.transform.position
    }

    /// Returns the scaling of the camera settings.
    pub fn camera_scaling(&self) -> CameraScaling {
        self.camera_settings.load().mode
    }

    /// Returns the zoom of the camera settings.
    pub fn zoom(&self) -> f32 {
        self.camera_settings.load().zoom
    }

    pub fn set_zoom(&self, zoom: f32) {
        let settings = self.camera_settings();
        self.camera_settings.store(settings.zoom(zoom))
    }

    /// Sets the camera settings.
    pub fn set_camera_settings(&self, settings: CameraSettings) {
        self.camera_settings.store(settings)
    }

    /// Gets the camera settins.
    pub fn camera_settings(&self) -> CameraSettings {
        self.camera_settings.load()
    }

    /// Returns the position of a given side with given window dimensions to world space.
    ///
    /// Be careful! Don't use this when the camera is locked.
    pub fn side_to_world(&self, direction: [f32; 2], dimensions: Vec2) -> Vec2 { // Change this to remove dimensions.
        let camera = Self::camera_position(self);
        let direction = [direction[0] * 2.0 - 1.0, direction[1] * 2.0 - 1.0];
        let dimensions = scale(Self::camera_scaling(self), dimensions);
        let zoom = 1.0 / Self::zoom(self);
        vec2(
            direction[0] * (dimensions.x * zoom) + camera.x * 2.0,
            direction[1] * (dimensions.y * zoom) + camera.y * 2.0,
        )
    }

    /// Checks if the layer contains this object.
    pub fn contains_object(&self, object_id: &usize) -> bool {
        self.objects_map.lock().contains_key(object_id)
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
                node.object.set_isometry(
                    (*rigid_body.translation()).into(),
                    rigid_body.rotation().angle(),
                );
            }
        }
    }

    /// Gets the gravity parameter.
    pub fn gravity(&self) -> Vec2 {
        self.physics.lock().gravity.into()
    }
    /// Sets the gravity parameter.
    pub fn set_gravity(&self, gravity: Vec2) {
        self.physics.lock().gravity = gravity.into();
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
    pub fn get_joint(&self, handle: ImpulseJointHandle) -> Option<joints::GenericJoint> {
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
    ) -> Result<(), NoJointError> {
        if let Some(joint) = self.physics.lock().impulse_joint_set.get_mut(handle) {
            joint.data = data.into().data;
            Ok(())
        } else {
            Err(NoJointError)
        }
    }
    /// Removes a joint.
    pub fn remove_joint(&self, handle: ImpulseJointHandle, wake_up: bool) {
        self.physics
            .lock()
            .impulse_joint_set
            .remove(handle, wake_up);
    }

    /// Increments the object ID counter by one and returns it.
    pub(crate) fn increment_id(&self) -> usize {
        self.latest_object.fetch_add(1, Ordering::AcqRel) as usize
    }

    pub(crate) fn add_object(&self, id: usize, object: &NObject) {
        self.objects_map.lock().insert(id, object.clone());
    }

    /// Returns the nearest collider id from a specific location.
    pub fn query_nearest_collider_at(&self, position: Vec2) -> Option<usize> {
        let mut physics = self.physics.lock();
        physics.update_query_pipeline();

        let result = physics.query_pipeline.project_point(
            &physics.rigid_body_set,
            &physics.collider_set,
            &position.into(),
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

        let result = physics.query_pipeline.cast_ray(
            &physics.rigid_body_set,
            &physics.collider_set,
            &Ray::new(position.into(), direction.into()),
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

        let result = physics.query_pipeline.cast_ray_and_get_normal(
            &physics.rigid_body_set,
            &physics.collider_set,
            &Ray::new(position.into(), direction.into()),
            time_of_impact,
            solid,
            QueryFilter::default(),
        );

        if let Some((handle, intersection)) = result {
            Some((
                physics.collider_set.get(handle).unwrap().user_data as usize,
                intersection.normal.into(),
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

        if direction.eq(&vec2(0.0, 0.0)) {
            physics.query_pipeline.intersections_with_point(
                bodies,
                colliders,
                &position.into(),
                filter,
                callback,
            );
        } else {
            physics.query_pipeline.intersections_with_ray(
                bodies,
                colliders,
                &Ray::new(position.into(), direction.into()),
                time_of_impact,
                solid,
                filter,
                |handle, _| callback(handle),
            );
        };
        intersections
    }

    /// Cast a shape and return the first collider intersecting with it.
    pub fn intersection_with_shape(&self, shape: Shape, position: (Vec2, f32)) -> Option<usize> {
        let mut physics = self.physics.lock();
        physics.update_query_pipeline();

        let result = physics.query_pipeline.intersection_with_shape(
            &physics.rigid_body_set,
            &physics.collider_set,
            &position.into(),
            shape.0.as_ref(),
            QueryFilter::default(),
        );
        result.map(|handle| physics.collider_set.get(handle).unwrap().user_data as usize)
    }

    /// Cast a shape and return the first collider intersecting with it.
    pub fn intersections_with_shape(&self, shape: Shape, position: (Vec2, f32)) -> Vec<usize> {
        let mut physics = self.physics.lock();
        physics.update_query_pipeline();

        let mut intersections = vec![];
        let callback = |handle| {
            intersections.push(physics.collider_set.get(handle).unwrap().user_data as usize);
            true
        };

        physics.query_pipeline.intersections_with_shape(
            &physics.rigid_body_set,
            &physics.collider_set,
            &position.into(),
            shape.0.as_ref(),
            QueryFilter::default(),
            callback,
        );
        intersections
    }

    /// Moves an object on the given index in it's parents children order.
    pub fn move_to(&self, object: &Object, index: usize) -> Result<(), Box<dyn std::error::Error>> {
        let node = object.as_node().expect("object uninitialized");
        let count = Self::count_children(&node);

        if count > index {
            return Err(Box::new(MoveError));
        } else {
            Self::move_object_to(node, index);
        }
        Ok(())
    }

    /// Moves an object one down in it's parents children order.
    pub fn move_down(&self, object: &Object) -> Result<(), Box<dyn std::error::Error>> {
        let node = object.as_node().expect("object uninitialized");
        let parent = Self::get_parent(&node);
        let index = Self::find_child_index(&parent, &node);
        if index == 0 {
            return Err(Box::new(MoveError));
        } else {
            Self::move_object_to(node, index - 1);
        }
        Ok(())
    }

    /// Moves an object one up in it's parents children order.
    pub fn move_up(&self, object: &Object) -> Result<(), Box<dyn std::error::Error>> {
        let node = object.as_node().expect("object uninitialized");
        let parent = Self::get_parent(&node);
        let count = Self::count_children(&node);
        let index = Self::find_child_index(&parent, &node);
        if count == index {
            return Err(Box::new(MoveError));
        } else {
            Self::move_object_to(node, count + 1);
        }
        Ok(())
    }

    /// Moves an object all the way to the top of it's parents children list.
    pub fn move_to_top(&self, object: &Object) -> Result<(), NoObjectError> {
        let node = object.as_node().expect("object uninitialized");
        Self::move_object_to(node, 0);
        Ok(())
    }

    /// Moves an object all the way to the bottom of it's parents children list.
    pub fn move_to_bottom(&self, object: &Object) -> Result<(), NoObjectError> {
        let node = object.as_node().expect("object uninitialized");
        let count = Self::count_children(&node) - 1;
        Self::move_object_to(node, count);
        Ok(())
    }

    fn get_parent(object: &NObject) -> NObject {
        object.lock().parent.clone().unwrap().upgrade().unwrap()
    }

    fn find_child_index(parent: &NObject, object: &NObject) -> usize {
        let parent = parent.lock();
        parent
            .children
            .clone()
            .into_iter()
            .position(|x| Arc::ptr_eq(&x, object))
            .unwrap()
    }

    fn count_children(object: &NObject) -> usize {
        let parent = Self::get_parent(object);
        let parent = parent.lock();
        parent.children.len()
    }

    /// Moves an object on the given index in it's parents children order.
    fn move_object_to(src: NObject, dst: usize) {
        let parent = src.lock().parent.clone().unwrap().upgrade().unwrap();
        let mut parent = parent.lock();
        let index = parent
            .children
            .clone()
            .into_iter()
            .position(|x| Arc::ptr_eq(&x, &src))
            .unwrap();
        parent.children.swap(index, dst);
    }

    pub fn children_count(&self, parent: &Object) -> Result<usize, NoObjectError> {
        let node = parent.as_node().expect("object uninitialized");
        Ok(Self::count_children(&node))
    }
}

impl PartialEq for Layer {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.root, &other.root)
            && Arc::ptr_eq(&self.camera, &other.camera)
            && Arc::ptr_eq(&self.objects_map, &other.objects_map)
    }
}

impl Eq for Layer {}

impl std::hash::Hash for Layer {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        Arc::as_ptr(&self.root).hash(state);
        Arc::as_ptr(&self.camera).hash(state);
        Arc::as_ptr(&self.objects_map).hash(state);
    }
}
