//use std::{error::Error, io::ErrorKind};
use thiserror::Error;

#[derive(Error, Debug)]
#[error("No object found")]
pub struct NoObjectError;

#[derive(Error, Debug)]
#[error("This object already exists in the objects list.")]
pub struct ObjectExistsError;

#[derive(Error, Debug)]
#[error("This parent for this object isn't added to the objects list.")]
pub struct NoParentError;
