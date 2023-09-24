use glam::Vec2;
use rapier2d::{
    dynamics::GenericJoint as RGenericJoint,
    prelude::{JointAxis, JointEnabled, JointLimits, JointMotor, MotorModel, Real, UnitVector},
};

pub use rapier2d::dynamics::JointAxesMask;

// GenericJoint

#[derive(Copy, Clone, Debug, PartialEq, Default)]
/// A generic joint.
pub struct GenericJoint {
    pub data: RGenericJoint,
}

impl GenericJoint {
    /// Creates a new generic joint that locks the specified degrees of freedom.
    #[must_use]
    pub fn new(locked_axes: JointAxesMask) -> Self {
        Self {
            data: RGenericJoint::new(locked_axes),
        }
    }

    /// Is this joint enabled?
    pub fn is_enabled(&self) -> bool {
        self.data.enabled == JointEnabled::Enabled
    }

    /// Set whether this joint is enabled or not.
    pub fn set_enabled(&mut self, enabled: bool) {
        self.data.set_enabled(enabled);
    }

    /// Add the specified axes to the set of axes locked by this joint.
    pub fn lock_axes(&mut self, axes: JointAxesMask) -> &mut Self {
        self.data = *self.data.lock_axes(axes);
        self
    }

    /// Sets the joint’s frame, expressed in the first rigid-body’s local-space.
    pub fn set_local_frame1(&mut self, local_frame: (Vec2, f32)) -> &mut Self {
        self.data.set_local_frame1(local_frame.into());
        self
    }

    /// Sets the joint’s frame, expressed in the second rigid-body’s local-space.
    pub fn set_local_frame2(&mut self, local_frame: (Vec2, f32)) -> &mut Self {
        self.data.set_local_frame2(local_frame.into());
        self
    }

    /// The principal (local X) axis of this joint, expressed in the first rigid-body’s local-space.
    #[must_use]
    pub fn local_axis1(&self) -> Vec2 {
        self.data.local_axis1().into()
    }

    /// Sets the principal (local X) axis of this joint, expressed in the first rigid-body’s local-space.
    pub fn set_local_axis1(&mut self, local_axis: Vec2) -> &mut Self {
        self.data
            .set_local_axis1(UnitVector::new_normalize(local_axis.into()));
        self
    }

    /// The principal (local X) axis of this joint, expressed in the second rigid-body’s local-space.
    #[must_use]
    pub fn local_axis2(&self) -> Vec2 {
        self.data.local_axis2().into()
    }

    /// Sets the principal (local X) axis of this joint, expressed in the second rigid-body’s local-space.
    pub fn set_local_axis2(&mut self, local_axis: Vec2) -> &mut Self {
        self.data
            .set_local_axis2(UnitVector::new_normalize(local_axis.into()));
        self
    }

    /// The anchor of this joint, expressed in the first rigid-body’s local-space.
    #[must_use]
    pub fn local_anchor1(&self) -> Vec2 {
        self.data.local_anchor1().into()
    }

    /// Sets anchor of this joint, expressed in the first rigid-body’s local-space.
    pub fn set_local_anchor1(&mut self, anchor1: Vec2) -> &mut Self {
        self.data.set_local_anchor1(anchor1.into());
        self
    }

    /// The anchor of this joint, expressed in the second rigid-body’s local-space.
    #[must_use]
    pub fn local_anchor2(&self) -> Vec2 {
        self.data.local_anchor2().into()
    }

    /// Sets anchor of this joint, expressed in the second rigid-body’s local-space.
    pub fn set_local_anchor2(&mut self, anchor2: Vec2) -> &mut Self {
        self.data.set_local_anchor2(anchor2.into());
        self
    }

    /// Are contacts between the attached rigid-bodies enabled?
    pub fn contacts_enabled(&self) -> bool {
        self.data.contacts_enabled()
    }

    /// Sets whether contacts between the attached rigid-bodies are enabled.
    pub fn set_contacts_enabled(&mut self, enabled: bool) -> &mut Self {
        self.data.set_contacts_enabled(enabled);
        self
    }

    /// The joint limits along the specified axis.
    #[must_use]
    pub fn limits(&self, axis: JointAxis) -> Option<&JointLimits<Real>> {
        self.data.limits(axis)
    }

    /// Sets the joint limits along the specified axis.
    pub fn set_limits(&mut self, axis: JointAxis, limits: [Real; 2]) -> &mut Self {
        self.data.set_limits(axis, limits);
        self
    }

    /// The spring-like motor model along the specified axis of this joint.
    #[must_use]
    pub fn motor_model(&self, axis: JointAxis) -> Option<MotorModel> {
        self.data.motor_model(axis)
    }

    /// Set the spring-like model used by the motor to reach the desired target velocity and position.
    pub fn set_motor_model(&mut self, axis: JointAxis, model: MotorModel) -> &mut Self {
        self.data.set_motor_model(axis, model);
        self
    }

    /// Sets the target velocity this motor needs to reach.
    pub fn set_motor_velocity(
        &mut self,
        axis: JointAxis,
        target_vel: Real,
        factor: Real,
    ) -> &mut Self {
        self.set_motor(
            axis,
            self.data.motors[axis as usize].target_pos,
            target_vel,
            0.0,
            factor,
        )
    }

