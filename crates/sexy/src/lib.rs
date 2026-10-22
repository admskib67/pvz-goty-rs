//! Engine-independent reimplementations of the parts of PopCap's
//! SexyAppFramework and TodLib that PvZ is built on.
//!
//! Nothing here depends on Bevy, so it can be unit-tested directly and reused
//! by tools.

pub mod files;
pub mod image;
pub mod resources;
pub mod tod;
