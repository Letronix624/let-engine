use crate::{Data, Transform};
use glam::f32::{vec2, Vec2};
use parking_lot::Mutex;
pub use rapier2d::parry::transformation::vhacd::VHACDParameters;
use rapier2d::prelude::*;
use std::sync::Arc;

pub(crate) type APhysics = Arc<Mutex<Physics>>;

pub use rapier2d::dynamics::{
    IntegrationParameters, LockedAxes, RigidBodyActivation, RigidBodyType,
};

pub(crate) struct Physics {
    pub rigid_body_set: RigidBodySet,
    pub collider_set: ColliderSet,

    pub gravity: Vector<Real>,
    pub integration_parameters: IntegrationParameters,
    pub island_manager: IslandManager,
    pub broad_phase: BroadPhase,
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
            broad_phase: BroadPhase::new(),
            narrow_phase: NarrowPhase::new(),
            impulse_joint_set: ImpulseJointSet::new(),
            multibody_joint_set: MultibodyJointSet::new(),
            ccd_solver: CCDSolver::new(),
            query_pipeline: QueryPipeline::new(),
            query_pipeline_out_of_date: false,
        }
    }
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
            None,
            &(),
            &(),
        );
        self.query_pipeline.update(&self.rigid_body_set, &self.collider_set);
        self.query_pipeline_out_of_date = false;
    }
    pub fn update_query_pipeline(&mut self) {
        if self.query_pipeline_out_of_date {
            self.query_pipeline.update(&self.rigid_body_set, &self.collider_set);
            self.query_pipeline_out_of_date = false;
        }
    }
    pub fn remove_collider(&mut self, handle: ColliderHandle) {
        let colliders = &mut self.collider_set;
        let island_manager = &mut self.island_manager;
        let rigid_body_set = &mut self.rigid_body_set;
        colliders.remove(handle, island_manager, rigid_body_set, true);
    }
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
    pub fn insert_with_parent(
        &mut self,
        collider: rapier2d::geometry::Collider,
        rigid_body_handle: RigidBodyHandle,
    ) -> ColliderHandle {
        self.collider_set
            .insert_with_parent(collider, rigid_body_handle, &mut self.rigid_body_set)
    }
    pub fn set_parent(
        &mut self,
        handle: ColliderHandle,
        new_parent_handle: Option<RigidBodyHandle>,
    ) {
        self.collider_set
            .set_parent(handle, new_parent_handle, &mut self.rigid_body_set)
    }
}

#[derive(Default, Clone)]
pub(crate) struct ObjectPhysics {
    pub physics: Option<APhysics>,
    pub collider: Option<Collider>,
    pub rigid_body: Option<RigidBody>,
    pub collider_handle: Option<ColliderHandle>,
    pub rigid_body_handle: Option<RigidBodyHandle>,
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
    pub fn update(&mut self, public_transform: Transform, id: u128) {
        let mut physics = self.physics.as_ref().unwrap().lock();
        physics.query_pipeline_out_of_date = true;
        match (
            self.collider.as_mut(),
            self.rigid_body.as_mut(),
            self.collider_handle.as_ref(),
            self.rigid_body_handle.as_ref(),
        ) {
            (Some(collider), None, None, None) => {
                collider.0.set_position(public_transform.into());
                collider.0.user_data = id;
                self.collider_handle = Some(
                    physics.collider_set.insert(collider.0.clone())
                );
            }
            (None, Some(rigid_body), None, None) => {
                rigid_body.0.set_position(public_transform.into(), true);
                rigid_body.0.user_data = id;
                self.rigid_body_handle = Some(physics.rigid_body_set.insert(rigid_body.0.clone()));
            }
            (Some(collider), Some(rigid_body), None, None) => {
                rigid_body.0.set_position(public_transform.into(), true);
                rigid_body.0.user_data = id;
                collider.0.set_position(vec2(0.0, 0.0).into()); //please make this somehow editable by the user in the future at some point. Goal right now is to make it work.
                collider.0.user_data = id;
                let rigid_body_handle = physics.rigid_body_set.insert(rigid_body.0.clone());
                self.collider_handle =
                    Some(physics.insert_with_parent(collider.0.clone(), rigid_body_handle));
                self.rigid_body_handle = Some(rigid_body_handle);
            }
            (None, None, Some(collider_handle), None) => {
                physics.remove_collider(*collider_handle);
                self.collider_handle = None;
            }
            (Some(collider), None, Some(collider_handle), None) => {
                collider.0.set_position(public_transform.into());
                let public_collider = physics.collider_set.get_mut(*collider_handle).unwrap();
                *public_collider = collider.0.clone();
            }
            (None, Some(rigid_body), Some(collider_handle), None) => {
                physics.remove_collider(*collider_handle);
                self.collider_handle = None;

                rigid_body.0.set_position(public_transform.into(), true);
                rigid_body.0.user_data = id;
                self.rigid_body_handle = Some(physics.rigid_body_set.insert(rigid_body.0.clone()));
            }
            (Some(collider), Some(rigid_body), Some(collider_handle), None) => {
                rigid_body.0.set_position(public_transform.into(), true);
                rigid_body.0.user_data = id;
                let rigid_body_handle = Some(physics.rigid_body_set.insert(rigid_body.0.clone()));
                collider.0.set_position(vec2(0.0, 0.0).into());
                physics.set_parent(*collider_handle, rigid_body_handle);
                let public_collider = physics.collider_set.get_mut(*collider_handle).unwrap();
                *public_collider = collider.0.clone();
                self.rigid_body_handle = rigid_body_handle;
            }
            (None, None, None, Some(rigid_body_handle)) => {
                physics.remove_rigid_body(*rigid_body_handle, false);
                self.rigid_body_handle = None;
            }
            (Some(collider), None, None, Some(rigid_body_handle)) => {
                physics.remove_rigid_body(*rigid_body_handle, false);
                self.rigid_body_handle = None;

                collider.0.set_position(public_transform.into());
                collider.0.user_data = id;
                self.collider_handle = Some(physics.collider_set.insert(collider.0.clone()));
            }
            (None, Some(rigid_body), None, Some(rigid_body_handle)) => {
                rigid_body.0.set_position(public_transform.into(), true);
                rigid_body.0.user_data = id;
                let public_body = physics.rigid_body_set.get_mut(*rigid_body_handle).unwrap();
                *public_body = rigid_body.0.clone();
            }
            (Some(collider), Some(rigid_body), None, Some(rigid_body_handle)) => {
                rigid_body.0.set_position(public_transform.into(), true);
                rigid_body.0.user_data = id;

                collider.0.set_position(vec2(0.0, 0.0).into()); //please make this somehow editable by the user in the future at some point. Goal right now is to make it work.
                collider.0.user_data = id;
                self.collider_handle =
                    Some(physics.insert_with_parent(collider.0.clone(), *rigid_body_handle));

                let public_body = physics.rigid_body_set.get_mut(*rigid_body_handle).unwrap();
                *public_body = rigid_body.0.clone();
            }
            (None, None, Some(collider_handle), Some(rigid_body_handle)) => {
                physics.remove_rigid_body(*rigid_body_handle, true);
                physics.remove_collider(*collider_handle);
                self.rigid_body_handle = None;
                self.collider_handle = None;
            }
            (Some(collider), None, Some(collider_handle), Some(rigid_body_handle)) => {
                collider.0.set_position(public_transform.into());
                let public_collider = physics.collider_set.get_mut(*collider_handle).unwrap();
                *public_collider = collider.0.clone();

                physics.remove_rigid_body(*rigid_body_handle, false);
                self.rigid_body_handle = None;
            }
            (None, Some(rigid_body), Some(collider_handle), Some(rigid_body_handle)) => {
                physics.remove_collider(*collider_handle);
                self.collider_handle = None;

                rigid_body.0.set_position(public_transform.into(), true);
                rigid_body.0.user_data = id;
                let public_body = physics.rigid_body_set.get_mut(*rigid_body_handle).unwrap();
                *public_body = rigid_body.0.clone();
            }
            (Some(collider), Some(rigid_body), Some(collider_handle), Some(rigid_body_handle)) => {
                collider.0.set_position(vec2(0.0, 0.0).into());
                let public_collider = physics.collider_set.get_mut(*collider_handle).unwrap();
                *public_collider = collider.0.clone();

                rigid_body.0.set_position(public_transform.into(), true);
                rigid_body.0.user_data = id;
                let public_body = physics.rigid_body_set.get_mut(*rigid_body_handle).unwrap();
                *public_body = rigid_body.0.clone();
            }
            _ => (),
        }
    }
    pub fn remove(&mut self) {

        self.collider = None;
        self.rigid_body = None;

        Self::update(self, Transform::default(), 0);
    }
}

