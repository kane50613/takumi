use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;

use taffy::Size;

use crate::{
  Fonts,
  layout::{
    Viewport,
    style::{Affine, CalcArena, Color, ComputedStyle, SizingContext, StyleSheet},
  },
  resources::image::ImageSource,
};

/// The context for the internal rendering. You should not construct this directly.
#[derive(Clone)]
pub struct RenderContext<'g> {
  /// The font registry shared across renders.
  pub fonts: &'g Fonts,
  /// This render's working copy of the parley context (parley queries need `&mut`), shared
  /// down the node tree. The registry itself stays immutable.
  pub font_cx: Rc<RefCell<parley::FontContext>>,
  /// The fallback family chain for this render, appended to every run's font stack.
  pub fallback_families: Rc<[String]>,
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
  pub images: HashMap<Arc<str>, ImageSource>,
  /// The stylesheets to apply before layout/rendering.
  pub stylesheet: Rc<StyleSheet>,
}

impl<'g> RenderContext<'g> {
  pub fn new(
    fonts: &'g Fonts,
    viewport: Viewport,
    images: HashMap<Arc<str>, ImageSource>,
    stylesheet: Rc<StyleSheet>,
    time: u64,
    font_families: Option<&[String]>,
  ) -> Self {
    let mut font_cx = fonts.query_context();
    let fallback_families = fonts.resolve_fallbacks(&mut font_cx, font_families).into();
    Self {
      fonts,
      font_cx: Rc::new(RefCell::new(font_cx)),
      fallback_families,
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
      images,
      stylesheet,
    }
  }

  /// Internal, only used in tests.
  pub fn new_test(fonts: &'g Fonts, viewport: Viewport) -> Self {
    Self::new(
      fonts,
      viewport,
      Default::default(),
      Default::default(),
      0,
      None,
    )
  }

  pub fn from_parent(
    parent: &Self,
    style: ComputedStyle,
    sizing: SizingContext,
    current_color: Color,
  ) -> Self {
    Self {
      fonts: parent.fonts,
      font_cx: parent.font_cx.clone(),
      fallback_families: parent.fallback_families.clone(),
      transform: parent.transform,
      style: Box::new(style),
      current_color,
      time: parent.time,
      draw_debug_border: parent.draw_debug_border,
      images: parent.images.clone(),
      sizing,
      stylesheet: parent.stylesheet.clone(),
    }
  }
}