    /// Sets the target angle this motor needs to reach.
    pub fn set_motor_position(
        &mut self,
        axis: JointAxis,
        target_pos: Real,
        stiffness: Real,
        damping: Real,
    ) -> &mut Self {
        self.set_motor(axis, target_pos, 0.0, stiffness, damping)
    }

    /// Sets the maximum force the motor can deliver along the specified axis.
    pub fn set_motor_max_force(&mut self, axis: JointAxis, max_force: Real) -> &mut Self {
        self.data.motors[axis as usize].max_force = max_force;
        self
    }

    /// The motor affecting the joint’s degree of freedom along the specified axis.
    #[must_use]
    pub fn motor(&self, axis: JointAxis) -> Option<&JointMotor> {
        self.data.motor(axis)
    }

    /// Configure both the target angle and target velocity of the motor.
    pub fn set_motor(
        &mut self,
        axis: JointAxis,
        target_pos: Real,
        target_vel: Real,
        stiffness: Real,
        damping: Real,
    ) -> &mut Self {
        self.data
            .set_motor(axis, target_pos, target_vel, stiffness, damping);
        self
    }
}

/// Create generic joints using the builder pattern.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct GenericJointBuilder(pub GenericJoint);

impl GenericJointBuilder {
    /// Creates a new generic joint builder.
    #[must_use]
    pub fn new(locked_axes: JointAxesMask) -> Self {
        Self(GenericJoint::new(locked_axes))
    }

    /// Sets the degrees of freedom locked by the joint.
    #[must_use]
    pub fn locked_axes(mut self, axes: JointAxesMask) -> Self {
        self.0.data.locked_axes = axes;
        self
    }

    /// Sets whether contacts between the attached rigid-bodies are enabled.
    #[must_use]
    pub fn contacts_enabled(mut self, enabled: bool) -> Self {
        self.0.data.contacts_enabled = enabled;
        self
    }

    /// Sets the joint’s frame, expressed in the first rigid-body’s local-space.
    #[must_use]
    pub fn local_frame1(mut self, local_frame: (Vec2, f32)) -> Self {
        self.0.set_local_frame1(local_frame);
        self
    }

    /// Sets the joint’s frame, expressed in the second rigid-body’s local-space.
    #[must_use]
    pub fn local_frame2(mut self, local_frame: (Vec2, f32)) -> Self {
        self.0.set_local_frame2(local_frame);
        self
    }

    /// Sets the principal (local X) axis of this joint, expressed in the first rigid-body’s local-space.
    #[must_use]
    pub fn local_axis1(mut self, local_axis: Vec2) -> Self {
        self.0.set_local_axis1(local_axis);
        self
    }

    /// Sets the principal (local X) axis of this joint, expressed in the second rigid-body’s local-space.
    #[must_use]
    pub fn local_axis2(mut self, local_axis: Vec2) -> Self {
        self.0.set_local_axis2(local_axis);
        self
    }

    /// Sets the anchor of this joint, expressed in the first rigid-body’s local-space.
    #[must_use]
    pub fn local_anchor1(mut self, anchor1: Vec2) -> Self {
        self.0.set_local_anchor1(anchor1);
        self
    }

    /// Sets the anchor of this joint, expressed in the second rigid-body’s local-space.
    #[must_use]
    pub fn local_anchor2(mut self, anchor2: Vec2) -> Self {
        self.0.set_local_anchor2(anchor2);
        self
    }

    /// Sets the joint limits along the specified axis.
    #[must_use]
    pub fn limits(mut self, axis: JointAxis, limits: [Real; 2]) -> Self {
        self.0.set_limits(axis, limits);
        self
    }

    /// Sets the coupled degrees of freedom for this joint’s limits and motor.
    #[must_use]
    pub fn coupled_axes(mut self, axes: JointAxesMask) -> Self {
        self.0.data.coupled_axes = axes;
        self
    }

    /// Set the spring-like model used by the motor to reach the desired target velocity and position.
    #[must_use]
    pub fn motor_model(mut self, axis: JointAxis, model: MotorModel) -> Self {
        self.0.set_motor_model(axis, model);
        self
    }

    /// Sets the target velocity this motor needs to reach.
    #[must_use]
    pub fn motor_velocity(mut self, axis: JointAxis, target_vel: Real, factor: Real) -> Self {
        self.0.set_motor_velocity(axis, target_vel, factor);
        self
    }

    /// Sets the target angle this motor needs to reach.
    #[must_use]
    pub fn motor_position(
        mut self,
        axis: JointAxis,
        target_pos: Real,
        stiffness: Real,
        damping: Real,
    ) -> Self {
        self.0
            .set_motor_position(axis, target_pos, stiffness, damping);
        self
    }

    /// Configure both the target angle and target velocity of the motor.
    #[must_use]
    pub fn set_motor(
        mut self,
        axis: JointAxis,
        target_pos: Real,
        target_vel: Real,
        stiffness: Real,
        damping: Real,
    ) -> Self {
        self.0
            .set_motor(axis, target_pos, target_vel, stiffness, damping);
        self
    }

