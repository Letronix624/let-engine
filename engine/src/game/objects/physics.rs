use crate::{Data, Transform};
use glam::f32::Vec2;
use parking_lot::Mutex;
use rapier2d::parry::transformation::vhacd::VHACDParameters;
use rapier2d::prelude::*;
use std::sync::Arc;

pub type APhysics = Arc<Mutex<Physics>>;

pub struct Physics {
    pub(crate) rigid_body_set: RigidBodySet,
    pub(crate) collider_set: ColliderSet,

    pub(crate) gravity: Vector<Real>,
    integration_parameters: IntegrationParameters,
    pub(crate) island_manager: IslandManager,
    broad_phase: BroadPhase,
    narrow_phase: NarrowPhase,
    impulse_joint_set: ImpulseJointSet,
    multibody_joint_set: MultibodyJointSet,
    ccd_solver: CCDSolver,
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
            gravity: vector!(0.0, -9.81),
            integration_parameters: IntegrationParameters::default(),
            island_manager: IslandManager::new(),
            broad_phase: BroadPhase::new(),
            narrow_phase: NarrowPhase::new(),
            impulse_joint_set: ImpulseJointSet::new(),
            multibody_joint_set: MultibodyJointSet::new(),
            ccd_solver: CCDSolver::new(),
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
    }
}

#[derive(Clone)]
pub struct Collider {
    pub collider: rapier2d::geometry::Collider,
}

