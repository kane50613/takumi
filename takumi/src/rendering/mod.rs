/// Background and color drawing functions
mod background_drawing;
/// Canvas operations and image blending
mod canvas;
mod components;
/// Debug drawing utilities
mod debug_drawing;
/// Image drawing functions
mod image_drawing;
pub(crate) mod inline_drawing;
/// Main image renderer and viewport management
mod render;
/// Text drawing functions
mod text_drawing;
mod write;

use std::{collections::HashMap, sync::Arc};

pub(crate) use background_drawing::*;
pub(crate) use canvas::*;
pub(crate) use components::*;
pub(crate) use debug_drawing::*;
pub(crate) use image_drawing::*;
pub use render::*;
pub(crate) use text_drawing::*;
pub use write::*;

use crate::{
  GlobalContext,
  layout::{
    Viewport,
    style::{Affine, Color, InheritedStyle},
  },
  resources::image::ImageSource,
};

/// The sizing context used for length value resolving.
#[derive(Clone, Copy)]
pub(crate) struct Sizing {
  /// The viewport for the image renderer.
  pub(crate) viewport: Viewport,
  /// The font size in pixels.
  pub(crate) font_size: f32,
}

/// The context for the internal rendering. You should not construct this directly.
#[derive(Clone)]
pub struct RenderContext<'g> {
  /// The global context.
  pub(crate) global: &'g GlobalContext,
  /// The scale factor for the image renderer.
  pub(crate) transform: Affine,
  /// The sizing context.
  pub(crate) sizing: Sizing,
  /// What the `currentColor` value is resolved to.
  pub(crate) current_color: Color,
  /// The style after inheritance.
  pub(crate) style: InheritedStyle,
  /// Whether to draw debug borders.
  pub(crate) draw_debug_border: bool,
  /// The resources fetched externally.
  pub(crate) fetched_resources: HashMap<Arc<str>, Arc<ImageSource>>,
}

impl<'g> RenderContext<'g> {
  pub(crate) fn new(
    global: &'g GlobalContext,
    viewport: Viewport,
    fetched_resources: HashMap<Arc<str>, Arc<ImageSource>>,
  ) -> Self {
    Self {
      global,
      sizing: Sizing {
        viewport,
        font_size: viewport.font_size,
      },
      transform: Affine::IDENTITY,
      current_color: Color::black(),
      style: InheritedStyle::default(),
      draw_debug_border: false,
      fetched_resources,
    }
  }
}
