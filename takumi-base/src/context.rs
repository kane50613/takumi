use std::rc::Rc;
use std::sync::Arc;
use std::{cell::RefCell, collections::HashMap};

use typed_builder::TypedBuilder;

use crate::Fonts;
use crate::{
  layout::style::{Affine, Color, ComputedStyle, SizingContext, StyleSheet},
  resources::image::ImageSource,
};

/// The context for the internal rendering. You should not construct this directly.
#[derive(Clone, TypedBuilder)]
#[non_exhaustive]
pub struct RenderContext {
  /// The font registry shared across renders.
  pub(crate) fonts: Rc<RefCell<Fonts>>,
  /// The sizing context.
  pub sizing: SizingContext,
  /// The fallback family chain for this render, appended to every run's font stack.
  #[builder(default)]
  pub(crate) fallback_families: Option<Rc<[String]>>,
  /// The scale factor for the image renderer.
  #[builder(default = Affine::IDENTITY)]
  pub transform: Affine,
  /// What the `currentColor` value is resolved to.
  #[builder(default = Color::black())]
  pub current_color: Color,
  /// The style after inheritance.
  #[builder(default)]
  pub style: Box<ComputedStyle>,
  /// The active time for animation sampling.
  #[builder(default = 0)]
  pub time_ms: u64,
  /// Whether to draw debug borders.
  #[builder(default = false)]
  pub draw_debug_border: bool,
  /// The resources fetched externally.
  #[builder(default)]
  pub(crate) images: Rc<HashMap<Arc<str>, ImageSource>>,
  /// The stylesheets to apply before layout/rendering.
  #[builder(default)]
  pub(crate) stylesheet: Rc<StyleSheet>,
}

impl RenderContext {
  pub(crate) fn from_parent(
    parent: &Self,
    style: ComputedStyle,
    sizing: SizingContext,
    current_color: Color,
  ) -> Self {
    Self {
      fonts: parent.fonts.clone(),
      fallback_families: parent.fallback_families.clone(),
      transform: parent.transform,
      style: Box::new(style),
      current_color,
      time_ms: parent.time_ms,
      draw_debug_border: parent.draw_debug_border,
      images: parent.images.clone(),
      sizing,
      stylesheet: parent.stylesheet.clone(),
    }
  }
}
