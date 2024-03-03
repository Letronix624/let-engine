use crate::{error::objects::*, prelude::*};

use super::ObjectsMap;
use anyhow::Result;
use crossbeam::atomic::AtomicCell;
use indexmap::{indexset, IndexSet};

#[cfg(feature = "audio")]
use kira::spatial::listener::ListenerHandle;
use parking_lot::Mutex;
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
};

/// The whole scene seen with all it's layers.
pub struct Scene {
    layers: Mutex<IndexSet<Arc<Layer>>>,
    #[cfg(feature = "physics")]
    physics_pipeline: Mutex<PhysicsPipeline>,
}

impl Scene {
    /// Updates the scene physics and layers.
    #[cfg(feature = "physics")]
    pub fn update(&self, physics: bool) -> Result<()> {
        let layers = self.layers.lock();

        let mut pipeline = self.physics_pipeline.lock();
        if physics {
            for layer in layers.iter() {
                layer.step_physics(&mut pipeline);
                #[cfg(feature = "audio")]
                layer.update()?;
            }
        } else {
            #[cfg(feature = "audio")]
            for layer in layers.iter() {
                layer.update()?;
            }
        }
        Ok(())
    }

    /// Updates all the layers.
    #[cfg(all(feature = "audio", not(feature = "physics")))]
    pub fn update(&self) {
        let layers = self.layers.lock();
        for layer in layers.iter() {
            layer.update();
        }
    }

    /// Initializes a new layer into the scene.
    pub fn new_layer(&self) -> Arc<Layer> {
        let layer = Layer::new().unwrap();
        self.layers.lock().insert(layer.clone());

        layer
    }

    /// Removes a layer from the scene.
    pub fn remove_layer(&self, layer: &mut Layer) -> Result<(), NoLayerError> {
        let node: NObject;
        let mut layers = self.layers.lock();
        if layers.shift_remove(layer) {
            node = layer.root.clone();
        } else {
            return Err(NoLayerError);
        }
        let mut objectguard = node.lock();

        //delete all the children of the layer too.
        objectguard.remove_children(
            &mut layer.objects_map.lock(),
            #[cfg(feature = "physics")]
            &mut layer.rigid_body_roots.lock(),
        );
        layers.shift_remove(layer);

        Ok(())
    }

    /// Returns an IndexSet of all layers.
    pub fn layers(&self) -> IndexSet<Arc<Layer>> {
        self.layers.lock().clone()
    }

    /// Returns a layer by index in case it exists.
    pub fn layer(&self, index: usize) -> Option<Arc<Layer>> {
        self.layers.lock().get_index(index).cloned()
    }

    //Add support to serialize and deserialize scenes. load and unload.
    //Add those functions to game.
}
impl Default for Scene {
    fn default() -> Self {
        Self {
            layers: Mutex::new(indexset![]),
            #[cfg(feature = "physics")]
            physics_pipeline: Mutex::new(PhysicsPipeline::new()),
        }
    }
}

/// A layer struct holding it's own object hierarchy, camera and physics iteration.
pub struct Layer {
    pub(crate) root: NObject,
    pub(crate) camera: Mutex<NObject>,
    #[cfg(feature = "audio")]
    old_camera: Mutex<NewObject>,
    camera_settings: AtomicCell<CameraSettings>,
    pub(crate) objects_map: Mutex<ObjectsMap>,
    #[cfg(feature = "physics")]
    rigid_body_roots: Mutex<ObjectsMap>,
    latest_object: AtomicU64,
    #[cfg(feature = "physics")]
    physics: Mutex<Physics>,
    #[cfg(feature = "physics")]
    physics_enabled: std::sync::atomic::AtomicBool,
    #[cfg(feature = "audio")]
    pub(crate) listener: Mutex<std::sync::OnceLock<ListenerHandle>>,
}

impl Layer {
    /// Creates a new layer with the given root.
    pub(crate) fn new() -> Result<Arc<Self>> {
        let root = Arc::new_cyclic(|weak| {
            Mutex::new(Node {
                object: Object::root(weak.clone()),
                // parent: None,
                #[cfg(feature = "physics")]
                rigid_body_parent: None,
                children: vec![],
            })
        });
        let mut objects_map = HashMap::new();
        objects_map.insert(0, root.clone());
        let layer = Arc::new(Self {
            root: root.clone(),
            camera: Mutex::new(root),
            #[cfg(feature = "audio")]
            old_camera: Mutex::new(NewObject::default()),
            camera_settings: AtomicCell::new(CameraSettings::default()),
            objects_map: Mutex::new(objects_map),
            #[cfg(feature = "physics")]
            rigid_body_roots: Mutex::new(HashMap::new()),
            latest_object: AtomicU64::new(1),
            #[cfg(feature = "physics")]
            physics: Mutex::new(Physics::new()),
            #[cfg(feature = "physics")]
            physics_enabled: std::sync::atomic::AtomicBool::new(true),
            #[cfg(feature = "audio")]
            listener: Mutex::new(std::sync::OnceLock::new()),
        });
        #[cfg(feature = "audio")]
        RESOURCES
            .audio_server
            .send(AudioUpdate::NewLayer(layer.clone()))?;
        #[allow(clippy::let_and_return)]
        Ok(layer)
    }
    /// Used by the proc macro to initialize the physics for an object.
    #[cfg(feature = "physics")]
    pub(crate) fn physics(&self) -> &Mutex<Physics> {
        &self.physics
    }
    #[cfg(feature = "physics")]
    pub(crate) fn rigid_body_roots(&self) -> &Mutex<ObjectsMap> {
        &self.rigid_body_roots
    }
    /// Sets the camera of this layer.
    pub fn set_camera(&self, camera: &Object) -> Result<(), ObjectError> {
        *self.camera.lock() = camera.as_node()?;
        Ok(())
    }

