use std::{collections::HashMap, rc::Rc, sync::Arc};

use typed_builder::TypedBuilder;

use crate::{
  layout::inline::{MeasureCache, ShapeCache},
  resources::{font::FontsSnapshot, image::ImageSource},
  style::{Affine, Color, ComputedStyle, SizingContext, StyleSheet},
};

/// The context for the internal rendering. You should not construct this directly.
#[derive(Clone, TypedBuilder)]
#[non_exhaustive]
pub struct RenderContext {
  pub(crate) fonts: FontsSnapshot,
  /// The sizing context.
  pub sizing: SizingContext,
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
  /// Whether this box is a cell of a table that collapses its borders. Set
  /// during lowering, which is the last place a table cell is still one.
  #[builder(default = false)]
  pub collapsed_borders: bool,
  /// The resources fetched externally.
  #[builder(default)]
  pub(crate) images: Rc<HashMap<Arc<str>, ImageSource>>,
  /// The stylesheets to apply before layout/rendering.
  #[builder(default)]
  pub(crate) stylesheet: Arc<StyleSheet>,
  /// Per-render cache of shaped text-only inline layouts, shared by every
  /// context derived from the same root.
  #[builder(default)]
  pub(crate) shape_cache: ShapeCache,
  /// Per-render cache of measured text-node sizes, shared like `shape_cache`.
  #[builder(default)]
  pub(crate) measure_cache: MeasureCache,
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
      transform: parent.transform,
      style: Box::new(style),
      current_color,
      time_ms: parent.time_ms,
      draw_debug_border: parent.draw_debug_border,
      collapsed_borders: false,
      images: parent.images.clone(),
      sizing,
      stylesheet: parent.stylesheet.clone(),
      shape_cache: parent.shape_cache.clone(),
      measure_cache: parent.measure_cache.clone(),
    }
  }
}
