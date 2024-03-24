//! Wrapping of Rapiers colliders to be used with Let Engine and Glam.

use crate::prelude::*;
use rapier2d::prelude::*;

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
    #[cfg(feature = "client")]
    pub fn trimesh(data: Data) -> Self {
        Self::new(Shape::trimesh(data))
    }
    /// Initializes a triangle mesh shape defined by its vertex and index buffers.
    #[cfg(not(feature = "client"))]
    pub fn trimesh(data: (Vec<Vec2>, Vec<[u32; 3]>)) -> Self {
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
    #[cfg(feature = "client")]
    pub fn trimesh(data: Data) -> Self {
        Self(SharedShape::trimesh(
            data.vertices().iter().map(|x| x.position.into()).collect(),
            data.indices()
                .chunks(3)
                .map(|x| [x[0], x[1], x[2]])
                .collect(),
        ))
    }
    /// Initializes a triangle mesh shape defined by its vertex and index buffers.
    #[cfg(not(feature = "client"))]
    pub fn trimesh(data: (Vec<Vec2>, Vec<[u32; 3]>)) -> Self {
        Self(SharedShape::trimesh(
            data.0.into_iter().map(|x| x.into()).collect(),
            data.1,
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
