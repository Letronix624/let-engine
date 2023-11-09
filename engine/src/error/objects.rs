//! Object based errors.

//use std::{error::Error, io::ErrorKind};
use thiserror::Error;

/// This error gets returned when the layer that gets specified when an object needs to get added
/// doesn't exit in the objects list anymore.
#[derive(Error, Debug)]
#[error("No Layer found")]
pub struct NoLayerError;

/// This error gets returned when one of the objects input into register_joint doesn't have a rigid body to attach the joint to.
#[derive(Error, Debug)]
#[error("One of the objects does not have a rigid body")]
pub struct NoRigidBodyError;

#[derive(Error, Debug)]
#[error("This joint was not found in this layer.")]
pub struct NoJointError;

/// Errors that happen in object and layer functions.
#[derive(Error, Debug)]
pub enum ObjectError {
    /// The object you are trying to use is not initialized to a layer.
    #[error("This object is not initialized to a layer.")]
    Uninitialized,
    /// The parent of the object you are trying to use is not initialized to the same layer or not initialized at all.
    #[error("The parent is not initialized to the same layer.")]
    UninitializedParent,
    /// The move operation has failed.
    #[error("This object can not be moved to this position:\n{0}")]
    Move(String),
}
