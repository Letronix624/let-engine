//! Physics related structs.

use crate::objects::Transform;
use glam::f32::Vec2;
use nalgebra::Isometry2;
use parking_lot::Mutex;
pub use rapier2d::parry::transformation::vhacd::VHACDParameters;
use rapier2d::prelude::*;

mod colliders;
pub mod joints;
mod rigid_bodies;
pub use colliders::{Collider, ColliderBuilder, Shape};
pub use rigid_bodies::{NoRigidBodyError, RigidBody, RigidBodyBuilder};

pub use rapier2d::dynamics::{
    CoefficientCombineRule, ImpulseJointHandle, IntegrationParameters, LockedAxes,
    RigidBodyActivation, RigidBodyType,
};

use super::{Node, Object};

/// Physics stuff.
pub(crate) struct Physics {
    pub rigid_body_set: RigidBodySet,
    pub collider_set: ColliderSet,

    pub gravity: Vector<Real>,
    pub integration_parameters: IntegrationParameters,
    pub island_manager: IslandManager,
    pub broad_phase: BroadPhaseMultiSap,
    pub narrow_phase: NarrowPhase,
    pub impulse_joint_set: ImpulseJointSet,
    pub multibody_joint_set: MultibodyJointSet,
    pub ccd_solver: CCDSolver,

    pub query_pipeline: QueryPipeline,
    pub query_pipeline_out_of_date: bool,
}

impl Default for Physics {
    fn default() -> Self {
        Self::new()
    }
}

impl Physics {
    pub fn new() -> Self {
        Self {
            rigid_body_set: RigidBodySet::new(),
            collider_set: ColliderSet::new(),
            gravity: vector!(0.0, 9.81),
            integration_parameters: IntegrationParameters::default(),
            island_manager: IslandManager::new(),
            broad_phase: BroadPhaseMultiSap::new(),
            narrow_phase: NarrowPhase::new(),
            impulse_joint_set: ImpulseJointSet::new(),
            multibody_joint_set: MultibodyJointSet::new(),
            ccd_solver: CCDSolver::new(),
            query_pipeline: QueryPipeline::new(),
            query_pipeline_out_of_date: false,
        }
    }
    /// Physics iteration.
    pub fn step(&mut self, physics_pipeline: &mut PhysicsPipeline) {
        physics_pipeline.step(
            &self.gravity,
            &self.integration_parameters,
            &mut self.island_manager,
            &mut self.broad_phase,
            &mut self.narrow_phase,
            &mut self.rigid_body_set,
            &mut self.collider_set,
            &mut self.impulse_joint_set,
            &mut self.multibody_joint_set,
            &mut self.ccd_solver,
            None, // Doesn't update that well with the query pipeline in here.
            &(),
            &(),
        );
        // So it updates here.
        self.query_pipeline.update(&self.collider_set);
        self.query_pipeline_out_of_date = false;
    }
    /// Updates the query pipeline if it requires one after someone manually moved a collider.
    pub fn update_query_pipeline(&mut self) {
        if self.query_pipeline_out_of_date {
            self.query_pipeline.update(&self.collider_set);
            self.query_pipeline_out_of_date = false;
        }
    }
    /// Removes a collider.
    pub fn remove_collider(&mut self, handle: ColliderHandle) {
        let colliders = &mut self.collider_set;
        let island_manager = &mut self.island_manager;
        let rigid_body_set = &mut self.rigid_body_set;
        colliders.remove(handle, island_manager, rigid_body_set, true);
    }
    /// Removes a rigid body.
    pub fn remove_rigid_body(&mut self, handle: RigidBodyHandle, remove_colliders: bool) {
        let rigid_bodies = &mut self.rigid_body_set;
        let island_manager = &mut self.island_manager;
        let collider_set = &mut self.collider_set;
        let impulse_joint_set = &mut self.impulse_joint_set;
        let multibody_joint_set = &mut self.multibody_joint_set;
        rigid_bodies.remove(
            handle,
            island_manager,
            collider_set,
            impulse_joint_set,
            multibody_joint_set,
            remove_colliders,
        );
    }
    /// Adds a rigidbody with given collider child.
    pub fn insert_with_parent(
        &mut self,
        collider: rapier2d::geometry::Collider,
        rigid_body_handle: RigidBodyHandle,
    ) -> ColliderHandle {
        self.collider_set
            .insert_with_parent(collider, rigid_body_handle, &mut self.rigid_body_set)
    }
    /// Sets the parent of a specific collider to a new parent.
    pub fn set_parent(
        &mut self,
        handle: ColliderHandle,
        new_parent_handle: Option<RigidBodyHandle>,
    ) {
        self.collider_set
            .set_parent(handle, new_parent_handle, &mut self.rigid_body_set)
    }
}