impl Collider {
    pub fn is_sensor(&self) -> bool {
        self.collider.is_sensor()
    }
    pub fn set_sensor(&mut self, is_sensor: bool) {
        self.collider.set_sensor(is_sensor)
    }
    pub fn friction(&self) -> Real {
        self.collider.friction()
    }
    pub fn set_friction(&mut self, coefficient: Real) {
        self.collider.set_friction(coefficient)
    }
    pub fn friction_combine_rule(&self) -> CoefficientCombineRule {
        self.collider.friction_combine_rule()
    }
    pub fn set_friction_combine_rule(&mut self, rule: CoefficientCombineRule) {
        self.collider.set_friction_combine_rule(rule)
    }
    pub fn restitution(&self) -> Real {
        self.collider.restitution()
    }
    pub fn set_restitution(&mut self, coefficient: Real) {
        self.collider.set_restitution(coefficient)
    }
    pub fn restitution_combine_rule(&self) -> CoefficientCombineRule {
        self.collider.restitution_combine_rule()
    }
    pub fn set_restitution_combine_rule(&mut self, rule: CoefficientCombineRule) {
        self.collider.set_restitution_combine_rule(rule)
    }
    pub fn set_contact_force_event_threshold(&mut self, threshold: Real) {
        self.collider.set_contact_force_event_threshold(threshold)
    }
    pub fn is_enabled(&self) -> bool {
        self.collider.is_enabled()
    }
    pub fn set_enabled(&mut self, enabled: bool) {
        self.collider.set_enabled(enabled)
    }
    pub fn volume(&self) -> Real {
        self.collider.volume()
    }
    pub fn density(&self) -> Real {
        self.collider.density()
    }
    pub fn mass(&self) -> Real {
        self.collider.mass()
    }
    pub fn set_density(&mut self, density: Real) {
        self.collider.set_density(density)
    }
    pub fn set_mass(&mut self, mass: Real) {
        self.collider.set_mass(mass)
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

impl ColliderBuilder {
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
        Collider {
            collider: rapier2d::geometry::ColliderBuilder {
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
        }
    }
    pub fn compound(shapes: Vec<(Transform, Shape)>) -> Self {
        Self::new(Shape::compound(shapes))
    }
    pub fn circle(radius: Real) -> Self {
        Self::new(Shape::circle(radius))
    }
    pub fn square(hx: Real, hy: Real) -> Self {
        Self::new(Shape::square(hx, hy))
    }
    pub fn rounded_square(hx: Real, hy: Real, border_radius: Real) -> Self {
        Self::new(Shape::rounded_square(hx, hy, border_radius))
    }
    pub fn capsule_x(half_height: Real, radius: Real) -> Self {
        Self::new(Shape::capsule_x(half_height, radius))
    }
    pub fn capsule_y(half_height: Real, radius: Real) -> Self {
        Self::new(Shape::capsule_x(half_height, radius))
    }
    pub fn segment(a: Vec2, b: Vec2) -> Self {
        Self::new(Shape::segment(a, b))
    }
    pub fn triangle(a: Vec2, b: Vec2, c: Vec2) -> Self {
        Self::new(Shape::triangle(a, b, c))
    }
    pub fn round_triangle(a: Vec2, b: Vec2, c: Vec2, radius: Real) -> Self {
        Self::new(Shape::round_triangle(a, b, c, radius))
    }
    pub fn polyline(vertices: Vec<Vec2>, indices: Option<Vec<[u32; 2]>>) -> Self {
        Self::new(Shape::polyline(vertices, indices))
    }
    pub fn trimesh(data: Data) -> Self {
        Self::new(Shape::trimesh(data))
    }
    pub fn convex_decomposition(vertices: &[Vec2], indices: &[[u32; 2]]) -> Self {
        Self::new(Shape::convex_decomposition(vertices, indices))
    }
    pub fn round_convex_decomposition(
        vertices: &[Vec2],
        indices: &[[u32; 2]],
        radius: Real,
    ) -> Self {
        Self::new(Shape::round_convex_decomposition(vertices, indices, radius))
    }
    pub fn convex_decomposition_with_params(
        vertices: &[Vec2],
        indices: &[[u32; 2]],
        params: &VHACDParameters,
    ) -> Self {
        Self::new(Shape::convex_decomposition_with_params(
            vertices, indices, params,
        ))
    }
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
    pub fn convex_hull(points: &[Vec2]) -> Option<Self> {
        let shape = Shape::convex_hull(points);
        shape.map(Self::new)
    }
    pub fn convex_polyline(points: &[Vec2]) -> Option<Self> {
        let shape = Shape::convex_polyline(points);
        shape.map(Self::new)
    }
    pub fn heightfield(heights: Vec<Real>, scale: Vec2) -> Self {
        Self::new(Shape::heightfield(heights, scale))
    }
    pub fn from_shared_shape(shape: SharedShape) -> Self {
        Self::new(Shape::from_shared_shape(shape))
    }
}
pub struct Shape(pub(crate) SharedShape);

impl Shape {
    pub fn compound(shapes: Vec<(Transform, Shape)>) -> Self {
        Self(SharedShape::compound(
            shapes
                .into_iter()
                .map(|x| ((x.0.position, x.0.rotation).into(), x.1 .0))
                .collect(),
        ))
    }
    pub fn circle(radius: Real) -> Self {
        Self(SharedShape::ball(radius))
    }
    pub fn square(hx: Real, hy: Real) -> Self {
        Self(SharedShape::cuboid(hx, hy))
    }
    pub fn rounded_square(hx: Real, hy: Real, border_radius: Real) -> Self {
        Self(SharedShape::round_cuboid(hx, hy, border_radius))
    }
    pub fn capsule_x(half_height: Real, radius: Real) -> Self {
        Self(SharedShape::capsule_x(half_height, radius))
    }
    pub fn capsule_y(half_height: Real, radius: Real) -> Self {
        Self(SharedShape::capsule_x(half_height, radius))
    }
    pub fn segment(a: Vec2, b: Vec2) -> Self {
        Self(SharedShape::segment(a.into(), b.into()))
    }
    pub fn triangle(a: Vec2, b: Vec2, c: Vec2) -> Self {
        Self(SharedShape::triangle(a.into(), b.into(), c.into()))
    }
    pub fn round_triangle(a: Vec2, b: Vec2, c: Vec2, radius: Real) -> Self {
        Self(SharedShape::round_triangle(
            a.into(),
            b.into(),
            c.into(),
            radius,
        ))
    }
    pub fn polyline(vertices: Vec<Vec2>, indices: Option<Vec<[u32; 2]>>) -> Self {
        Self(SharedShape::polyline(
            vertices.into_iter().map(|x| x.into()).collect(),
            indices,
        ))
    }
    pub fn trimesh(data: Data) -> Self {
        Self(SharedShape::trimesh(
            data.vertices
                .into_iter()
                .map(|x| x.position.into())
                .collect(),
            data.indices.chunks(3).map(|x| [x[0], x[1], x[3]]).collect(),
        ))
    }
    pub fn convex_decomposition(vertices: &[Vec2], indices: &[[u32; 2]]) -> Self {
        let vertices = vertices
            .iter()
            .map(|x| Point::from(x.to_array()))
            .collect::<Vec<Point<Real>>>();
        Self(SharedShape::convex_decomposition(&vertices, indices))
    }
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
    pub fn convex_hull(points: &[Vec2]) -> Option<Self> {
        let points = points
            .iter()
            .map(|x| Point::from(x.to_array()))
            .collect::<Vec<Point<Real>>>();
        let shape = SharedShape::convex_hull(&points);
        shape.map(Self)
    }
    pub fn convex_polyline(points: &[Vec2]) -> Option<Self> {
        let points = points
            .iter()
            .map(|x| Point::from(x.to_array()))
            .collect::<Vec<Point<Real>>>();
        let shape = SharedShape::convex_polyline(points);
        shape.map(Self)
    }
    pub fn heightfield(heights: Vec<Real>, scale: Vec2) -> Self {
        Self(SharedShape::heightfield(heights.into(), scale.into()))
    }
    pub fn from_shared_shape(shape: SharedShape) -> Self {
        Self(shape)
    }
}