    /// Sets the maximum force the motor can deliver along the specified axis.
    #[must_use]
    pub fn motor_max_force(mut self, axis: JointAxis, max_force: Real) -> Self {
        self.0.set_motor_max_force(axis, max_force);
        self
    }

    /// Builds the generic joint.
    #[must_use]
    pub fn build(self) -> GenericJoint {
        self.0
    }
}

impl From<GenericJointBuilder> for GenericJoint {
    fn from(val: GenericJointBuilder) -> Self {
        val.0
    }
}

// FixedJoint

#[derive(Copy, Clone, Debug, PartialEq)]
#[repr(transparent)]
/// A fixed joint, locks all relative motion between two bodies.
pub struct FixedJoint {
    /// The underlying joint data.
    pub data: GenericJoint,
}

impl Default for FixedJoint {
    fn default() -> Self {
        FixedJoint::new()
    }
}

impl FixedJoint {
    /// Creates a new fixed joint.
    #[must_use]
    pub fn new() -> Self {
        let data = GenericJointBuilder::new(JointAxesMask::LOCKED_FIXED_AXES).build();
        Self { data }
    }

    /// Are contacts between the attached rigid-bodies enabled?
    pub fn contacts_enabled(&self) -> bool {
        self.data.data.contacts_enabled
    }

    /// Sets whether contacts between the attached rigid-bodies are enabled.
    pub fn set_contacts_enabled(&mut self, enabled: bool) -> &mut Self {
        self.data.set_contacts_enabled(enabled);
        self
    }

    /// The joint’s frame, expressed in the first rigid-body’s local-space.
    #[must_use]
    pub fn local_frame1(&self) -> (Vec2, f32) {
        self.data.data.local_frame1.into()
    }

    /// Sets the joint’s frame, expressed in the first rigid-body’s local-space.
    pub fn set_local_frame1(&mut self, local_frame: (Vec2, f32)) -> &mut Self {
        self.data.set_local_frame1(local_frame);
        self
    }

    /// The joint’s frame, expressed in the second rigid-body’s local-space.
    #[must_use]
    pub fn local_frame2(&self) -> (Vec2, f32) {
        self.data.data.local_frame2.into()
    }

    /// Sets joint’s frame, expressed in the second rigid-body’s local-space.
    pub fn set_local_frame2(&mut self, local_frame: (Vec2, f32)) -> &mut Self {
        self.data.set_local_frame2(local_frame);
        self
    }

    /// The joint’s anchor, expressed in the local-space of the first rigid-body.
    #[must_use]
    pub fn local_anchor1(&self) -> Vec2 {
        self.data.local_anchor1()
    }

    /// Sets the joint’s anchor, expressed in the local-space of the first rigid-body.
    pub fn set_local_anchor1(&mut self, anchor1: Vec2) -> &mut Self {
        self.data.set_local_anchor1(anchor1);
        self
    }

    /// The joint’s anchor, expressed in the local-space of the second rigid-body.
    #[must_use]
    pub fn local_anchor2(&self) -> Vec2 {
        self.data.local_anchor2()
    }

    /// Sets the joint’s anchor, expressed in the local-space of the second rigid-body.
    pub fn set_local_anchor2(&mut self, anchor2: Vec2) -> &mut Self {
        self.data.set_local_anchor2(anchor2);
        self
    }
}

/// Create fixed joints using the builder pattern.
#[derive(Copy, Clone, Debug, PartialEq, Default)]
pub struct FixedJointBuilder(pub FixedJoint);

impl FixedJointBuilder {
    /// Creates a new builder for fixed joints.
    pub fn new() -> Self {
        Self(FixedJoint::new())
    }

    /// Sets whether contacts between the attached rigid-bodies are enabled.
    #[must_use]
    pub fn contacts_enabled(mut self, enabled: bool) -> Self {
        self.0.set_contacts_enabled(enabled);
        self
    }

    /// Sets the joint’s frame, expressed in the first rigid-body’s local-space.
    #[must_use]
    pub fn local_frame1(mut self, local_frame: (Vec2, f32)) -> Self {
        self.0.set_local_frame1(local_frame);
        self
    }

    /// Sets joint’s frame, expressed in the second rigid-body’s local-space.
    #[must_use]
    pub fn local_frame2(mut self, local_frame: (Vec2, f32)) -> Self {
        self.0.set_local_frame2(local_frame);
        self
    }

    /// Sets the joint’s anchor, expressed in the local-space of the first rigid-body.
    #[must_use]
    pub fn local_anchor1(mut self, anchor1: Vec2) -> Self {
        self.0.set_local_anchor1(anchor1);
        self
    }

    /// Sets the joint’s anchor, expressed in the local-space of the second rigid-body.
    #[must_use]
    pub fn local_anchor2(mut self, anchor2: Vec2) -> Self {
        self.0.set_local_anchor2(anchor2);
        self
    }

    /// Build the fixed joint.
    #[must_use]
    pub fn build(self) -> FixedJoint {
        self.0
    }
}

impl From<FixedJointBuilder> for GenericJoint {
    fn from(val: FixedJointBuilder) -> Self {
        val.0.data
    }
}

// PrismaticJoint

