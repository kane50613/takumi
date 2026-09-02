use std::{collections::HashMap, rc::Rc, sync::Arc};

use typed_builder::TypedBuilder;

use crate::{
  layout::inline::{MeasureCache, ShapeCache},
  resources::{font::FontsSnapshot, image::ImageSource},
  style::{Affine, Color, ComputedStyle, SizingContext, StyleSheet},
};

/// What every context of one render shares.
struct RenderShared {
  fonts: FontsSnapshot,
  images: Rc<HashMap<Arc<str>, ImageSource>>,
  stylesheet: Arc<StyleSheet>,
  shape_cache: ShapeCache,
  measure_cache: MeasureCache,
  time_ms: u64,
  draw_debug_border: bool,
  dither_gradients: bool,
}

/// The values a render starts from, collected by [`RenderContext::builder`].
#[derive(TypedBuilder)]
#[builder(
  builder_type(name = RenderContextBuilder),
  build_method(into = RenderContext)
)]
pub struct RenderContextInit {
  fonts: FontsSnapshot,
  sizing: SizingContext,
  #[builder(default = Affine::IDENTITY)]
  transform: Affine,
  #[builder(default = Color::black())]
  current_color: Color,
  #[builder(default)]
  style: Box<ComputedStyle>,
  #[builder(default = 0)]
  time_ms: u64,
  #[builder(default = false)]
  draw_debug_border: bool,
  #[builder(default = false)]
  dither_gradients: bool,
  #[builder(default = false)]
  collapsed_borders: bool,
  #[builder(default = false)]
  intrinsic_min_content: bool,
  #[builder(default)]
  images: Rc<HashMap<Arc<str>, ImageSource>>,
  #[builder(default)]
  stylesheet: Arc<StyleSheet>,
  #[builder(default)]
  shape_cache: ShapeCache,
  #[builder(default)]
  measure_cache: MeasureCache,
}

impl From<RenderContextInit> for RenderContext {
  fn from(init: RenderContextInit) -> Self {
    Self {
      shared: Rc::new(RenderShared {
        fonts: init.fonts,
        images: init.images,
        stylesheet: init.stylesheet,
        shape_cache: init.shape_cache,
        measure_cache: init.measure_cache,
        time_ms: init.time_ms,
        draw_debug_border: init.draw_debug_border,
        dither_gradients: init.dither_gradients,
      }),
      sizing: init.sizing,
      transform: init.transform,
      current_color: init.current_color,
      style: init.style,
      collapsed_borders: init.collapsed_borders,
      intrinsic_min_content: init.intrinsic_min_content,
    }
  }
}

/// The context for the internal rendering.
#[derive(Clone)]
#[non_exhaustive]
pub struct RenderContext {
  shared: Rc<RenderShared>,
  /// The sizing context.
  pub sizing: SizingContext,
  /// The scale factor for the image renderer.
  pub transform: Affine,
  /// What the `currentColor` value is resolved to.
  pub current_color: Color,
  /// The style after inheritance.
  pub style: Box<ComputedStyle>,
  /// Whether this box is a cell of a table that collapses its borders.
  pub(crate) collapsed_borders: bool,
  /// Whether a min-content measurement reports the widest run it could not break instead of the
  /// zero width it wrapped against.
  pub(crate) intrinsic_min_content: bool,
}

/// A [`RenderContextBuilder`] with nothing set yet.
type UnsetRenderContextBuilder =
  RenderContextBuilder<((), (), (), (), (), (), (), (), (), (), (), (), (), ())>;

impl RenderContext {
  /// Starts a root context; `fonts` and `sizing` are required.
  pub fn builder() -> UnsetRenderContextBuilder {
    RenderContextInit::builder()
  }

  /// The font snapshot this render draws with.
  pub fn fonts(&self) -> &FontsSnapshot {
    &self.shared.fonts
  }

  /// The active time for animation sampling.
  pub fn time_ms(&self) -> u64 {
    self.shared.time_ms
  }

  /// Whether to draw debug borders.
  pub fn draw_debug_border(&self) -> bool {
    self.shared.draw_debug_border
  }

  /// Whether gradient fills dither before quantizing, set from the render's `dithering` option.
  pub fn dither_gradients(&self) -> bool {
    self.shared.dither_gradients
  }

  /// The resources fetched externally.
  pub(crate) fn images(&self) -> &HashMap<Arc<str>, ImageSource> {
    &self.shared.images
  }

  /// The stylesheets to apply before layout/rendering.
  pub(crate) fn stylesheet(&self) -> &Arc<StyleSheet> {
    &self.shared.stylesheet
  }

  /// Per-render cache of shaped text-only inline layouts.
  pub(crate) fn shape_cache(&self) -> &ShapeCache {
    &self.shared.shape_cache
  }

  /// Per-render cache of measured text-node sizes.
  pub(crate) fn measure_cache(&self) -> &MeasureCache {
    &self.shared.measure_cache
  }

  pub(crate) fn from_parent(
    parent: &Self,
    style: ComputedStyle,
    sizing: SizingContext,
    current_color: Color,
  ) -> Self {
    Self {
      shared: parent.shared.clone(),
      sizing,
      transform: parent.transform,
      current_color,
      style: Box::new(style),
      collapsed_borders: false,
      intrinsic_min_content: parent.intrinsic_min_content,
    }
  }
}
