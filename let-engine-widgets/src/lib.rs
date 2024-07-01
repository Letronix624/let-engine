//! This library only works if the client feature of the let engine is active.

pub mod labels;

/// Run this at the start of every update to make sure the widgets all work correctly.
pub fn update() {
    labels::LABELIFIER.lock().update().unwrap();
}

/// Clears the font cache and resizes the pixel buffer. Shaves memory after heavy label use.
///
/// Should be called from time to time for example after loading screens or when memory usage goes to high.
pub fn clear_cache() {
    labels::LABELIFIER.lock().clear_cache();
}
