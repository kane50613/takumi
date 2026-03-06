/// Background and color drawing functions
mod background_drawing;
mod blend;
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
mod webp;
mod write;

use std::{collections::HashMap, rc::Rc, sync::Arc};

use taffy::Size;

pub(crate) use background_drawing::*;
pub(crate) use blend::*;
pub(crate) use canvas::*;
pub(crate) use components::*;
pub(crate) use debug_drawing::*;
pub(crate) use image_drawing::*;
pub use render::*;
pub(crate) use text_drawing::*;
pub use write::*;

#[cfg(feature = "css_stylesheet_parsing")]
use crate::layout::style::selector::StyleSheet;
use crate::{
  GlobalContext,
  layout::{
    Viewport,
    style::{Affine, CalcArena, Color, ResolvedStyle},
  },
  resources::image::ImageSource,
};

/// The sizing context used for length value resolving.
#[derive(Clone)]
pub(crate) struct Sizing {
  /// The viewport for the image renderer.
  pub(crate) viewport: Viewport,
  /// The nearest query container size (content box) in device pixels.
  pub(crate) container_size: Size<Option<f32>>,
  /// The font size in pixels.
  pub(crate) font_size: f32,
  /// The calc arena shared by the current layout tree.
  pub(crate) calc_arena: Rc<CalcArena>,
}

impl Sizing {
  pub(crate) fn query_container_width(&self) -> f32 {
    self
      .container_size
      .width
      .unwrap_or(self.viewport.width.unwrap_or_default() as f32)
  }

  pub(crate) fn query_container_height(&self) -> f32 {
    self
      .container_size
      .height
      .unwrap_or(self.viewport.height.unwrap_or_default() as f32)
  }
}

/// The absolute animation time used when resolving animated styles.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RenderTime {
  /// The current time on the global timeline in milliseconds.
  pub time_ms: u64,
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
  pub(crate) style: Box<ResolvedStyle>,
  /// The active time for animation sampling.
  pub(crate) time: RenderTime,
  /// Whether to draw debug borders.
  pub(crate) draw_debug_border: bool,
  /// The resources fetched externally.
  pub(crate) fetched_resources: HashMap<Arc<str>, Arc<ImageSource>>,
  /// The stylesheets to apply before layout/rendering.
  #[cfg(feature = "css_stylesheet_parsing")]
  pub(crate) stylesheets: Rc<[StyleSheet]>,
}

impl<'g> RenderContext<'g> {
  #[cfg(feature = "css_stylesheet_parsing")]
  pub(crate) fn new<I: IntoIterator<Item = StyleSheet>>(
    global: &'g GlobalContext,
    viewport: Viewport,
    fetched_resources: HashMap<Arc<str>, Arc<ImageSource>>,
    stylesheets: I,
    time: RenderTime,
  ) -> Self {
    Self {
      global,
      sizing: Sizing {
        viewport,
        container_size: Size::NONE,
        font_size: viewport.font_size,
        calc_arena: Rc::new(CalcArena::default()),
      },
      transform: Affine::IDENTITY,
      current_color: Color::black(),
      style: Box::default(),
      time,
      draw_debug_border: false,
      fetched_resources,
      stylesheets: Rc::from_iter(stylesheets),
    }
  }

  #[cfg(not(feature = "css_stylesheet_parsing"))]
  pub(crate) fn new(
    global: &'g GlobalContext,
    viewport: Viewport,
    fetched_resources: HashMap<Arc<str>, Arc<ImageSource>>,
    time: RenderTime,
  ) -> Self {
    Self {
      global,
      sizing: Sizing {
        viewport,
        container_size: Size::NONE,
        font_size: viewport.font_size,
        calc_arena: Rc::new(CalcArena::default()),
      },
      transform: Affine::IDENTITY,
      current_color: Color::black(),
      style: Box::default(),
      time,
      draw_debug_border: false,
      fetched_resources,
    }
  }

  /// Internal, only used in tests.
  #[cfg(test)]
  pub(crate) fn new_test(global: &'g GlobalContext, viewport: Viewport) -> Self {
    #[cfg(feature = "css_stylesheet_parsing")]
    {
      use std::iter::empty;
      Self::new(
        global,
        viewport,
        Default::default(),
        empty(),
        RenderTime::default(),
      )
    }
    #[cfg(not(feature = "css_stylesheet_parsing"))]
    {
      Self::new(global, viewport, Default::default(), RenderTime::default())
    }
  }
}

#[inline(always)]
pub(crate) fn fast_div_255(v: u32) -> u8 {
  fast_div_255_u32(v) as u8
}

/// Fast division by 255 by approximating `v / 255` using bitwise operations.
#[inline(always)]
pub(crate) fn fast_div_255_u32(v: u32) -> u32 {
  ((v.wrapping_add(128).wrapping_add(v >> 8)) >> 8).min(255)
}
