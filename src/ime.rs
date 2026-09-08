//! Shared IME engine and GPU debug-companion entry points.

pub mod companion;
mod core_engine;
mod core_helpers;
mod core_icons_a;
mod core_icons_b;
mod prediction;
pub mod settings;

pub use prediction::PredictionStatus;

pub use core_engine::*;
pub(crate) use core_helpers::*;
pub(crate) use core_icons_a::*;
pub(crate) use core_icons_b::*;

#[cfg(feature = "gpu")]
pub mod gpu;
