use std::{cell::Cell, collections::HashMap, rc::Rc, sync::Arc};

use typed_builder::TypedBuilder;

use crate::{
  font_style::ExpandedFamilies,
  fonts::{
    font::Font,
    font_description::{FontDescription, FontFallbackKey},
    font_fallback_list::FontFallbackList,
    font_fallback_map::FontFallbackMap,
  },
  geometry::{ComputedLayout, Line, Point},
  layout::inline::InlineLayoutCache,
  resources::{
    font::FontsSnapshot,
    image::{ImageSource, ResourceCache},
  },
  style::{
    Affine, AppliedTextDecorations, Color, ComputedStyle, MatchedStyleCache, SizingContext,
    StyleSheet,
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
  matched_styles: MatchedStyleCache,
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
        matched_styles: MatchedStyleCache::default(),
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
      margins_adjoin: Cell::new(Line::FALSE),
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
  /// Whether its children's margins adjoin its block-start and block-end edges in its last layout,
  /// so they collapse through it.
  margins_adjoin: Cell<Line<bool>>,
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

  /// Per-render cache of the styles boxes resolved from the declarations they matched.
  pub(crate) fn matched_styles(&self) -> &MatchedStyleCache {
    &self.shared.matched_styles
  }

  /// The font `description` names, its fallback list shared across the render.
  pub(crate) fn font(&self, description: FontDescription) -> Font {
    Font {
      fallback_list: self.font_fallback_list(&description.fallback),
      description,
    }
  }

  /// Blink's `FontFallbackMap::Get`: the fallback list `key` names, shared across the render.
  pub(crate) fn font_fallback_list(&self, key: &FontFallbackKey) -> Rc<FontFallbackList> {
    self.shared.font_fallback_map.get(&self.shared.fonts, key)
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
      margins_adjoin: Cell::new(Line::FALSE),
    }
  }

  pub(crate) fn from_parent(
    parent: &Self,
    style: impl Into<Rc<ComputedStyle>>,
    sizing: SizingContext,
    current_color: Color,
  ) -> Self {
    let style = style.into();

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
      style,
      collapsed_borders: false,
      definite_height: Cell::new(None),
      margins_adjoin: Cell::new(Line::FALSE),
    }
  }

  /// The content height the box's last layout fixed before its content.
  pub(crate) fn definite_height(&self) -> Option<f32> {
    self.definite_height.get()
  }

  pub(crate) fn set_definite_height(&self, height: Option<f32>) {
    self.definite_height.set(height);
  }

  /// Whether its children's margins adjoin its block-start and block-end edges in its last layout.
  pub(crate) fn margins_adjoin(&self) -> Line<bool> {
    self.margins_adjoin.get()
  }

  pub(crate) fn set_margins_adjoin(&self, margins_adjoin: Line<bool>) {
    self.margins_adjoin.set(margins_adjoin);
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