#[derive(Copy, Clone, Debug, PartialEq)]
#[repr(transparent)]
/// A prismatic joint, locks all relative motion between two bodies except for translation along the joint’s principal axis.
pub struct PrismaticJoint {
    /// The underlying joint data.
    pub data: GenericJoint,
}

impl PrismaticJoint {
    /// Creates a new prismatic joint allowing only relative translations along the specified axis.
    ///
    /// This axis is expressed in the local-space of both rigid-bodies.
    pub fn new(axis: Vec2) -> Self {
        let data = GenericJointBuilder::new(JointAxesMask::LOCKED_PRISMATIC_AXES)
            .local_axis1(axis)
            .local_axis2(axis)
            .build();
        Self { data }
    }

    /// The underlying generic joint.
    pub fn data(&self) -> &GenericJoint {
        &self.data
    }

    /// Are contacts between the attached rigid-bodies enabled?
    pub fn contacts_enabled(&self) -> bool {
        self.data.contacts_enabled()
    }

    /// Sets whether contacts between the attached rigid-bodies are enabled.
    pub fn set_contacts_enabled(&mut self, enabled: bool) -> &mut Self {
        self.data.set_contacts_enabled(enabled);
        self
    }

    /// The joint’s anchor, expressed in the local-space of the first rigid-body.
    #[must_use]
    pub fn local_anchor1(&self) -> Vec2 {
        self.data.local_anchor1()
    }

    /// Sets the joint’s anchor, expressed in the local-space of the first rigid-body.
    pub fn set_local_anchor1(&mut self, anchor1: Vec2) -> &mut Self {
        self.data.set_local_anchor1(anchor1);
        self
    }

    /// The joint’s anchor, expressed in the local-space of the second rigid-body.
    #[must_use]
    pub fn local_anchor2(&self) -> Vec2 {
        self.data.local_anchor2()
    }

    /// Sets the joint’s anchor, expressed in the local-space of the second rigid-body.
    pub fn set_local_anchor2(&mut self, anchor2: Vec2) -> &mut Self {
        self.data.set_local_anchor2(anchor2);
        self
    }

    /// The principal axis of the joint, expressed in the local-space of the first rigid-body.
    #[must_use]
    pub fn local_axis1(&self) -> Vec2 {
        self.data.local_axis1()
    }

    /// Sets the principal axis of the joint, expressed in the local-space of the first rigid-body.
    pub fn set_local_axis1(&mut self, axis1: Vec2) -> &mut Self {
        self.data.set_local_axis1(axis1);
        self
    }

    /// The principal axis of the joint, expressed in the local-space of the second rigid-body.
    #[must_use]
    pub fn local_axis2(&self) -> Vec2 {
        self.data.local_axis2()
    }

    /// Sets the principal axis of the joint, expressed in the local-space of the second rigid-body.
    pub fn set_local_axis2(&mut self, axis2: Vec2) -> &mut Self {
        self.data.set_local_axis2(axis2);
        self
    }

    /// The motor affecting the joint’s translational degree of freedom.
    #[must_use]
    pub fn motor(&self) -> Option<&JointMotor> {
        self.data.motor(JointAxis::X)
    }

    /// Set the spring-like model used by the motor to reach the desired target velocity and position.
    pub fn set_motor_model(&mut self, model: MotorModel) -> &mut Self {
        self.data.set_motor_model(JointAxis::X, model);
        self
    }

    /// Sets the target velocity this motor needs to reach.
    pub fn set_motor_velocity(&mut self, target_vel: Real, factor: Real) -> &mut Self {
        self.data
            .set_motor_velocity(JointAxis::X, target_vel, factor);
        self
    }

    /// Sets the target angle this motor needs to reach.
    pub fn set_motor_position(
        &mut self,
        target_pos: Real,
        stiffness: Real,
        damping: Real,
    ) -> &mut Self {
        self.data
            .set_motor_position(JointAxis::X, target_pos, stiffness, damping);
        self
    }

    /// Configure both the target angle and target velocity of the motor.
    pub fn set_motor(
        &mut self,
        target_pos: Real,
        target_vel: Real,
        stiffness: Real,
        damping: Real,
    ) -> &mut Self {
        self.data
            .set_motor(JointAxis::X, target_pos, target_vel, stiffness, damping);
        self
    }

    /// Sets the maximum force the motor can deliver.
    pub fn set_motor_max_force(&mut self, max_force: Real) -> &mut Self {
        self.data.set_motor_max_force(JointAxis::X, max_force);
        self
    }

    /// The limit distance attached bodies can translate along the joint’s principal axis.
    #[must_use]
    pub fn limits(&self) -> Option<&JointLimits<Real>> {
        self.data.limits(JointAxis::X)
    }

    /// Sets the `[min,max]` limit distances attached bodies can translate along the joint’s principal axis.
    pub fn set_limits(&mut self, limits: [Real; 2]) -> &mut Self {
        self.data.set_limits(JointAxis::X, limits);
        self
    }
}
impl From<PrismaticJoint> for GenericJoint {
    fn from(val: PrismaticJoint) -> Self {
        val.data
    }
}

/// Create prismatic joints using the builder pattern.
///
/// A prismatic joint locks all relative motion except for translations along the joint’s principal axis.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct PrismaticJointBuilder(pub PrismaticJoint);