#[derive(Clone)]
pub struct Collider(pub rapier2d::geometry::Collider);

impl Collider {
    /// Is this collider a sensor?
    pub fn is_sensor(&self) -> bool {
        self.0.is_sensor()
    }

    /// Sets whether or not this is a sensor collider.
    pub fn set_sensor(&mut self, is_sensor: bool) {
        self.0.set_sensor(is_sensor)
    }

    /// The friction coefficient of this collider.
    pub fn friction(&self) -> Real {
        self.0.friction()
    }

    /// Sets the friction coefficient of this collider.
    pub fn set_friction(&mut self, coefficient: Real) {
        self.0.set_friction(coefficient)
    }

    /// The combine rule used by this collider to combine its friction
    /// coefficient with the friction coefficient of the other collider it
    /// is in contact with.
    pub fn friction_combine_rule(&self) -> CoefficientCombineRule {
        self.0.friction_combine_rule()
    }

    /// Sets the combine rule used by this collider to combine its friction
    /// coefficient with the friction coefficient of the other collider it
    /// is in contact with.
    pub fn set_friction_combine_rule(&mut self, rule: CoefficientCombineRule) {
        self.0.set_friction_combine_rule(rule)
    }

    /// The restitution coefficient of this collider.
    pub fn restitution(&self) -> Real {
        self.0.restitution()
    }

    /// Sets the restitution coefficient of this collider.
    pub fn set_restitution(&mut self, coefficient: Real) {
        self.0.set_restitution(coefficient)
    }

    /// The combine rule used by this collider to combine its restitution
    /// coefficient with the restitution coefficient of the other collider it
    /// is in contact with.
    pub fn restitution_combine_rule(&self) -> CoefficientCombineRule {
        self.0.restitution_combine_rule()
    }

    /// Sets the combine rule used by this collider to combine its restitution
    /// coefficient with the restitution coefficient of the other collider it
    /// is in contact with.
    pub fn set_restitution_combine_rule(&mut self, rule: CoefficientCombineRule) {
        self.0.set_restitution_combine_rule(rule)
    }

    /// Sets the total force magnitude beyond which a contact force event can be emitted.
    pub fn set_contact_force_event_threshold(&mut self, threshold: Real) {
        self.0.set_contact_force_event_threshold(threshold)
    }

    /// Is this collider enabled?
    pub fn is_enabled(&self) -> bool {
        self.0.is_enabled()
    }

    /// Sets whether or not this collider is enabled.
    pub fn set_enabled(&mut self, enabled: bool) {
        self.0.set_enabled(enabled)
    }

    /// The volume (or surface in 2D) of this collider.
    pub fn volume(&self) -> Real {
        self.0.volume()
    }

    /// The density of this collider.
    pub fn density(&self) -> Real {
        self.0.density()
    }

    /// The mass of this collider.
    pub fn mass(&self) -> Real {
        self.0.mass()
    }

    /// Sets the uniform density of this collider.
    pub fn set_density(&mut self, density: Real) {
        self.0.set_density(density)
    }

    /// Sets the mass of this collider.
    pub fn set_mass(&mut self, mass: Real) {
        self.0.set_mass(mass)
    }

    /// The total force magnitude beyond which a contact force event can be emitted.
    pub fn contact_force_event_threshold(&self) -> Real {
        self.0.contact_force_event_threshold()
    }
}