    /// Returns the position of the camera object.
    #[allow(dead_code)]
    pub(crate) fn camera_transform(&self) -> Transform {
        self.camera.lock().lock().object.transform
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
    /// x -1.0 to 1.0 for left to right
    ///
    /// y -1.0 to 1.0 for up to down
    #[cfg(feature = "client")]
    pub fn side_to_world(&self, direction: Vec2) -> Vec2 {
        // Change this to remove dimensions.
        let camera = self.camera_transform();
        let dimensions = if let Some(window) = SETTINGS.window() {
            window.inner_size()
        } else {
            vec2(1000.0, 1000.0)
        };
        let dimensions = Self::camera_scaling(self).scale(dimensions);
        let zoom = 1.0 / Self::zoom(self);
        vec2(
            direction[0] * (dimensions.x * zoom) + camera.position.x * 2.0,
            -direction[1] * (dimensions.y * zoom) + camera.position.y * 2.0,
        )
    }

    /// Checks if the layer contains this object.
    pub fn contains_object(&self, object_id: &usize) -> bool {
        self.objects_map.lock().contains_key(object_id)
    }
    #[cfg(feature = "audio")]
    pub(crate) fn update(&self) -> Result<()> {
        use glam::Quat;

        let mut old_camera = self.old_camera.lock();
        let camera = self.camera.lock().lock().object.to_new();
        if *old_camera != camera {
            *old_camera = camera;
            if let Some(listener) = self.listener.lock().get_mut() {
                let cam_transform = self.camera_transform();
                listener
                    .set_position(cam_transform.position.extend(0.0), Tween::default().into())?;
                listener.set_orientation(
                    Quat::from_rotation_z(cam_transform.rotation),
                    Tween::default().into(),
                )?;
            }
        }
        Ok(())
    }
    /// Increments the object ID counter by one and returns it.
    pub(crate) fn increment_id(&self) -> usize {
        self.latest_object.fetch_add(1, Ordering::AcqRel) as usize
    }

    pub(crate) fn add_object(&self, id: usize, object: &NObject) {
        self.objects_map.lock().insert(id, object.clone());
    }

    /// Moves an object on the given index in it's parents children order.
    ///
    /// Returns
    pub fn move_to(&self, object: &Object, index: usize) -> Result<(), ObjectError> {
        let node = object.as_node()?;
        let count = Self::count_children(&node).ok_or(ObjectError::NoParent)?;

        if count < index {
            return Err(ObjectError::Move(format!(
                "This object can not be moved to {index}.\nYou can not go above {count}"
            )));
        } else {
            Self::move_object_to(node, index);
        }
        Ok(())
    }

    /// Moves an object one up in it's parents children order.
    pub fn move_up(&self, object: &Object) -> Result<(), ObjectError> {
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
            Self::move_object_to(node, index - 1);
        }
        Ok(())
    }

    /// Moves an object one down in it's parents children order.
    pub fn move_down(&self, object: &Object) -> Result<(), ObjectError> {
        let node = object.as_node()?;
        if Arc::ptr_eq(&node, &self.root) {
            return Err(ObjectError::NoParent);
        }
        let parent = node.lock().object.parent_node();
        let count = Self::count_children(&node).ok_or(ObjectError::NoParent)?;
        let index = Self::find_child_index(&parent, &node).ok_or(ObjectError::NoParent)?;
        if count == index {
            return Err(ObjectError::Move(format!(
                "Object already at the bottom of the layer: {index}"
            )));
        } else {
            Self::move_object_to(node, count + 1);
        }
        Ok(())
    }

    /// Moves an object all the way to the top of it's parents children list.
    pub fn move_to_top(&self, object: &Object) -> Result<(), ObjectError> {
        let node = object.as_node()?;
        Self::move_object_to(node, 0);
        Ok(())
    }

    /// Moves an object all the way to the bottom of it's parents children list.
    pub fn move_to_bottom(&self, object: &Object) -> Result<(), ObjectError> {
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

    pub fn children_count(&self, parent: &Object) -> Result<usize, ObjectError> {
        let node = parent.as_node()?;
        Self::count_children(&node).ok_or(ObjectError::NoParent)
    }
}

#[cfg(feature = "physics")]
use rapier2d::prelude::*;

/// Physics
#[cfg_attr(docsrs, doc(cfg(feature = "physics")))]
#[cfg(feature = "physics")]
impl Layer {
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
    pub fn intersection_with_shape(
        &self,
        shape: physics::Shape,
        position: (Vec2, f32),
    ) -> Option<usize> {
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