impl PrismaticJointBuilder {
    /// Creates a new builder for prismatic joints.
    ///
    /// This axis is expressed in the local-space of both rigid-bodies.
    pub fn new(axis: Vec2) -> Self {
        Self(PrismaticJoint::new(axis))
    }

    /// Sets whether contacts between the attached rigid-bodies are enabled.
    #[must_use]
    pub fn contacts_enabled(mut self, enabled: bool) -> Self {
        self.0.set_contacts_enabled(enabled);
        self
    }

    /// Sets the joint’s anchor, expressed in the local-space of the first rigid-body.
    #[must_use]
    pub fn local_anchor1(mut self, anchor1: Vec2) -> Self {
        self.0.set_local_anchor1(anchor1);
        self
    }

    /// Sets the joint’s anchor, expressed in the local-space of the second rigid-body.
    #[must_use]
    pub fn local_anchor2(mut self, anchor2: Vec2) -> Self {
        self.0.set_local_anchor2(anchor2);
        self
    }

    /// Sets the principal axis of the joint, expressed in the local-space of the first rigid-body.
    #[must_use]
    pub fn local_axis1(mut self, axis1: Vec2) -> Self {
        self.0.set_local_axis1(axis1);
        self
    }

    /// Sets the principal axis of the joint, expressed in the local-space of the second rigid-body.
    #[must_use]
    pub fn local_axis2(mut self, axis2: Vec2) -> Self {
        self.0.set_local_axis2(axis2);
        self
    }

    /// Set the spring-like model used by the motor to reach the desired target velocity and position.
    #[must_use]
    pub fn motor_model(mut self, model: MotorModel) -> Self {
        self.0.set_motor_model(model);
        self
    }

    /// Sets the target velocity this motor needs to reach.
    #[must_use]
    pub fn motor_velocity(mut self, target_vel: Real, factor: Real) -> Self {
        self.0.set_motor_velocity(target_vel, factor);
        self
    }

    /// Sets the target angle this motor needs to reach.
    #[must_use]
    pub fn motor_position(mut self, target_pos: Real, stiffness: Real, damping: Real) -> Self {
        self.0.set_motor_position(target_pos, stiffness, damping);
        self
    }

    /// Configure both the target angle and target velocity of the motor.
    #[must_use]
    pub fn set_motor(
        mut self,
        target_pos: Real,
        target_vel: Real,
        stiffness: Real,
        damping: Real,
    ) -> Self {
        self.0.set_motor(target_pos, target_vel, stiffness, damping);
        self
    }

    /// Sets the maximum force the motor can deliver.
    #[must_use]
    pub fn motor_max_force(mut self, max_force: Real) -> Self {
        self.0.set_motor_max_force(max_force);
        self
    }

    /// Sets the `[min,max]` limit distances attached bodies can translate along the joint’s principal axis.
    #[must_use]
    pub fn limits(mut self, limits: [Real; 2]) -> Self {
        self.0.set_limits(limits);
        self
    }

    /// Builds the prismatic joint.
    #[must_use]
    pub fn build(self) -> PrismaticJoint {
        self.0
    }
}
impl From<PrismaticJointBuilder> for GenericJoint {
    fn from(val: PrismaticJointBuilder) -> Self {
        val.0.into()
    }
}

// RevoluteJoint

#[derive(Copy, Clone, Debug, PartialEq)]
#[repr(transparent)]
/// A revolute joint, locks all relative motion except for rotation along the joint’s principal axis.
pub struct RevoluteJoint {
    /// The underlying joint data.
    pub data: GenericJoint,
}

impl RevoluteJoint {
    /// Creates a new revolute joint allowing only relative rotations.
    pub fn new() -> Self {
        let data = GenericJointBuilder::new(JointAxesMask::LOCKED_REVOLUTE_AXES);
        Self { data: data.build() }
    }

    /// Creates a new revolute joint allowing only relative rotations along the specified axis.
    ///
    /// This axis is expressed in the local-space of both rigid-bodies.
    // #[cfg(feature = "dim3")]
    // pub fn new(axis: UnitVector<Real>) -> Self {
    //     let data = GenericJointBuilder::new(JointAxesMask::LOCKED_REVOLUTE_AXES)
    //         .local_axis1(axis)
    //         .local_axis2(axis)
    //         .build();
    //     Self { data }
    // }

    /// The underlying generic joint.
    pub fn data(&self) -> &GenericJoint {
        &self.data
    }

    /// Are contacts between the attached rigid-bodies enabled?
    pub fn contacts_enabled(&self) -> bool {
        self.data.contacts_enabled()
    }

    /// Sets whether contacts between the attached rigid-bodies are enabled.
    pub fn set_contacts_enabled(&mut self, enabled: bool) -> &mut Self {
        self.data.set_contacts_enabled(enabled);
        self
    }

    /// The joint’s anchor, expressed in the local-space of the first rigid-body.
    #[must_use]
    pub fn local_anchor1(&self) -> Vec2 {
        self.data.local_anchor1()
    }

    /// Sets the joint’s anchor, expressed in the local-space of the first rigid-body.
    pub fn set_local_anchor1(&mut self, anchor1: Vec2) -> &mut Self {
        self.data.set_local_anchor1(anchor1);
        self
    }