pub struct ColliderBuilder {
    pub shape: Shape,
    pub mass_properties: ColliderMassProps,
    pub friction: Real,
    pub friction_combine_rule: CoefficientCombineRule,
    pub restitution: Real,
    pub restitution_combine_rule: CoefficientCombineRule,
    pub transform: Transform,
    pub is_sensor: bool,
    pub active_collision_types: ActiveCollisionTypes,
    pub active_hooks: ActiveHooks,
    pub active_events: ActiveEvents,
    pub collision_groups: InteractionGroups,
    pub solver_groups: InteractionGroups,
    pub enabled: bool,
    pub contact_force_event_threshold: Real,
}

impl From<Transform> for Isometry<Real> {
    fn from(val: Transform) -> Self {
        (val.position, val.rotation).into()
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

impl ColliderBuilder {
    /// Initialize a new collider builder with the given shape.
    pub fn new(shape: Shape) -> Self {
        Self {
            shape,
            mass_properties: ColliderMassProps::default(),
            friction: 0.5,
            friction_combine_rule: CoefficientCombineRule::Average,
            restitution: 0.0,
            restitution_combine_rule: CoefficientCombineRule::Average,
            transform: Transform::default(),
            is_sensor: false,
            active_collision_types: ActiveCollisionTypes::default(),
            active_hooks: ActiveHooks::empty(),
            active_events: ActiveEvents::empty(),
            collision_groups: InteractionGroups::all(),
            solver_groups: InteractionGroups::all(),
            enabled: true,
            contact_force_event_threshold: 0.0,
        }
    }

    pub fn build(self) -> Collider {
        Collider(
            rapier2d::geometry::ColliderBuilder {
                shape: self.shape.0,
                mass_properties: self.mass_properties,
                friction: self.friction,
                friction_combine_rule: self.friction_combine_rule,
                restitution: self.restitution,
                restitution_combine_rule: self.restitution_combine_rule,
                is_sensor: self.is_sensor,
                active_collision_types: self.active_collision_types,
                active_hooks: self.active_hooks,
                active_events: self.active_events,
                collision_groups: self.collision_groups,
                solver_groups: self.solver_groups,
                enabled: self.enabled,
                contact_force_event_threshold: self.contact_force_event_threshold,
                user_data: 0,
                position: self.transform.into(),
            }
            .build(),
        )
    }

    /// Initialize a new collider builder with a compound shape.
    pub fn compound(shapes: Vec<(Transform, Shape)>) -> Self {
        Self::new(Shape::compound(shapes))
    }

    /// Initialize a new collider builder with a circle shape defined by its radius.
    pub fn circle(radius: Real) -> Self {
        Self::new(Shape::circle(radius))
    }

    /// Initialize a new collider builder with a cuboid shape defined by its half-extents.
    pub fn square(hx: Real, hy: Real) -> Self {
        Self::new(Shape::square(hx, hy))
    }

    /// Initialize a new collider builder with a round cuboid shape defined by its half-extents
    /// and border radius.
    pub fn rounded_square(hx: Real, hy: Real, border_radius: Real) -> Self {
        Self::new(Shape::rounded_square(hx, hy, border_radius))
    }

    /// Initialize a capsule collider from its endpoints and radius.
    pub fn capsule(a: Vec2, b: Vec2, radius: Real) -> Self {
        Self::new(Shape::capsule(a, b, radius))
    }

    /// Initialize a new collider builder with a capsule shape aligned with the `x` axis.
    pub fn capsule_x(half_height: Real, radius: Real) -> Self {
        Self::new(Shape::capsule_x(half_height, radius))
    }

    /// Initialize a new collider builder with a capsule shape aligned with the `y` axis.
    pub fn capsule_y(half_height: Real, radius: Real) -> Self {
        Self::new(Shape::capsule_x(half_height, radius))
    }

    /// Initializes a collider builder with a segment shape.
    pub fn segment(a: Vec2, b: Vec2) -> Self {
        Self::new(Shape::segment(a, b))
    }

    /// Initializes a collider builder with a triangle shape.
    pub fn triangle(a: Vec2, b: Vec2, c: Vec2) -> Self {
        Self::new(Shape::triangle(a, b, c))
    }

    /// Initializes a collider builder with a triangle shape with round corners.
    pub fn round_triangle(a: Vec2, b: Vec2, c: Vec2, radius: Real) -> Self {
        Self::new(Shape::round_triangle(a, b, c, radius))
    }

    /// Initializes a collider builder with a polyline shape defined by its vertex and index buffers.
    pub fn polyline(vertices: Vec<Vec2>, indices: Option<Vec<[u32; 2]>>) -> Self {
        Self::new(Shape::polyline(vertices, indices))
    }

    /// Initializes a collider builder with a triangle mesh shape defined by its vertex and index buffers.
    pub fn trimesh(data: Data) -> Self {
        Self::new(Shape::trimesh(data))
    }

    /// Initializes a collider builder with a compound shape obtained from the decomposition of
    /// the given trimesh (in 3D) or polyline (in 2D) into convex parts.
    pub fn convex_decomposition(vertices: &[Vec2], indices: &[[u32; 2]]) -> Self {
        Self::new(Shape::convex_decomposition(vertices, indices))
    }

    /// Initializes a collider builder with a compound shape obtained from the decomposition of
    /// the given trimesh (in 3D) or polyline (in 2D) into convex parts dilated with round corners.
    pub fn round_convex_decomposition(
        vertices: &[Vec2],
        indices: &[[u32; 2]],
        radius: Real,
    ) -> Self {
        Self::new(Shape::round_convex_decomposition(vertices, indices, radius))
    }

    /// Initializes a collider builder with a compound shape obtained from the decomposition of
    /// the given trimesh (in 3D) or polyline (in 2D) into convex parts.
    pub fn convex_decomposition_with_params(
        vertices: &[Vec2],
        indices: &[[u32; 2]],
        params: &VHACDParameters,
    ) -> Self {
        Self::new(Shape::convex_decomposition_with_params(
            vertices, indices, params,
        ))
    }

    /// Initializes a collider builder with a compound shape obtained from the decomposition of
    /// the given trimesh (in 3D) or polyline (in 2D) into convex parts dilated with round corners.
    pub fn round_convex_decomposition_with_params(
        vertices: &[Vec2],
        indices: &[[u32; 2]],
        params: &VHACDParameters,
        radius: Real,
    ) -> Self {
        Self::new(Shape::round_convex_decomposition_with_params(
            vertices, indices, params, radius,
        ))
    }

    /// Initializes a new collider builder with a 2D convex polygon or 3D convex polyhedron
    /// obtained after computing the convex-hull of the given points.
    pub fn convex_hull(points: &[Vec2]) -> Option<Self> {
        let shape = Shape::convex_hull(points);
        shape.map(Self::new)
    }

    /// Initializes a new collider builder with a round 2D convex polygon or 3D convex polyhedron
    /// obtained after computing the convex-hull of the given points. The shape is dilated
    /// by a sphere of radius `border_radius`.
    pub fn round_convex_hull(points: &[Vec2], border_radius: Real) -> Option<Self> {
        let shape = Shape::round_convex_hull(points, border_radius);
        shape.map(Self::new)
    }

    /// Creates a new collider builder that is a convex polygon formed by the
    /// given polyline assumed to be convex (no convex-hull will be automatically
    /// computed).
    pub fn convex_polyline(points: &[Vec2]) -> Option<Self> {
        let shape = Shape::convex_polyline(points);
        shape.map(Self::new)
    }

    /// Creates a new collider builder that is a round convex polygon formed by the
    /// given polyline assumed to be convex (no convex-hull will be automatically
    /// computed). The polygon shape is dilated by a sphere of radius `border_radius`.
    pub fn round_convex_polyline(points: Vec<Vec2>, border_radius: Real) -> Option<Self> {
        Shape::round_convex_polyline(points, border_radius).map(Self::new)
    }

    /// Initializes a collider builder with a heightfield shape defined by its set of height and a scale
    /// factor along each coordinate axis.
    pub fn heightfield(heights: Vec<Real>, scale: Vec2) -> Self {
        Self::new(Shape::heightfield(heights, scale))
    }

    /// Make a collider from a Rapier or Parry shape.
    pub fn from_shared_shape(shape: SharedShape) -> Self {
        Self::new(Shape::from_shared_shape(shape))
    }

    /// Sets whether or not the collider built by this builder is a sensor.
    pub fn sensor(mut self, is_sensor: bool) -> Self {
        self.is_sensor = is_sensor;
        self
    } 

    /// Sets the friction coefficient of the collider this builder will build.
    pub fn friction(mut self, friction: Real) -> Self {
        self.friction = friction;
        self
    }

    /// Sets the rule to be used to combine two friction coefficients in a contact.
    pub fn friction_combine_rule(mut self, rule: CoefficientCombineRule) -> Self {
        self.friction_combine_rule = rule;
        self
    }

    /// Sets the restitution coefficient of the collider this builder will build.
    pub fn restitution(mut self, restitution: Real) -> Self {
        self.restitution = restitution;
        self
    }

    /// Sets the rule to be used to combine two restitution coefficients in a contact.
    pub fn restitution_combine_rule(mut self, rule: CoefficientCombineRule) -> Self {
        self.restitution_combine_rule = rule;
        self
    }

    /// Sets the uniform density of the collider this builder will build.
    ///
    /// This will be overridden by a call to [`Self::mass`] or [`Self::mass_properties`] so it only
    /// makes sense to call either [`Self::density`] or [`Self::mass`] or [`Self::mass_properties`].
    ///
    /// The mass and angular inertia of this collider will be computed automatically based on its
    /// shape.
    pub fn density(mut self, density: Real) -> Self {
        self.mass_properties = ColliderMassProps::Density(density);
        self
    }

    /// Sets the mass of the collider this builder will build.
    ///
    /// This will be overridden by a call to [`Self::density`] or [`Self::mass_properties`] so it only
    /// makes sense to call either [`Self::density`] or [`Self::mass`] or [`Self::mass_properties`].
    ///
    /// The angular inertia of this collider will be computed automatically based on its shape
    /// and this mass value.
    pub fn mass(mut self, mass: Real) -> Self {
        self.mass_properties = ColliderMassProps::Mass(mass);
        self
    }

    /// Enable or disable the collider after its creation.
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }


}

#[derive(Clone, Default, Debug)]
pub struct RigidBody(pub(crate) rapier2d::dynamics::RigidBody);

impl RigidBody {
    /// The activation status of this rigid-body.
    pub fn activation(&self) -> &RigidBodyActivation {
        self.0.activation()
    }

