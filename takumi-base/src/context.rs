use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;

use taffy::Size;

use crate::{
  GlobalContext,
  layout::{
    Viewport,
    style::{Affine, CalcArena, Color, ComputedStyle, SizingContext, StyleSheet},
  },
  resources::image::ImageSource,
};

/// The context for the internal rendering. You should not construct this directly.
#[derive(Clone)]
pub struct RenderContext<'g> {
  /// The global context.
  pub global: &'g GlobalContext,
  /// The scale factor for the image renderer.
  pub transform: Affine,
  /// The sizing context.
  pub sizing: SizingContext,
  /// What the `currentColor` value is resolved to.
  pub current_color: Color,
  /// The style after inheritance.
  pub style: Box<ComputedStyle>,
  /// The active time for animation sampling.
  pub time: u64,
  /// Whether to draw debug borders.
  pub draw_debug_border: bool,
  /// The resources fetched externally.
  pub fetched_resources: HashMap<Arc<str>, ImageSource>,
  /// The stylesheets to apply before layout/rendering.
  pub stylesheet: Rc<StyleSheet>,
}

impl<'g> RenderContext<'g> {
  pub fn new(
    global: &'g GlobalContext,
    viewport: Viewport,
    fetched_resources: HashMap<Arc<str>, ImageSource>,
    stylesheet: Rc<StyleSheet>,
    time: u64,
  ) -> Self {
    Self {
      global,
      sizing: SizingContext {
        viewport,
        container_size: Size::NONE,
        font_size: viewport.font_size,
        root_font_size: None,
        line_height: 0.0,
        root_line_height: None,
        calc_arena: Rc::new(CalcArena::default()),
      },
      transform: Affine::IDENTITY,
      current_color: Color::black(),
      style: Box::default(),
      time,
      draw_debug_border: false,
      fetched_resources,
      stylesheet,
    }
  }

  /// Internal, only used in tests.
  pub fn new_test(global: &'g GlobalContext, viewport: Viewport) -> Self {
    Self::new(global, viewport, Default::default(), Default::default(), 0)
  }

  pub fn from_parent(
    parent: &Self,
    style: ComputedStyle,
    sizing: SizingContext,
    current_color: Color,
  ) -> Self {
    Self {
      global: parent.global,
      transform: parent.transform,
      style: Box::new(style),
      current_color,
      time: parent.time,
      draw_debug_border: parent.draw_debug_border,
      fetched_resources: parent.fetched_resources.clone(),
      sizing,
      stylesheet: parent.stylesheet.clone(),
    }
  }
}
