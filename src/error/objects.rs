//use std::{error::Error, io::ErrorKind};
use thiserror::Error;

/// This error gets returned when the game can't find a matching object in the game object list
/// as the object that got input as a parent.
#[derive(Error, Debug)]
#[error("No object found")]
pub struct NoObjectError;

/// This error gets returned when the parent of the object that wants to get added isn't
/// in the game object list anymore.
#[derive(Error, Debug)]
#[error("This parent for this object isn't added to the objects list.")]
pub struct NoParentError;

/// This error gets returned when the layer that gets specified when an object needs to get added
/// doesn't exit in the objects list anymore.
#[derive(Error, Debug)]
#[error("No Layer found")]
pub struct NoLayerError;