    /// Mutable reference to the activation status of this rigid-body.
    pub fn activation_mut(&mut self) -> &mut RigidBodyActivation {
        self.0.activation_mut()
    }

    /// Is this rigid-body enabled?
    pub fn is_enabled(&self) -> bool {
        self.0.is_enabled()
    }

    /// Sets whether this rigid-body is enabled or not.
    pub fn set_enabled(&mut self, enabled: bool) {
        self.0.set_enabled(enabled)
    }
    /// The linear damping coefficient of this rigid-body.
    #[inline]
    pub fn linear_damping(&self) -> Real {
        self.0.linear_damping()
    }

    /// Sets the linear damping coefficient of this rigid-body.
    #[inline]
    pub fn set_linear_damping(&mut self, damping: Real) {
        self.0.set_linear_damping(damping)
    }

    /// The angular damping coefficient of this rigid-body.
    #[inline]
    pub fn angular_damping(&self) -> Real {
        self.0.angular_damping()
    }

    /// Sets the angular damping coefficient of this rigid-body.
    #[inline]
    pub fn set_angular_damping(&mut self, damping: Real) {
        self.0.set_angular_damping(damping)
    }

    /// The type of this rigid-body.
    pub fn body_type(&self) -> RigidBodyType {
        self.0.body_type()
    }

    /// Sets the type of this rigid-body.
    pub fn set_body_type(&mut self, status: RigidBodyType, wake_up: bool) {
        self.0.set_body_type(status, wake_up)
    }

    /// The world-space center-of-mass of this rigid-body.
    #[inline]
    pub fn center_of_mass(&self) -> Vec2 {
        (*self.0.center_of_mass()).into()
    }

    /// The mass-properties of this rigid-body.
    #[inline]
    pub fn mass_properties(&self) -> &RigidBodyMassProps {
        self.0.mass_properties()
    }

