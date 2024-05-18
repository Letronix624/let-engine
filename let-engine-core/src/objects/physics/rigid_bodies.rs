//! Wrapping of Rapiers rigid bodies to be used with Let Engine and Glam.

use glam::{vec2, Vec2};
use nalgebra::Isometry2;
use rapier2d::prelude::*;

use thiserror::Error;

/// This error gets returned when one of the objects input into register_joint does not have a rigid body to attach the joint to.
#[derive(Error, Debug)]
#[error("One of the objects does not have a rigid body")]
pub struct NoRigidBodyError;

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
        let p = *self.0.center_of_mass();

        vec2(p.x, p.y)
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
        let p = *self.0.next_position();

        (vec2(p.translation.x, p.translation.y), p.rotation.angle())
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
        let p = *self.0.linvel();

        vec2(p.x, p.y)
    }

    /// The angular velocity of this rigid-body.
    pub fn angvel(&self) -> Real {
        self.0.angvel()
    }

    pub fn set_linvel(&mut self, linvel: Vec2, wake_up: bool) {
        let vec = mint::Vector2::from(linvel);
        self.0.set_linvel(vec.into(), wake_up)
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
            self.0
                .set_next_kinematic_rotation(Rotation::from_angle(rotation));
        }
    }

    /// If this rigid body is kinematic, sets its future translation after the next timestep integration.
    pub fn set_next_kinematic_translation(&mut self, translation: Vec2) {
        let vec = mint::Vector2::from(translation);
        self.0.set_next_kinematic_translation(vec.into())
    }

    /// If this rigid body is kinematic, sets its future position after the next timestep integration.
    pub fn set_next_kinematic_position(&mut self, pos: (Vec2, f32)) {
        let vec = mint::Vector2::from(pos.0);
        let iso = Isometry2::new(vec.into(), pos.1);
        self.0.set_next_kinematic_position(iso)
    }

    /// Predicts the next position of this rigid-body, by integrating its velocity and forces
    /// by a time of `dt`.
    pub fn predict_position_using_velocity_and_forces(&self, dt: Real) -> (Vec2, f32) {
        let iso = self.0.predict_position_using_velocity_and_forces(dt);
        (
            vec2(iso.translation.x, iso.translation.y),
            iso.rotation.angle(),
        )
    }

    /// Predicts the next position of this rigid-body, by integrating its velocity
    /// by a time of `dt`.
    pub fn predict_position_using_velocity(&self, dt: Real) -> (Vec2, f32) {
        let iso = self.0.predict_position_using_velocity(dt);
        (
            vec2(iso.translation.x, iso.translation.y),
            iso.rotation.angle(),
        )
    }
}

impl RigidBody {
    /// The velocity of the given world-space point on this rigid-body.
    pub fn velocity_at_point(&self, point: Vec2) -> Vec2 {
        let point = mint::Point2::from(point);
        let pos = *self.0.velocity_at_point(&point.into());
        (pos.x, pos.y).into()
    }

    /// The kinetic energy of this body.
    pub fn kinetic_energy(&self) -> Real {
        self.0.kinetic_energy()
    }

    /// The potential energy of this body in a gravity field.
    pub fn gravitational_potential_energy(&self, dt: Real, gravity: Vec2) -> Real {
        let vec = mint::Vector2::from(gravity);
        self.0.gravitational_potential_energy(dt, vec.into())
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
        let vec = mint::Vector2::from(force);
        self.0.add_force(vec.into(), wake_up)
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
        let force = mint::Vector2::from(force);
        let point = mint::Point2::from(point);
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
        let impulse = mint::Vector2::from(impulse);
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
        let impulse = mint::Vector2::from(impulse);
        let point = mint::Point2::from(point);
        self.0
            .apply_impulse_at_point(impulse.into(), point.into(), wake_up)
    }

    /// Retrieves the constant force(s) that the user has added to the body.
    ///
    /// Returns zero if the rigid-body isn’t dynamic.
    pub fn user_force(&self) -> Vec2 {
        let force = self.0.user_force();
        vec2(force.x, force.y)
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
        let linvel = mint::Vector2::from(self.linvel);
        let builder = rapier2d::dynamics::RigidBodyBuilder::new(self.body_type)
            .locked_axes(self.mprops_flags)
            .linvel(linvel.into())
            .angvel(self.angvel)
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

impl From<RigidBodyBuilder> for RigidBody {
    fn from(val: RigidBodyBuilder) -> Self {
        val.build()
    }
}
