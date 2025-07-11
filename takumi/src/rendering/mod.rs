//! Rendering system for the takumi image generation library.
//!
//! This module contains all rendering-related functionality including:
//! - Canvas operations and image blending
//! - Drawing utilities for shapes and primitives
//! - Image renderer and viewport management
//! - Performance-optimized rendering operations

/// Background and color drawing functions
pub mod background_drawing;
/// Canvas operations and image blending
pub mod canvas;
/// Debug drawing utilities
pub mod debug_drawing;
/// Image drawing functions
pub mod image_drawing;
/// Main image renderer and viewport management
pub mod renderer;
/// Text drawing functions
pub mod text_drawing;

pub use background_drawing::*;
pub use canvas::*;
pub use debug_drawing::*;
pub use image_drawing::*;
pub use renderer::*;
pub use text_drawing::*;