    /// The dominance group of this rigid-body.
    ///
    /// This method always returns `i8::MAX + 1` for non-dynamic
    /// rigid-bodies.
    #[inline]
    pub fn effective_dominance_group(&self) -> i16 {
        self.0.effective_dominance_group()
    }

    /// The axes along which this rigid-body cannot translate or rotate.
    #[inline]
    pub fn locked_axes(&self) -> LockedAxes {
        self.0.locked_axes()
    }

    /// Sets the axes along which this rigid-body cannot translate or rotate.
    #[inline]
    pub fn set_locked_axes(&mut self, locked_axes: LockedAxes, wake_up: bool) {
        self.0.set_locked_axes(locked_axes, wake_up)
    }

    #[inline]
    /// Locks or unlocks all the rotations of this rigid-body.
    pub fn lock_rotations(&mut self, locked: bool, wake_up: bool) {
        self.0.lock_rotations(locked, wake_up)
    }

    #[inline]
    /// Locks or unlocks all the rotations of this rigid-body.
    pub fn lock_translations(&mut self, locked: bool, wake_up: bool) {
        self.0.lock_translations(locked, wake_up)
    }

    #[inline]
    /// Locks or unlocks translation of this rigid-body along each cartesian axes.
    pub fn set_enabled_translations(
        &mut self,
        allow_translation_x: bool,
        allow_translation_y: bool,
        wake_up: bool,
    ) {
        self.0
            .set_enabled_translations(allow_translation_x, allow_translation_y, wake_up)
    }

    /// Are the translations of this rigid-body locked?
    pub fn is_translation_locked(&self) -> bool {
        self.0.is_translation_locked()
    }

    /// Is the rotation of this rigid-body locked?
    pub fn is_rotation_locked(&self) -> bool {
        self.0.is_rotation_locked()
    }

    /// Enables of disable CCD (continuous collision-detection) for this rigid-body.
    ///
    /// CCD prevents tunneling, but may still allow limited interpenetration of colliders.
    pub fn enable_ccd(&mut self, enabled: bool) {
        self.0.enable_ccd(enabled)
    }

    /// Is CCD (continous collision-detection) enabled for this rigid-body?
    pub fn is_ccd_enabled(&self) -> bool {
        self.0.is_ccd_enabled()
    }

    // This is different from `is_ccd_enabled`. This checks that CCD
    // is active for this rigid-body, i.e., if it was seen to move fast
    // enough to justify a CCD run.
    /// Is CCD active for this rigid-body?
    ///
    /// The CCD is considered active if the rigid-body is moving at
    /// a velocity greater than an automatically-computed threshold.
    ///
    /// This is not the same as `self.is_ccd_enabled` which only
    /// checks if CCD is enabled to run for this rigid-body or if
    /// it is completely disabled (independently from its velocity).
    pub fn is_ccd_active(&self) -> bool {
        self.0.is_ccd_active()
    }

    /// Is this rigid body dynamic?
    ///
    /// A dynamic body can move freely and is affected by forces.
    pub fn is_dynamic(&self) -> bool {
        self.0.is_dynamic()
    }

    /// Is this rigid body kinematic?
    ///
    /// A kinematic body can move freely but is not affected by forces.
    pub fn is_kinematic(&self) -> bool {
        self.0.is_kinematic()
    }

    /// Is this rigid body fixed?
    ///
    /// A fixed body cannot move and is not affected by forces.
    pub fn is_fixed(&self) -> bool {
        self.0.is_fixed()
    }

    /// The mass of this rigid body.
    ///
    /// Returns zero if this rigid body has an infinite mass.
    pub fn mass(&self) -> Real {
        self.0.mass()
    }

    /// The predicted position of this rigid-body.
    ///
    /// If this rigid-body is kinematic this value is set by the `set_next_kinematic_position`
    /// method and is used for estimating the kinematic body velocity at the next timestep.
    /// For non-kinematic bodies, this value is currently unspecified.
    pub fn next_position(&self) -> (Vec2, f32) {
        (*self.0.next_position()).into()
    }

    /// The scale factor applied to the gravity affecting this rigid-body.
    pub fn gravity_scale(&self) -> Real {
        self.0.gravity_scale()
    }

    /// Sets the gravity scale facter for this rigid-body.
    pub fn set_gravity_scale(&mut self, scale: Real, wake_up: bool) {
        self.0.set_gravity_scale(scale, wake_up)
    }

    /// The dominance group of this rigid-body.
    pub fn dominance_group(&self) -> i8 {
        self.0.dominance_group()
    }

    /// The dominance group of this rigid-body.
    pub fn set_dominance_group(&mut self, dominance: i8) {
        self.0.set_dominance_group(dominance)
    }

    /// Put this rigid body to sleep.
    ///
    /// A sleeping body no longer moves and is no longer simulated by the physics engine unless
    /// it is waken up. It can be woken manually with `self.wake_up` or automatically due to
    /// external forces like contacts.
    pub fn sleep(&mut self) {
        self.0.sleep()
    }

    /// Wakes up this rigid body if it is sleeping.
    ///
    /// If `strong` is `true` then it is assured that the rigid-body will
    /// remain awake during multiple subsequent timesteps.
    pub fn wake_up(&mut self, strong: bool) {
        self.0.wake_up(strong)
    }

    /// Is this rigid body sleeping?
    pub fn is_sleeping(&self) -> bool {
        self.0.is_sleeping()
    }

    /// Is the velocity of this body not zero?
    pub fn is_moving(&self) -> bool {
        self.0.is_moving()
    }

    /// The linear velocity of this rigid-body.
    pub fn linvel(&self) -> Vec2 {
        (*self.0.linvel()).into()
    }

    /// The angular velocity of this rigid-body.
    pub fn angvel(&self) -> Real {
        self.0.angvel()
    }

    pub fn set_linvel(&mut self, linvel: Vec2, wake_up: bool) {
        self.0.set_linvel(linvel.into(), wake_up)
    }

    /// The angular velocity of this rigid-body.
    ///
    /// If `wake_up` is `true` then the rigid-body will be woken up if it was
    /// put to sleep because it did not move for a while.
    pub fn set_angvel(&mut self, angvel: Real, wake_up: bool) {
        self.0.set_angvel(angvel, wake_up)
    }
    
