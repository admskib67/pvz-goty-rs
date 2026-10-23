//! Engine-independent reimplementations of the parts of PopCap's
//! SexyAppFramework and TodLib that PvZ is built on.
//!
//! Nothing here depends on Bevy, so it can be unit-tested directly and reused
//! by tools.

pub mod data_array;
pub mod files;
pub mod font;
pub mod image;
pub mod matrix;
pub mod rand;
pub mod reanim;
pub mod resources;
pub mod strings;
pub mod tod;