/// The physics part that every object holds.
///
/// It holds a Arc to the physics part so it can update it with Sync.
///
/// It also holds the collider, it's position, rigid body and all it's handles.
#[derive(Clone, Default)]
pub(crate) struct ObjectPhysics {
    pub collider: Option<colliders::Collider>,
    pub local_collider_position: Vec2,
    pub rigid_body: Option<rigid_bodies::RigidBody>,
    pub collider_handle: Option<ColliderHandle>,
    pub rigid_body_handle: Option<RigidBodyHandle>,
}
impl PartialEq for ObjectPhysics {
    fn eq(&self, other: &Self) -> bool {
        self.local_collider_position == other.local_collider_position
            && self.collider_handle == other.collider_handle
            && self.rigid_body_handle == other.rigid_body_handle
    }
}

impl std::fmt::Debug for ObjectPhysics {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ObjectPhysics")
            .field("collider", &self.collider.is_some())
            .field("rigid body", &self.rigid_body.is_some())
            .field("collider handle", &self.collider_handle.is_some())
            .field("rigid body handle", &self.rigid_body_handle.is_some())
            .finish()
    }
}
impl ObjectPhysics {
    /// Updates the physics part of the objects on Sync.
    pub fn update(
        &mut self,
        transform: &Transform,
        parent: &mut Node<Object>,
        rigid_body_object: &mut crate::objects::RigidBodyParent,
        id: u128,
        physics: &mut Physics,
    ) -> Option<Transform> {
        let parent_transform = parent.object.transform;
        let public_transform = transform.combine(parent_transform);

        physics.query_pipeline_out_of_date = true;

        // What happens in every combination.
        match (
            self.collider.as_mut(),
            self.rigid_body.as_mut(),
            self.collider_handle.as_ref(),
            self.rigid_body_handle.as_ref(),
        ) {
            // Adds a collider to the collider set.
            (Some(collider), None, None, None) => {
                collider.0.set_position(public_transform.into());
                collider.0.user_data = id;
                self.collider_handle = Some(physics.collider_set.insert(collider.0.clone()));
            }
            // Adds a colliderless rigid body to the rigid body set.
            (None, Some(rigid_body), None, None) => {
                rigid_body.0.set_position(public_transform.into(), true);
                rigid_body.0.user_data = id;
                let handle = physics.rigid_body_set.insert(rigid_body.0.clone());
                self.rigid_body_handle = Some(handle);
            }
            // Adds a collider with a rigid body parent to both the collider and rigid body set.
            (Some(collider), Some(rigid_body), None, None) => {
                rigid_body.0.set_position(public_transform.into(), true);
                rigid_body.0.user_data = id;
                let pos = mint::Vector2::from(self.local_collider_position);
                let iso = Isometry2::new(pos.into(), 0.0);
                collider.0.set_position(iso);
                collider.0.user_data = id;
                let rigid_body_handle = physics.rigid_body_set.insert(rigid_body.0.clone());
                self.collider_handle =
                    Some(physics.insert_with_parent(collider.0.clone(), rigid_body_handle));
                self.rigid_body_handle = Some(rigid_body_handle);
            }
            // Removes a collider from the collider set.
            (None, None, Some(collider_handle), None) => {
                physics.remove_collider(*collider_handle);
                self.collider_handle = None;
            }
            // Updates the collider in the collider set.
            (Some(collider), None, Some(collider_handle), None) => {
                collider.0.set_position(public_transform.into());
                let public_collider = physics.collider_set.get_mut(*collider_handle)?;
                *public_collider = collider.0.clone();
            }
            // Adds a colliderless rigid body to the rigid body set and removes a collider from a collider set.
            (None, Some(rigid_body), Some(collider_handle), None) => {
                physics.remove_collider(*collider_handle);
                self.collider_handle = None;

                rigid_body.0.set_position(public_transform.into(), true);
                rigid_body.0.user_data = id;
                self.rigid_body_handle = Some(physics.rigid_body_set.insert(rigid_body.0.clone()));
            }
            // Updates the collider in the collider set to be parentless at the public position and removes the rigid body from it's set.
            (Some(collider), Some(rigid_body), Some(collider_handle), None) => {
                rigid_body.0.set_position(public_transform.into(), true);
                rigid_body.0.user_data = id;
                let rigid_body_handle = Some(physics.rigid_body_set.insert(rigid_body.0.clone()));
                let pos = mint::Vector2::from(self.local_collider_position);
                let iso = Isometry2::new(pos.into(), 0.0);
                collider.0.set_position(iso);
                physics.set_parent(*collider_handle, rigid_body_handle);
                let public_collider = physics.collider_set.get_mut(*collider_handle)?;
                *public_collider = collider.0.clone();
                self.rigid_body_handle = rigid_body_handle;
            }
            // Removes a colliderless rigidbody from the rigid body set.
            (None, None, None, Some(rigid_body_handle)) => {
                physics.remove_rigid_body(*rigid_body_handle, false);
                self.rigid_body_handle = None;
            }
            // Adds a collider to the collider set and removes a colliderless rigid body from the rigid body set.
            (Some(collider), None, None, Some(rigid_body_handle)) => {
                physics.remove_rigid_body(*rigid_body_handle, false);
                self.rigid_body_handle = None;

                collider.0.set_position(public_transform.into());
                collider.0.user_data = id;
                self.collider_handle = Some(physics.collider_set.insert(collider.0.clone()));
            }
            // Updates the colliderless rigid body in it's rigid body set.
            (None, Some(rigid_body), None, Some(rigid_body_handle)) => {
                rigid_body.0.set_position(public_transform.into(), true);
                rigid_body.0.user_data = id;
                let public_body = physics.rigid_body_set.get_mut(*rigid_body_handle)?;
                *public_body = rigid_body.0.clone();
            }
            // Adds the collider to the collider set giving the rigid body a collider and updating it in it's rigid body set.
            (Some(collider), Some(rigid_body), None, Some(rigid_body_handle)) => {
                rigid_body.0.set_position(public_transform.into(), true);
                rigid_body.0.user_data = id;

                let pos = mint::Vector2::from(self.local_collider_position);
                let iso = Isometry2::new(pos.into(), 0.0);
                collider.0.set_position(iso);
                collider.0.user_data = id;
                self.collider_handle =
                    Some(physics.insert_with_parent(collider.0.clone(), *rigid_body_handle));

                let public_body = physics.rigid_body_set.get_mut(*rigid_body_handle)?;
                *public_body = rigid_body.0.clone();
            }
            // Removes both the collider and rigid body from it's sets.
            (None, None, Some(collider_handle), Some(rigid_body_handle)) => {
                physics.remove_rigid_body(*rigid_body_handle, true);
                physics.remove_collider(*collider_handle);
                self.rigid_body_handle = None;
                self.collider_handle = None;
            }
            // Updates the collider in the collider set and removes it's rigid body parent from it's rigid body set.
            (Some(collider), None, Some(collider_handle), Some(rigid_body_handle)) => {
                collider.0.set_position(public_transform.into());
                let public_collider = physics.collider_set.get_mut(*collider_handle)?;
                *public_collider = collider.0.clone();

                physics.remove_rigid_body(*rigid_body_handle, false);
                self.rigid_body_handle = None;
            }
            // Removes the collider from it's collider set leaving the rigid body to be colliderless and updates it.
            (None, Some(rigid_body), Some(collider_handle), Some(rigid_body_handle)) => {
                physics.remove_collider(*collider_handle);
                self.collider_handle = None;

                rigid_body.0.set_position(public_transform.into(), true);
                rigid_body.0.user_data = id;
                let public_body = physics.rigid_body_set.get_mut(*rigid_body_handle)?;
                *public_body = rigid_body.0.clone();
            }
            // Updates everything in it's sets.
            (Some(collider), Some(rigid_body), Some(collider_handle), Some(rigid_body_handle)) => {
                let pos = mint::Vector2::from(self.local_collider_position);
                let iso = Isometry2::new(pos.into(), 0.0);
                collider.0.set_position(iso);
                let public_collider = physics.collider_set.get_mut(*collider_handle)?;
                *public_collider = collider.0.clone();

                rigid_body.0.set_position(public_transform.into(), true);
                rigid_body.0.user_data = id;
                let public_body = physics.rigid_body_set.get_mut(*rigid_body_handle)?;
                *public_body = rigid_body.0.clone();
            }
            _ => (),
        };
        if rigid_body_object.is_none() && self.rigid_body.is_some() {
            *rigid_body_object = Some(None);
        }
        Some(parent_transform)
    }
    /// In case the object gets removed from the layer.
    pub fn remove(&mut self, physics: &Mutex<Physics>) {
        let mut physics = physics.lock();
        physics.query_pipeline_out_of_date = true;
        match (
            self.collider_handle.as_ref(),
            self.rigid_body_handle.as_ref(),
        ) {
            (Some(collider_handle), None) => {
                physics.remove_collider(*collider_handle);
            }
            (None, Some(rigid_body_handle)) => {
                physics.remove_rigid_body(*rigid_body_handle, false);
            }
            (Some(collider_handle), Some(rigid_body_handle)) => {
                physics.remove_rigid_body(*rigid_body_handle, true);
                physics.remove_collider(*collider_handle);
            }
            _ => (),
        }
        self.collider_handle = None;
        self.rigid_body_handle = None;
    }
}

impl From<Transform> for Isometry<Real> {
    fn from(val: Transform) -> Self {
        let pos = mint::Vector2::from(val.position);
        Isometry2::new(pos.into(), val.rotation)
    }
}

impl From<(Vec2, Vec2, f32)> for Transform {
    fn from(val: (Vec2, Vec2, f32)) -> Self {
        Transform {
            position: val.0,
            size: val.1,
            rotation: val.2,
        }
    }
}