    /// If this rigid body is kinematic, sets its future translation after the next timestep integration.
    pub fn set_next_kinematic_rotation(&mut self, rotation: Real) {
        if self.is_kinematic() {
            self.0.set_next_kinematic_rotation(Rotation::from_angle(rotation));
        }
    }

    /// If this rigid body is kinematic, sets its future translation after the next timestep integration.
    pub fn set_next_kinematic_translation(&mut self, translation: Vec2) {
        self.0.set_next_kinematic_translation(translation.into())
    }

    /// If this rigid body is kinematic, sets its future position after the next timestep integration.
    pub fn set_next_kinematic_position(&mut self, pos: (Vec2, f32)) {
        self.0.set_next_kinematic_position(pos.into())
    }

    /// Predicts the next position of this rigid-body, by integrating its velocity and forces
    /// by a time of `dt`.
    pub fn predict_position_using_velocity_and_forces(&self, dt: Real) -> (Vec2, f32) {
        self.0.predict_position_using_velocity_and_forces(dt).into()
    }
}

impl RigidBody {
    /// The velocity of the given world-space point on this rigid-body.
    pub fn velocity_at_point(&self, point: Vec2) -> Vec2 {
        let pos = *self.0.velocity_at_point(&point.into());
        (pos.x, pos.y).into()
    }

    /// The kinetic energy of this body.
    pub fn kinetic_energy(&self) -> Real {
        self.0.kinetic_energy()
    }

    /// The potential energy of this body in a gravity field.
    pub fn gravitational_potential_energy(&self, dt: Real, gravity: Vec2) -> Real {
        self.0.gravitational_potential_energy(dt, gravity.into())
    }
}

/// ## Applying forces and torques
impl RigidBody {
    /// Resets to zero all the constant (linear) forces manually applied to this rigid-body.
    pub fn reset_forces(&mut self, wake_up: bool) {
        self.0.reset_forces(wake_up)
    }

    /// Resets to zero all the constant torques manually applied to this rigid-body.
    pub fn reset_torques(&mut self, wake_up: bool) {
        self.0.reset_torques(wake_up)
    }

    /// Adds to this rigid-body a constant force applied at its center-of-mass.ç
    ///
    /// This does nothing on non-dynamic bodies.
    pub fn add_force(&mut self, force: Vec2, wake_up: bool) {
        self.0.add_force(force.into(), wake_up)
    }

    /// Adds to this rigid-body a constant torque at its center-of-mass.
    ///
    /// This does nothing on non-dynamic bodies.
    pub fn add_torque(&mut self, torque: Real, wake_up: bool) {
        self.0.add_torque(torque, wake_up)
    }

    /// Adds to this rigid-body a constant force at the given world-space point of this rigid-body.
    ///
    /// This does nothing on non-dynamic bodies.
    pub fn add_force_at_point(&mut self, force: Vec2, point: Vec2, wake_up: bool) {
        self.0
            .add_force_at_point(force.into(), point.into(), wake_up)
    }
}

/// ## Applying impulses and angular impulses
impl RigidBody {
    /// Applies an impulse at the center-of-mass of this rigid-body.
    /// The impulse is applied right away, changing the linear velocity.
    /// This does nothing on non-dynamic bodies.
    pub fn apply_impulse(&mut self, impulse: Vec2, wake_up: bool) {
        self.0.apply_impulse(impulse.into(), wake_up)
    }

    /// Applies an angular impulse at the center-of-mass of this rigid-body.
    /// The impulse is applied right away, changing the angular velocity.
    /// This does nothing on non-dynamic bodies.
    pub fn apply_torque_impulse(&mut self, torque_impulse: Real, wake_up: bool) {
        self.0.apply_torque_impulse(torque_impulse, wake_up)
    }

    /// Applies an impulse at the given world-space point of this rigid-body.
    /// The impulse is applied right away, changing the linear and/or angular velocities.
    /// This does nothing on non-dynamic bodies.
    pub fn apply_impulse_at_point(&mut self, impulse: Vec2, point: Vec2, wake_up: bool) {
        self.0
            .apply_impulse_at_point(impulse.into(), point.into(), wake_up)
    }

    /// Retrieves the constant force(s) that the user has added to the body.
    ///
    /// Returns zero if the rigid-body isn’t dynamic.
    pub fn user_force(&self) -> Vec2 {
        self.0.user_force().into()
    }
}

/// A builder for rigid-bodies.
#[derive(Clone, Debug, PartialEq)]
#[must_use = "Builder functions return the updated builder"]
pub struct RigidBodyBuilder {
    /// The linear velocity of the rigid-body to be built.
    pub linvel: Vec2,
    /// The angular velocity of the rigid-body to be built.
    pub angvel: Real,
    /// The scale factor applied to the gravity affecting the rigid-body to be built, `1.0` by default.
    pub gravity_scale: Real,
    /// Damping factor for gradually slowing down the translational motion of the rigid-body, `0.0` by default.
    pub linear_damping: Real,
    /// Damping factor for gradually slowing down the angular motion of the rigid-body, `0.0` by default.
    pub angular_damping: Real,
    body_type: RigidBodyType,
    mprops_flags: LockedAxes,
    /// Whether or not the rigid-body to be created can sleep if it reaches a dynamic equilibrium.
    pub can_sleep: bool,
    /// Whether or not the rigid-body is to be created asleep.
    pub sleeping: bool,
    /// Whether continuous collision-detection is enabled for the rigid-body to be built.
    ///
    /// CCD prevents tunneling, but may still allow limited interpenetration of colliders.
    pub ccd_enabled: bool,
    /// The dominance group of the rigid-body to be built.
    pub dominance_group: i8,
    /// Will the rigid-body being built be enabled?
    pub enabled: bool,
}

impl RigidBodyBuilder {
    /// Initialize a new builder for a rigid body which is either fixed, dynamic, or kinematic.
    pub fn new(body_type: RigidBodyType) -> Self {
        Self {
            linvel: Vec2::default(),
            angvel: 0.0,
            gravity_scale: 1.0,
            linear_damping: 0.0,
            angular_damping: 0.0,
            body_type,
            mprops_flags: LockedAxes::empty(),
            can_sleep: true,
            sleeping: false,
            ccd_enabled: false,
            dominance_group: 0,
            enabled: true,
        }
    }
    /// Initializes the builder of a new fixed rigid body.
    pub fn fixed() -> Self {
        Self::new(RigidBodyType::Fixed)
    }

