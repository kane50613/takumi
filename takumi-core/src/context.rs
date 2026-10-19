use std::{
  cell::{Cell, RefCell},
  collections::HashMap,
  rc::Rc,
  sync::Arc,
};

use rustc_hash::FxHashMap;
use typed_builder::TypedBuilder;

use crate::{
  font_style::ExpandedFamilies,
  fonts::{font::Font, font_description::FontDescription, font_fallback_map::FontFallbackMap},
  geometry::{ComputedLayout, Point},
  layout::inline::InlineLayoutCache,
  resources::{
    font::{FontsSnapshot, PrimaryFontMetrics},
    image::{ImageSource, ResourceCache},
  },
  style::{
    Affine, AppliedTextDecorations, Color, ComputedStyle, SizingContext, StyleSheet, TwCache,
  },
};

/// What every context of one render shares.
struct RenderShared {
  fonts: FontsSnapshot,
  images: Rc<HashMap<Arc<str>, ImageSource>>,
  resource_cache: Option<ResourceCache>,
  stylesheet: Arc<StyleSheet>,
  font_families: ExpandedFamilies,
  font_fallback_map: FontFallbackMap,
  inline_cache: InlineLayoutCache,
  tw_cache: TwCache,
  primary_font_metrics: RefCell<FxHashMap<u64, Option<PrimaryFontMetrics>>>,
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
  #[builder(default, setter(into))]
  style: Rc<ComputedStyle>,
  #[builder(default = 0)]
  time_ms: u64,
  #[builder(default = false)]
  draw_debug_border: bool,
  #[builder(default = false)]
  dither_gradients: bool,
  #[builder(default = false)]
  collapsed_borders: bool,
  #[builder(default)]
  images: Rc<HashMap<Arc<str>, ImageSource>>,
  /// The cache `data:` URI images decode through.
  #[builder(default)]
  resource_cache: Option<ResourceCache>,
  #[builder(default)]
  stylesheet: Arc<StyleSheet>,
}