    /// The joint’s anchor, expressed in the local-space of the second rigid-body.
    #[must_use]
    pub fn local_anchor2(&self) -> Vec2 {
        self.data.local_anchor2()
    }

    /// Sets the joint’s anchor, expressed in the local-space of the second rigid-body.
    pub fn set_local_anchor2(&mut self, anchor2: Vec2) -> &mut Self {
        self.data.set_local_anchor2(anchor2);
        self
    }

    /// The motor affecting the joint’s rotational degree of freedom.
    #[must_use]
    pub fn motor(&self) -> Option<&JointMotor> {
        self.data.motor(JointAxis::AngX)
    }

    /// Set the spring-like model used by the motor to reach the desired target velocity and position.
    pub fn set_motor_model(&mut self, model: MotorModel) -> &mut Self {
        self.data.set_motor_model(JointAxis::AngX, model);
        self
    }

    /// Sets the target velocity this motor needs to reach.
    pub fn set_motor_velocity(&mut self, target_vel: Real, factor: Real) -> &mut Self {
        self.data
            .set_motor_velocity(JointAxis::AngX, target_vel, factor);
        self
    }

    /// Sets the target angle this motor needs to reach.
    pub fn set_motor_position(
        &mut self,
        target_pos: Real,
        stiffness: Real,
        damping: Real,
    ) -> &mut Self {
        self.data
            .set_motor_position(JointAxis::AngX, target_pos, stiffness, damping);
        self
    }

    /// Configure both the target angle and target velocity of the motor.
    pub fn set_motor(
        &mut self,
        target_pos: Real,
        target_vel: Real,
        stiffness: Real,
        damping: Real,
    ) -> &mut Self {
        self.data
            .set_motor(JointAxis::AngX, target_pos, target_vel, stiffness, damping);
        self
    }

    /// Sets the maximum force the motor can deliver.
    pub fn set_motor_max_force(&mut self, max_force: Real) -> &mut Self {
        self.data.set_motor_max_force(JointAxis::AngX, max_force);
        self
    }

    /// The limit angle attached bodies can translate along the joint’s principal axis.
    #[must_use]
    pub fn limits(&self) -> Option<&JointLimits<Real>> {
        self.data.limits(JointAxis::AngX)
    }

    /// Sets the `[min,max]` limit angle attached bodies can translate along the joint’s principal axis.
    pub fn set_limits(&mut self, limits: [Real; 2]) -> &mut Self {
        self.data.set_limits(JointAxis::AngX, limits);
        self
    }
}

impl Default for RevoluteJoint {
    fn default() -> Self {
        Self::new()
    }
}

impl From<RevoluteJoint> for GenericJoint {
    fn from(val: RevoluteJoint) -> Self {
        val.data
    }
}

/// Create revolute joints using the builder pattern.
///
/// A revolute joint locks all relative motion except for rotations along the joint’s principal axis.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct RevoluteJointBuilder(pub RevoluteJoint);

impl RevoluteJointBuilder {
    /// Creates a new revolute joint builder.
    pub fn new() -> Self {
        Self(RevoluteJoint::new())
    }

    // /// Creates a new revolute joint builder, allowing only relative rotations along the specified axis.
    // ///
    // /// This axis is expressed in the local-space of both rigid-bodies.
    // #[cfg(feature = "dim3")]
    // pub fn new(axis: UnitVector<Real>) -> Self {
    //     Self(RevoluteJoint::new(axis))
    // }

    /// Sets whether contacts between the attached rigid-bodies are enabled.
    #[must_use]
    pub fn contacts_enabled(mut self, enabled: bool) -> Self {
        self.0.set_contacts_enabled(enabled);
        self
    }

    /// Sets the joint’s anchor, expressed in the local-space of the first rigid-body.
    #[must_use]
    pub fn local_anchor1(mut self, anchor1: Vec2) -> Self {
        self.0.set_local_anchor1(anchor1);
        self
    }

    /// Sets the joint’s anchor, expressed in the local-space of the second rigid-body.
    #[must_use]
    pub fn local_anchor2(mut self, anchor2: Vec2) -> Self {
        self.0.set_local_anchor2(anchor2);
        self
    }

    /// Set the spring-like model used by the motor to reach the desired target velocity and position.
    #[must_use]
    pub fn motor_model(mut self, model: MotorModel) -> Self {
        self.0.set_motor_model(model);
        self
    }

    /// Sets the target velocity this motor needs to reach.
    #[must_use]
    pub fn motor_velocity(mut self, target_vel: Real, factor: Real) -> Self {
        self.0.set_motor_velocity(target_vel, factor);
        self
    }

    /// Sets the target angle this motor needs to reach.
    #[must_use]
    pub fn motor_position(mut self, target_pos: Real, stiffness: Real, damping: Real) -> Self {
        self.0.set_motor_position(target_pos, stiffness, damping);
        self
    }

    /// Configure both the target angle and target velocity of the motor.
    #[must_use]
    pub fn motor(
        mut self,
        target_pos: Real,
        target_vel: Real,
        stiffness: Real,
        damping: Real,
    ) -> Self {
        self.0.set_motor(target_pos, target_vel, stiffness, damping);
        self
    }