    /// Initializes the builder of a new velocity-based kinematic rigid body.
    pub fn kinematic_velocity_based() -> Self {
        Self::new(RigidBodyType::KinematicVelocityBased)
    }

    /// Initializes the builder of a new position-based kinematic rigid body.
    pub fn kinematic_position_based() -> Self {
        Self::new(RigidBodyType::KinematicPositionBased)
    }

    /// Initializes the builder of a new dynamic rigid body.
    pub fn dynamic() -> Self {
        Self::new(RigidBodyType::Dynamic)
    }

    /// Sets the scale applied to the gravity force affecting the rigid-body to be created.
    pub fn gravity_scale(mut self, scale_factor: Real) -> Self {
        self.gravity_scale = scale_factor;
        self
    }

    /// Sets the dominance group of this rigid-body.
    pub fn dominance_group(mut self, group: i8) -> Self {
        self.dominance_group = group;
        self
    }

    /// Sets the axes along which this rigid-body cannot translate or rotate.
    pub fn locked_axes(mut self, locked_axes: LockedAxes) -> Self {
        self.mprops_flags = locked_axes;
        self
    }

    /// Prevents this rigid-body from translating because of forces.
    pub fn lock_translations(mut self) -> Self {
        self.mprops_flags.set(LockedAxes::TRANSLATION_LOCKED, true);
        self
    }

    /// Only allow translations of this rigid-body around specific coordinate axes.
    pub fn enabled_translations(
        mut self,
        allow_translations_x: bool,
        allow_translations_y: bool,
    ) -> Self {
        self.mprops_flags
            .set(LockedAxes::TRANSLATION_LOCKED_X, !allow_translations_x);
        self.mprops_flags
            .set(LockedAxes::TRANSLATION_LOCKED_Y, !allow_translations_y);
        self
    }

    /// Prevents this rigid-body from rotating because of forces.
    pub fn lock_rotations(mut self) -> Self {
        self.mprops_flags.set(LockedAxes::ROTATION_LOCKED_X, true);
        self.mprops_flags.set(LockedAxes::ROTATION_LOCKED_Y, true);
        self.mprops_flags.set(LockedAxes::ROTATION_LOCKED_Z, true);
        self
    }

    /// Sets the damping factor for the linear part of the rigid-body motion.
    ///
    /// The higher the linear damping factor is, the more quickly the rigid-body
    /// will slow-down its translational movement.
    pub fn linear_damping(mut self, factor: Real) -> Self {
        self.linear_damping = factor;
        self
    }

    /// Sets the damping factor for the angular part of the rigid-body motion.
    ///
    /// The higher the angular damping factor is, the more quickly the rigid-body
    /// will slow-down its rotational movement.
    pub fn angular_damping(mut self, factor: Real) -> Self {
        self.angular_damping = factor;
        self
    }

    /// Sets the initial linear velocity of the rigid-body to be created.
    pub fn linvel(mut self, linvel: Vec2) -> Self {
        self.linvel = linvel;
        self
    }
    
    /// Sets the initial angular velocity of the rigid-body to be created.
    pub fn angvel(mut self, angvel: Real) -> Self {
        self.angvel = angvel;
        self
    }

    /// Sets whether or not the rigid-body to be created can sleep if it reaches a dynamic equilibrium.
    pub fn can_sleep(mut self, can_sleep: bool) -> Self {
        self.can_sleep = can_sleep;
        self
    }

    /// Sets whether or not continuous collision-detection is enabled for this rigid-body.
    ///
    /// CCD prevents tunneling, but may still allow limited interpenetration of colliders.
    pub fn ccd_enabled(mut self, enabled: bool) -> Self {
        self.ccd_enabled = enabled;
        self
    }

    /// Sets whether or not the rigid-body is to be created asleep.
    pub fn sleeping(mut self, sleeping: bool) -> Self {
        self.sleeping = sleeping;
        self
    }

    /// Enable or disable the rigid-body after its creation.
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    /// Build a new rigid-body with the parameters configured with this builder.
    pub fn build(&self) -> RigidBody {
        let builder = rapier2d::dynamics::RigidBodyBuilder::new(self.body_type)
            .locked_axes(self.mprops_flags)
            .linvel(self.linvel.into())
            .angvel(self.angvel.into())
            .gravity_scale(self.gravity_scale)
            .linear_damping(self.linear_damping)
            .angular_damping(self.angular_damping)
            .can_sleep(self.can_sleep)
            .sleeping(self.sleeping)
            .ccd_enabled(self.ccd_enabled)
            .dominance_group(self.dominance_group)
            .enabled(self.enabled);

        RigidBody(builder.into())
    }
}

impl Into<RigidBody> for RigidBodyBuilder {
    fn into(self) -> RigidBody {
        self.build()
    }
}

pub struct Shape(pub(crate) SharedShape);

impl Shape {
    /// Initialize a compound shape defined by its subshapes.
    pub fn compound(shapes: Vec<(Transform, Shape)>) -> Self {
        Self(SharedShape::compound(
            shapes
                .into_iter()
                .map(|x| ((x.0.position, x.0.rotation).into(), x.1 .0))
                .collect(),
        ))
    }

    /// Initialize a circle shape defined by its radius.
    pub fn circle(radius: Real) -> Self {
        Self(SharedShape::ball(radius))
    }

    /// Initialize a cuboid shape defined by its half-extents.
    pub fn square(hx: Real, hy: Real) -> Self {
        Self(SharedShape::cuboid(hx, hy))
    }

    /// Initialize a round cuboid shape defined by its half-extents and border radius.
    pub fn rounded_square(hx: Real, hy: Real, border_radius: Real) -> Self {
        Self(SharedShape::round_cuboid(hx, hy, border_radius))
    }

    /// Initialize a capsule shape from its endpoints and radius.
    pub fn capsule(a: Vec2, b: Vec2, radius: Real) -> Self {
        Self(SharedShape::capsule(a.into(), b.into(), radius))
    }

