/// Background and color drawing functions
mod background_drawing;
/// Canvas operations and image blending
mod canvas;
mod components;
/// Debug drawing utilities
mod debug_drawing;
/// Image drawing functions
mod image_drawing;
/// Main image renderer and viewport management
mod render;
/// Text drawing functions
mod text_drawing;
// Transform (node subtree) rendering helpers
mod transform;

pub use background_drawing::*;
pub use canvas::*;
pub use components::*;
pub use debug_drawing::*;
pub use image_drawing::*;
pub use render::*;
pub use text_drawing::*;

use crate::{GlobalContext, layout::Viewport};

/// The context for the image renderer.
#[derive(Clone, Copy)]
pub struct RenderContext<'ctx> {
  /// The global context.
  pub global: &'ctx GlobalContext,
  /// The viewport for the image renderer.
  pub viewport: Viewport,
  /// The font size in pixels, used for em and rem units.
  pub parent_font_size: f32,
}