    /// Sets the maximum force the motor can deliver.
    #[must_use]
    pub fn motor_max_force(mut self, max_force: Real) -> Self {
        self.0.set_motor_max_force(max_force);
        self
    }

    /// Sets the `[min,max]` limit angles attached bodies can rotate along the joint’s principal axis.
    #[must_use]
    pub fn limits(mut self, limits: [Real; 2]) -> Self {
        self.0.set_limits(limits);
        self
    }

    /// Builds the revolute joint.
    #[must_use]
    pub fn build(self) -> RevoluteJoint {
        self.0
    }
}

impl Default for RevoluteJointBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl From<RevoluteJointBuilder> for GenericJoint {
    fn from(val: RevoluteJointBuilder) -> Self {
        val.0.into()
    }
}

// Rope Joints

#[derive(Copy, Clone, Debug, PartialEq)]
#[repr(transparent)]
/// A rope joint, limits the maximum distance between two bodies
pub struct RopeJoint {
    /// The underlying joint data.
    pub data: GenericJoint,
}

impl RopeJoint {
    /// Creates a new rope joint limiting the max distance between to bodies
    pub fn new() -> Self {
        let data = GenericJointBuilder::new(JointAxesMask::FREE_FIXED_AXES)
            .coupled_axes(JointAxesMask::LIN_AXES)
            .build();
        Self { data }
    }

    /// The underlying generic joint.
    pub fn data(&self) -> &GenericJoint {
        &self.data
    }

    /// Are contacts between the attached rigid-bodies enabled?
    pub fn contacts_enabled(&self) -> bool {
        self.data.contacts_enabled()
    }

    /// Sets whether contacts between the attached rigid-bodies are enabled.
    pub fn set_contacts_enabled(&mut self, enabled: bool) -> &mut Self {
        self.data.set_contacts_enabled(enabled);
        self
    }

    /// The joint’s anchor, expressed in the local-space of the first rigid-body.
    #[must_use]
    pub fn local_anchor1(&self) -> Vec2 {
        self.data.local_anchor1()
    }

    /// Sets the joint’s anchor, expressed in the local-space of the first rigid-body.
    pub fn set_local_anchor1(&mut self, anchor1: Vec2) -> &mut Self {
        self.data.set_local_anchor1(anchor1);
        self
    }

    /// The joint’s anchor, expressed in the local-space of the second rigid-body.
    #[must_use]
    pub fn local_anchor2(&self) -> Vec2 {
        self.data.local_anchor2()
    }

    /// Sets the joint’s anchor, expressed in the local-space of the second rigid-body.
    pub fn set_local_anchor2(&mut self, anchor2: Vec2) -> &mut Self {
        self.data.set_local_anchor2(anchor2);
        self
    }

    /// The principal axis of the joint, expressed in the local-space of the first rigid-body.
    #[must_use]
    pub fn local_axis1(&self) -> Vec2 {
        self.data.local_axis1()
    }

    /// Sets the principal axis of the joint, expressed in the local-space of the first rigid-body.
    pub fn set_local_axis1(&mut self, axis1: Vec2) -> &mut Self {
        self.data.set_local_axis1(axis1);
        self
    }

    /// The principal axis of the joint, expressed in the local-space of the second rigid-body.
    #[must_use]
    pub fn local_axis2(&self) -> Vec2 {
        self.data.local_axis2()
    }

    /// Sets the principal axis of the joint, expressed in the local-space of the second rigid-body.
    pub fn set_local_axis2(&mut self, axis2: Vec2) -> &mut Self {
        self.data.set_local_axis2(axis2);
        self
    }

    /// The motor affecting the joint’s translational degree of freedom.
    #[must_use]
    pub fn motor(&self, axis: JointAxis) -> Option<&JointMotor> {
        self.data.motor(axis)
    }

    /// Set the spring-like model used by the motor to reach the desired target velocity and position.
    pub fn set_motor_model(&mut self, model: MotorModel) -> &mut Self {
        self.data.set_motor_model(JointAxis::X, model);
        self.data.set_motor_model(JointAxis::Y, model);
        // #[cfg(feature = "dim3")]
        // self.data.set_motor_model(JointAxis::Z, model);
        self
    }

    /// Sets the target velocity this motor needs to reach.
    pub fn set_motor_velocity(&mut self, target_vel: Real, factor: Real) -> &mut Self {
        self.data
            .set_motor_velocity(JointAxis::X, target_vel, factor);
        self.data
            .set_motor_velocity(JointAxis::Y, target_vel, factor);
        // #[cfg(feature = "dim3")]
        // self.data
        //     .set_motor_velocity(JointAxis::Z, target_vel, factor);
        self
    }

    /// Sets the target angle this motor needs to reach.
    pub fn set_motor_position(
        &mut self,
        target_pos: Real,
        stiffness: Real,
        damping: Real,
    ) -> &mut Self {
        self.data
            .set_motor_position(JointAxis::X, target_pos, stiffness, damping);
        self.data
            .set_motor_position(JointAxis::Y, target_pos, stiffness, damping);
        // #[cfg(feature = "dim3")]
        // self.data
        //     .set_motor_position(JointAxis::Z, target_pos, stiffness, damping);
        self
    }