    /// Initialize a capsule shape aligned with the `x` axis.
    pub fn capsule_x(half_height: Real, radius: Real) -> Self {
        Self(SharedShape::capsule_x(half_height, radius))
    }

    /// Initialize a capsule shape aligned with the `y` axis.
    pub fn capsule_y(half_height: Real, radius: Real) -> Self {
        Self(SharedShape::capsule_x(half_height, radius))
    }

    /// Initialize a segment shape from its endpoints.
    pub fn segment(a: Vec2, b: Vec2) -> Self {
        Self(SharedShape::segment(a.into(), b.into()))
    }

    /// Initializes a triangle shape.
    pub fn triangle(a: Vec2, b: Vec2, c: Vec2) -> Self {
        Self(SharedShape::triangle(a.into(), b.into(), c.into()))
    }

    /// Initializes a triangle shape with round corners.
    pub fn round_triangle(a: Vec2, b: Vec2, c: Vec2, radius: Real) -> Self {
        Self(SharedShape::round_triangle(
            a.into(),
            b.into(),
            c.into(),
            radius,
        ))
    }

    /// Initializes a polyline shape defined by its vertex and index buffers.
    ///
    /// If no index buffer is provided, the polyline is assumed to describe a line strip.
    pub fn polyline(vertices: Vec<Vec2>, indices: Option<Vec<[u32; 2]>>) -> Self {
        Self(SharedShape::polyline(
            vertices.into_iter().map(|x| x.into()).collect(),
            indices,
        ))
    }

    /// Initializes a triangle mesh shape defined by its vertex and index buffers.
    pub fn trimesh(data: Data) -> Self {
        Self(SharedShape::trimesh(
            data.vertices
                .into_iter()
                .map(|x| x.position.into())
                .collect(),
            data.indices.chunks(3).map(|x| [x[0], x[1], x[2]]).collect(),
        ))
    }

    /// Initializes a compound shape obtained from the decomposition of the given
    /// polyline into convex parts.
    pub fn convex_decomposition(vertices: &[Vec2], indices: &[[u32; 2]]) -> Self {
        let vertices = vertices
            .iter()
            .map(|x| Point::from(x.to_array()))
            .collect::<Vec<Point<Real>>>();
        Self(SharedShape::convex_decomposition(&vertices, indices))
    }

    /// Initializes a compound shape obtained from the decomposition of the given
    /// polyline into convex parts dilated with round corners.
    pub fn round_convex_decomposition(
        vertices: &[Vec2],
        indices: &[[u32; 2]],
        radius: Real,
    ) -> Self {
        let vertices = vertices
            .iter()
            .map(|x| Point::from(x.to_array()))
            .collect::<Vec<Point<Real>>>();
        Self(SharedShape::round_convex_decomposition(
            &vertices, indices, radius,
        ))
    }

    /// Initializes a compound shape obtained from the decomposition of the given
    /// polyline into convex parts.
    pub fn convex_decomposition_with_params(
        vertices: &[Vec2],
        indices: &[[u32; 2]],
        params: &VHACDParameters,
    ) -> Self {
        let vertices = vertices
            .iter()
            .map(|x| Point::from(x.to_array()))
            .collect::<Vec<Point<Real>>>();
        Self(SharedShape::convex_decomposition_with_params(
            &vertices, indices, params,
        ))
    }

    /// Initializes a compound shape obtained from the decomposition of the given
    /// polyline into convex parts dilated with round corners.
    pub fn round_convex_decomposition_with_params(
        vertices: &[Vec2],
        indices: &[[u32; 2]],
        params: &VHACDParameters,
        radius: Real,
    ) -> Self {
        let vertices = vertices
            .iter()
            .map(|x| Point::from(x.to_array()))
            .collect::<Vec<Point<Real>>>();
        Self(SharedShape::round_convex_decomposition_with_params(
            &vertices, indices, params, radius,
        ))
    }

    /// Creates a new shared shape that is the convex-hull of the given points.
    pub fn convex_hull(points: &[Vec2]) -> Option<Self> {
        let points = points
            .iter()
            .map(|x| Point::from(x.to_array()))
            .collect::<Vec<Point<Real>>>();
        let shape = SharedShape::convex_hull(&points);
        shape.map(Self)
    }

    /// Creates a new shared shape with rounded corners that is the
    /// convex-hull of the given points, dilated by `border_radius`.
    pub fn round_convex_hull(points: &[Vec2], border_radius: Real) -> Option<Self> {
        let points = points
            .iter()
            .map(|x| Point::from(x.to_array()))
            .collect::<Vec<Point<Real>>>();
        let shape = SharedShape::round_convex_hull(&points, border_radius);
        shape.map(Self)
    }

    /// Creates a new shared shape that is a convex polygon formed by the
    /// given set of points assumed to form a convex polyline (no convex-hull will be automatically
    /// computed).
    pub fn convex_polyline(points: &[Vec2]) -> Option<Self> {
        let points = points
            .iter()
            .map(|x| Point::from(x.to_array()))
            .collect::<Vec<Point<Real>>>();
        let shape = SharedShape::convex_polyline(points);
        shape.map(Self)
    }

    /// Creates a new collider builder that is a round convex polygon formed by the
    /// given polyline assumed to be convex (no convex-hull will be automatically
    /// computed). The polygon shape is dilated by a sphere of radius `border_radius`.
    pub fn round_convex_polyline(points: Vec<Vec2>, border_radius: Real) -> Option<Self> {
        let points = points
            .iter()
            .map(|x| Point::from(x.to_array()))
            .collect::<Vec<Point<Real>>>();
        let shape = SharedShape::round_convex_polyline(points, border_radius);
        shape.map(Self)
    }

    /// Initializes an heightfield shape defined by its set of height and a scale
    /// factor along each coordinate axis.
    pub fn heightfield(heights: Vec<Real>, scale: Vec2) -> Self {
        Self(SharedShape::heightfield(heights.into(), scale.into()))
    }

    /// Initializes a let engine shape from a Rapier shared shape.
    pub fn from_shared_shape(shape: SharedShape) -> Self {
        Self(shape)
    }
}