impl From<RenderContextInit> for RenderContext {
  fn from(init: RenderContextInit) -> Self {
    Self {
      shared: Rc::new(RenderShared {
        fonts: init.fonts,
        images: init.images,
        resource_cache: init.resource_cache,
        stylesheet: init.stylesheet,
        font_families: ExpandedFamilies::default(),
        font_fallback_map: FontFallbackMap::default(),
        inline_cache: InlineLayoutCache::default(),
        tw_cache: TwCache::default(),
        primary_font_metrics: RefCell::default(),
        time_ms: init.time_ms,
        draw_debug_border: init.draw_debug_border,
        dither_gradients: init.dither_gradients,
      }),
      sizing: init.sizing,
      transform: init.transform,
      paint_offset: Point::ZERO,
      current_color: init.current_color,
      style: init.style,
      text_decorations: AppliedTextDecorations::default(),
      collapsed_borders: init.collapsed_borders,
      definite_height: Cell::new(None),
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
  /// Blink's paint offset of the space the box's layout location is measured in: where that space
  /// sits in the space Blink snaps paint to pixels in.
  pub paint_offset: Point<f32>,
  /// What the `currentColor` value is resolved to.
  pub current_color: Color,
  /// The style after inheritance.
  pub style: Rc<ComputedStyle>,
  /// The decorations the box's text paints, from it and the boxes around it.
  pub(crate) text_decorations: AppliedTextDecorations,
  /// Whether this box is a cell of a table that collapses its borders.
  pub(crate) collapsed_borders: bool,
  /// The content height the box's last layout fixed before its content, Blink's definite block
  /// size.
  definite_height: Cell<Option<f32>>,
}

/// A [`RenderContextBuilder`] with nothing set yet.
type UnsetRenderContextBuilder =
  RenderContextBuilder<((), (), (), (), (), (), (), (), (), (), (), ())>;

impl RenderContext {
  /// Starts a root context; `fonts` and `sizing` are required.
  pub fn builder() -> UnsetRenderContextBuilder {
    RenderContextInit::builder()
  }

  /// Blink's paint offset of the border box at `layout`, the box this context styles.
  pub fn box_paint_offset(&self, layout: ComputedLayout) -> Point<f32> {
    self.paint_offset + layout.location
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

  /// The cache `data:` URI images decode through, when the render has one.
  pub(crate) fn resource_cache(&self) -> Option<&ResourceCache> {
    self.shared.resource_cache.as_ref()
  }

  /// The stylesheets to apply before layout/rendering.
  pub(crate) fn stylesheet(&self) -> &Arc<StyleSheet> {
    &self.shared.stylesheet
  }

  pub(crate) fn inline_cache(&self) -> &InlineLayoutCache {
    &self.shared.inline_cache
  }

  pub(crate) fn font_families(&self) -> &ExpandedFamilies {
    &self.shared.font_families
  }

  /// Per-render cache of expanded Tailwind class lists.
  pub(crate) fn tw_cache(&self) -> &TwCache {
    &self.shared.tw_cache
  }

  /// The font `description` names, its fallback list shared across the render.
  pub(crate) fn font(&self, description: FontDescription) -> Font {
    Font {
      fallback_list: self
        .shared
        .font_fallback_map
        .get(&self.shared.fonts, &description),
      description,
    }
  }

  /// The primary font metrics for `key`, resolved once per render.
  pub(crate) fn cached_primary_font_metrics(
    &self,
    key: u64,
    resolve: impl FnOnce() -> Option<PrimaryFontMetrics>,
  ) -> Option<PrimaryFontMetrics> {
    if let Some(metrics) = self.shared.primary_font_metrics.borrow().get(&key) {
      return *metrics;
    }
    let metrics = resolve();
    self
      .shared
      .primary_font_metrics
      .borrow_mut()
      .insert(key, metrics);
    metrics
  }

  /// Blink's `CreateAnonymousStyleWithDisplay`, keeping the parent's applied text decorations.
  /// https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/css/resolver/style_resolver.cc
  pub(crate) fn for_anonymous(parent: &Self) -> Self {
    Self {
      shared: parent.shared.clone(),
      sizing: parent.sizing.clone(),
      transform: parent.transform,
      paint_offset: parent.paint_offset,
      current_color: parent.current_color,
      style: Rc::new(ComputedStyle::for_anonymous(&parent.style, &parent.sizing)),
      text_decorations: parent.text_decorations.clone(),
      collapsed_borders: parent.collapsed_borders,
      definite_height: Cell::new(None),
    }
  }

  pub(crate) fn from_parent(
    parent: &Self,
    style: ComputedStyle,
    sizing: SizingContext,
    current_color: Color,
  ) -> Self {
    Self {
      shared: parent.shared.clone(),
      text_decorations: parent.text_decorations.for_child(
        &parent.style,
        &style,
        &sizing,
        current_color,
      ),
      sizing,
      transform: parent.transform,
      paint_offset: parent.paint_offset,
      current_color,
      style: Rc::new(style),
      collapsed_borders: false,
      definite_height: Cell::new(None),
    }
  }

  /// The content height the box's last layout fixed before its content.
  pub(crate) fn definite_height(&self) -> Option<f32> {
    self.definite_height.get()
  }

  pub(crate) fn set_definite_height(&self, height: Option<f32>) {
    self.definite_height.set(height);
  }

  /// The style to change in place, copied first when another box shares it.
  pub(crate) fn style_mut(&mut self) -> &mut ComputedStyle {
    Rc::make_mut(&mut self.style)
  }

  /// Blockifies `display`, leaving a shared style shared when it is already block-level.
  pub(crate) fn blockify(&mut self) {
    let display = self.style.base_data.display.as_blockified();

    if display != self.style.base_data.display {
      self.style_mut().base_data_mut().display = display;
    }
  }
}