    /// Configure both the target angle and target velocity of the motor.
    pub fn set_motor(
        &mut self,
        target_pos: Real,
        target_vel: Real,
        stiffness: Real,
        damping: Real,
    ) -> &mut Self {
        self.data
            .set_motor(JointAxis::X, target_pos, target_vel, stiffness, damping);
        self.data
            .set_motor(JointAxis::Y, target_pos, target_vel, stiffness, damping);
        // #[cfg(feature = "dim3")]
        // self.data
        //     .set_motor(JointAxis::Y, target_pos, target_vel, stiffness, damping);
        self
    }

    /// Sets the maximum force the motor can deliver.
    pub fn set_motor_max_force(&mut self, max_force: Real) -> &mut Self {
        self.data.set_motor_max_force(JointAxis::X, max_force);
        self.data.set_motor_max_force(JointAxis::Y, max_force);
        // #[cfg(feature = "dim3")]
        // self.data.set_motor_max_force(JointAxis::Z, max_force);
        self
    }

    /// The limit maximum distance attached bodies can translate.
    #[must_use]
    pub fn limits(&self, axis: JointAxis) -> Option<&JointLimits<Real>> {
        self.data.limits(axis)
    }

    /// Sets the `[min,max]` limit distances attached bodies can translate.
    pub fn set_limits(&mut self, limits: [Real; 2]) -> &mut Self {
        self.data.set_limits(JointAxis::X, limits);
        self.data.set_limits(JointAxis::Y, limits);
        // #[cfg(feature = "dim3")]
        // self.data.set_limits(JointAxis::Z, limits);
        self
    }
}

impl Default for RopeJoint {
    fn default() -> Self {
        Self::new()
    }
}

impl From<RopeJoint> for GenericJoint {
    fn from(val: RopeJoint) -> Self {
        val.data
    }
}

/// Create rope joints using the builder pattern.
///
/// A rope joint, limits the maximum distance between two bodies.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct RopeJointBuilder(pub RopeJoint);

impl RopeJointBuilder {
    /// Creates a new builder for rope joints.
    ///
    /// This axis is expressed in the local-space of both rigid-bodies.
    pub fn new() -> Self {
        Self(RopeJoint::new())
    }

    /// Sets whether contacts between the attached rigid-bodies are enabled.
    #[must_use]
    pub fn contacts_enabled(mut self, enabled: bool) -> Self {
        self.0.set_contacts_enabled(enabled);
        self
    }

    /// Sets the joint’s anchor, expressed in the local-space of the first rigid-body.
    #[must_use]
    pub fn local_anchor1(mut self, anchor1: Vec2) -> Self {
        self.0.set_local_anchor1(anchor1);
        self
    }

    /// Sets the joint’s anchor, expressed in the local-space of the second rigid-body.
    #[must_use]
    pub fn local_anchor2(mut self, anchor2: Vec2) -> Self {
        self.0.set_local_anchor2(anchor2);
        self
    }

    /// Sets the principal axis of the joint, expressed in the local-space of the first rigid-body.
    #[must_use]
    pub fn local_axis1(mut self, axis1: Vec2) -> Self {
        self.0.set_local_axis1(axis1);
        self
    }

    /// Sets the principal axis of the joint, expressed in the local-space of the second rigid-body.
    #[must_use]
    pub fn local_axis2(mut self, axis2: Vec2) -> Self {
        self.0.set_local_axis2(axis2);
        self
    }

    /// Set the spring-like model used by the motor to reach the desired target velocity and position.
    #[must_use]
    pub fn motor_model(mut self, model: MotorModel) -> Self {
        self.0.set_motor_model(model);
        self
    }

    /// Sets the target velocity this motor needs to reach.
    #[must_use]
    pub fn motor_velocity(mut self, target_vel: Real, factor: Real) -> Self {
        self.0.set_motor_velocity(target_vel, factor);
        self
    }

    /// Sets the target angle this motor needs to reach.
    #[must_use]
    pub fn motor_position(mut self, target_pos: Real, stiffness: Real, damping: Real) -> Self {
        self.0.set_motor_position(target_pos, stiffness, damping);
        self
    }

    /// Configure both the target angle and target velocity of the motor.
    #[must_use]
    pub fn set_motor(
        mut self,
        target_pos: Real,
        target_vel: Real,
        stiffness: Real,
        damping: Real,
    ) -> Self {
        self.0.set_motor(target_pos, target_vel, stiffness, damping);
        self
    }

    /// Sets the maximum force the motor can deliver.
    #[must_use]
    pub fn motor_max_force(mut self, max_force: Real) -> Self {
        self.0.set_motor_max_force(max_force);
        self
    }

    /// Sets the `[min,max]` limit distances attached bodies can translate.
    #[must_use]
    pub fn limits(mut self, limits: [Real; 2]) -> Self {
        self.0.set_limits(limits);
        self
    }

    /// Builds the rope joint.
    #[must_use]
    pub fn build(self) -> RopeJoint {
        self.0
    }
}
impl From<RopeJointBuilder> for GenericJoint {
    fn from(val: RopeJointBuilder) -> Self {
        val.0.into()
    }
}
impl Default for RopeJointBuilder {
    fn default() -> Self {
        Self::new()
    }
}
