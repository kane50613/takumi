use std::{
  borrow::Cow,
  cell::RefCell,
  collections::{HashMap, HashSet},
  iter::once,
  ops::{Deref, DerefMut},
  sync::Arc,
};

use image::{Rgba, RgbaImage};
use parley::{
  GenericFamily, GlyphRun, LayoutContext, TextStyle, TreeBuilder,
  fontique::{
    Blob, Collection, CollectionOptions, FallbackKey, FontInfoOverride, Script, ScriptExt,
  },
};
use skrifa::{
  FontRef, GlyphId, MetadataProvider,
  bitmap::{BitmapData, BitmapGlyph, Origin},
  color::{Brush, ColorGlyphFormat, ColorPainter, CompositeMode, PaintCachedColorGlyph, Transform},
  instance::{LocationRef, Size},
  outline::{DrawSettings, OutlinePen},
  raw::types::{BoundingBox, F2Dot14},
};
use thiserror::Error;
use tiny_skia::Pixmap;

use crate::{
  layout::inline::{InlineBrush, InlineLayout},
  rendering::{Command, premultiplied_pixmap_from_rgba},
  resources::image_decoder::decode_png,
};

#[derive(Clone)]
pub(crate) enum ResolvedGlyph {
  Bitmap(ResolvedBitmapGlyph),
  Outline(ResolvedOutlineGlyph),
}

#[derive(Clone)]
pub(crate) struct ResolvedBitmapGlyph {
  pub(crate) pixmap: Pixmap,
  pub(crate) scale_x: f32,
  pub(crate) scale_y: f32,
  pub(crate) placement: ResolvedGlyphPlacement,
}

impl ResolvedBitmapGlyph {
  pub(crate) fn write_alpha_mask(&self, mask: &mut [u8]) {
    let width = self.placement.width as usize;
    let height = self.placement.height as usize;
    if width == 0 || height == 0 {
      return;
    }

    let alpha_len = width.saturating_mul(height);
    let mask_len = mask.len();
    let write_len = alpha_len.min(mask_len);
    let mask = &mut mask[..write_len];
    let source_width = self.pixmap.width() as usize;
    let source_height = self.pixmap.height() as usize;
    let source_raw = self.pixmap.data();

    if source_width == width && source_height == height {
      for (i, alpha) in source_raw.iter().skip(3).step_by(4).copied().enumerate() {
        if i >= mask.len() {
          break;
        }
        mask[i] = alpha;
      }
      return;
    }

    if source_width == 0 || source_height == 0 {
      return;
    }

    for y in 0..height {
      let mapped_y = ((y as f32 + 0.5) / self.scale_y - 0.5).round();
      let source_y = mapped_y.clamp(0.0, (source_height.saturating_sub(1)) as f32) as usize;

      for x in 0..width {
        let mapped_x = ((x as f32 + 0.5) / self.scale_x - 0.5).round();
        let source_x = mapped_x.clamp(0.0, (source_width.saturating_sub(1)) as f32) as usize;
        let source_index = (source_y * source_width + source_x) * 4 + 3;
        let mask_index = y * width + x;
        if mask_index >= mask.len() || source_index >= source_raw.len() {
          continue;
        }
        mask[mask_index] = source_raw[source_index];
      }
    }
  }
}

#[derive(Clone)]
pub(crate) enum ResolvedOutlineGlyph {
  Plain {
    paths: Vec<Command>,
    embolden: Option<f32>,
  },
  Color {
    paths: Vec<Command>,
    layers: Vec<ResolvedColorLayer>,
  },
}

#[derive(Clone)]
pub(crate) struct ResolvedColorLayer {
  pub(crate) paths: Vec<Command>,
  pub(crate) palette_index: u16,
  pub(crate) alpha: f32,
}

#[derive(Clone, Copy)]
pub(crate) struct ResolvedGlyphPlacement {
  pub(crate) left: i32,
  pub(crate) top: i32,
  pub(crate) width: u32,
  pub(crate) height: u32,
}

impl ResolvedOutlineGlyph {
  pub(crate) fn paths(&self) -> &[Command] {
    match self {
      Self::Plain { paths, .. } | Self::Color { paths, .. } => paths,
    }
  }

  pub(crate) fn embolden(&self) -> Option<f32> {
    match self {
      Self::Plain { embolden, .. } => *embolden,
      Self::Color { .. } => None,
    }
  }

  pub(crate) fn color_layers(&self) -> Option<&[ResolvedColorLayer]> {
    match self {
      Self::Plain { .. } => None,
      Self::Color { layers, .. } => Some(layers),
    }
  }
}

/// Matches the typical faux-bold expansion used by text rasterizers.
const SYNTHESIS_EMBOLDEN_FACTOR: f32 = 1.0 / 24.0;

pub(crate) fn synthesis_embolden_strength(font_size: f32) -> f32 {
  font_size * SYNTHESIS_EMBOLDEN_FACTOR
}

#[derive(Default)]
struct GlyphOutlinePen {
  paths: Vec<Command>,
}

impl GlyphOutlinePen {
  fn finish(self) -> Vec<Command> {
    self.paths
  }
}

impl OutlinePen for GlyphOutlinePen {
  fn move_to(&mut self, x: f32, y: f32) {
    self
      .paths
      .push(Command::MoveTo(tiny_skia::Point::from_xy(x, -y)));
  }

  fn line_to(&mut self, x: f32, y: f32) {
    self
      .paths
      .push(Command::LineTo(tiny_skia::Point::from_xy(x, -y)));
  }

  fn quad_to(&mut self, cx0: f32, cy0: f32, x: f32, y: f32) {
    self.paths.push(Command::QuadTo(
      tiny_skia::Point::from_xy(cx0, -cy0),
      tiny_skia::Point::from_xy(x, -y),
    ));
  }

  fn curve_to(&mut self, cx0: f32, cy0: f32, cx1: f32, cy1: f32, x: f32, y: f32) {
    self.paths.push(Command::CubicTo(
      tiny_skia::Point::from_xy(cx0, -cy0),
      tiny_skia::Point::from_xy(cx1, -cy1),
      tiny_skia::Point::from_xy(x, -y),
    ));
  }

  fn close(&mut self) {
    self.paths.push(Command::Close);
  }
}

struct ColorLayerCollector<'a, 'font> {
  font_ref: &'font FontRef<'a>,
  size: Size,
  location: LocationRef<'a>,
  layers: Vec<ResolvedColorLayer>,
}

impl<'a, 'font> ColorLayerCollector<'a, 'font> {
  fn new(font_ref: &'font FontRef<'a>, size: Size, location: LocationRef<'a>) -> Self {
    Self {
      font_ref,
      size,
      location,
      layers: Vec::new(),
    }
  }

  fn into_layers(self) -> Vec<ResolvedColorLayer> {
    self.layers
  }
}

struct GlyphResolveContext<'a, 'font> {
  font_ref: &'font FontRef<'a>,
  font_size: f32,
  size: Size,
  location: LocationRef<'a>,
  skew: Option<f32>,
  embolden: Option<f32>,
}

impl<'a, 'font> GlyphResolveContext<'a, 'font> {
  fn resolve_glyph(&self, glyph_id: u32) -> Option<ResolvedGlyph> {
    let glyph_id = GlyphId::new(glyph_id);

    self
      .resolve_bitmap_glyph(glyph_id)
      .map(ResolvedGlyph::Bitmap)
      .or_else(|| {
        // Color outline glyphs intentionally bypass skew synthesis so COLR
        // layer metrics stay aligned and stacked color layers are not deformed.
        self
          .resolve_color_outline_glyph(glyph_id)
          .map(ResolvedGlyph::Outline)
      })
      .or_else(|| {
        self
          .resolve_plain_outline_glyph(glyph_id)
          .map(ResolvedGlyph::Outline)
      })
  }

  fn resolve_bitmap_glyph(&self, glyph_id: GlyphId) -> Option<ResolvedBitmapGlyph> {
    let bitmap = self
      .font_ref
      .bitmap_strikes()
      .glyph_for_size(self.size, glyph_id)?;
    scale_bitmap_glyph(bitmap, self.font_size)
  }

  fn resolve_color_outline_glyph(&self, glyph_id: GlyphId) -> Option<ResolvedOutlineGlyph> {
    resolve_color_outline_glyph(self.font_ref, glyph_id, self.size, self.location)
  }

  fn resolve_plain_outline_glyph(&self, glyph_id: GlyphId) -> Option<ResolvedOutlineGlyph> {
    let mut paths = resolve_outline_commands(self.font_ref, glyph_id, self.size, self.location)?;
    if let Some(skew_degrees) = self.skew {
      transform_commands(&mut paths, skew_degrees);
    }

    Some(ResolvedOutlineGlyph::Plain {
      paths,
      embolden: self.embolden,
    })
  }
}

/// `ColorPainter` for `ColorLayerCollector` that only records COLR v0 layer
/// stacking. `push_transform`, `pop_transform`, `push_clip_glyph`,
/// `push_clip_box`, `pop_clip`, and `push_layer` are intentional no-ops, and
/// `fill_glyph` only records `Brush::Solid` layers, so gradients and other
/// non-solid brushes are silently skipped.
impl ColorPainter for ColorLayerCollector<'_, '_> {
  fn push_transform(&mut self, _transform: Transform) {}

  fn pop_transform(&mut self) {}

  fn push_clip_glyph(&mut self, _glyph_id: GlyphId) {}

  fn push_clip_box(&mut self, _clip_box: BoundingBox<f32>) {}

  fn pop_clip(&mut self) {}

  fn fill(&mut self, _brush: Brush<'_>) {}

  fn fill_glyph(
    &mut self,
    glyph_id: GlyphId,
    _brush_transform: Option<Transform>,
    brush: Brush<'_>,
  ) {
    let Brush::Solid {
      palette_index,
      alpha,
    } = brush
    else {
      return;
    };

    let Some(paths) = resolve_outline_commands(self.font_ref, glyph_id, self.size, self.location)
    else {
      return;
    };

    self.layers.push(ResolvedColorLayer {
      paths,
      palette_index,
      alpha,
    });
  }

  fn paint_cached_color_glyph(
    &mut self,
    _glyph: GlyphId,
  ) -> Result<PaintCachedColorGlyph, skrifa::color::PaintError> {
    Ok(PaintCachedColorGlyph::Unimplemented)
  }

  fn push_layer(&mut self, _composite_mode: CompositeMode) {}
}

fn resolve_outline_commands(
  font_ref: &FontRef<'_>,
  glyph_id: GlyphId,
  size: Size,
  location: LocationRef<'_>,
) -> Option<Vec<Command>> {
  let glyph = font_ref.outline_glyphs().get(glyph_id)?;
  let mut pen = GlyphOutlinePen::default();
  glyph
    .draw(DrawSettings::unhinted(size, location), &mut pen)
    .ok()?;
  Some(pen.finish())
}

fn transform_commands(paths: &mut [Command], skew_degrees: f32) {
  let skew_tangent = skew_degrees.to_radians().tan();
  for command in paths {
    match command {
      Command::MoveTo(point) | Command::LineTo(point) => {
        point.x += point.y * skew_tangent;
      }
      Command::QuadTo(control, point) => {
        control.x += control.y * skew_tangent;
        point.x += point.y * skew_tangent;
      }
      Command::CubicTo(control1, control2, point) => {
        control1.x += control1.y * skew_tangent;
        control2.x += control2.y * skew_tangent;
        point.x += point.y * skew_tangent;
      }
      Command::Close => {}
    }
  }
}

fn decode_bitmap_image(bitmap: &BitmapGlyph<'_>) -> Option<(Pixmap, Origin)> {
  let pixmap = match &bitmap.data {
    BitmapData::Png(bytes) => decode_png(bytes).ok()?,
    BitmapData::Bgra(bytes) => {
      let image = RgbaImage::from_fn(bitmap.width, bitmap.height, |x, y| {
        let index = ((y * bitmap.width + x) * 4) as usize;
        Rgba([
          bytes[index + 2],
          bytes[index + 1],
          bytes[index],
          bytes[index + 3],
        ])
      });
      premultiplied_pixmap_from_rgba(Cow::Owned(image))?
    }
    BitmapData::Mask(_) => return None,
  };

  Some((pixmap, bitmap.placement_origin))
}

fn scale_bitmap_glyph(bitmap: BitmapGlyph<'_>, font_size: f32) -> Option<ResolvedBitmapGlyph> {
  let (pixmap, origin) = decode_bitmap_image(&bitmap)?;
  let scale_x = if bitmap.ppem_x > 0.0 {
    font_size / bitmap.ppem_x
  } else {
    1.0
  };
  let scale_y = if bitmap.ppem_y > 0.0 {
    font_size / bitmap.ppem_y
  } else {
    1.0
  };
  let width = ((pixmap.width() as f32) * scale_x).round().max(1.0) as u32;
  let height = ((pixmap.height() as f32) * scale_y).round().max(1.0) as u32;
  let top = match origin {
    Origin::TopLeft => bitmap.inner_bearing_y,
    Origin::BottomLeft => bitmap.inner_bearing_y + bitmap.height as f32,
  };

  Some(ResolvedBitmapGlyph {
    pixmap,
    scale_x,
    scale_y,
    placement: ResolvedGlyphPlacement {
      left: (bitmap.inner_bearing_x * scale_x).round() as i32,
      top: (top * scale_y).round() as i32,
      width,
      height,
    },
  })
}

fn resolve_color_outline_glyph(
  font_ref: &FontRef<'_>,
  glyph_id: GlyphId,
  size: Size,
  location: LocationRef<'_>,
) -> Option<ResolvedOutlineGlyph> {
  let color_glyph = font_ref
    .color_glyphs()
    .get_with_format(glyph_id, ColorGlyphFormat::ColrV0)?;
  let mut collector = ColorLayerCollector::new(font_ref, size, location);
  color_glyph.paint(location, &mut collector).ok()?;
  let color_layers = collector.into_layers();
  if color_layers.is_empty() {
    return None;
  }

  let mut paths = Vec::new();
  for layer in &color_layers {
    paths.extend(layer.paths.iter().copied());
  }

  Some(ResolvedOutlineGlyph::Color {
    paths,
    layers: color_layers,
  })
}

/// Errors that can occur during font loading and conversion.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum FontError {
  /// Error occurred during WOFF conversion
  #[cfg(any(feature = "woff", feature = "woff2"))]
  #[error("Error occurred during WOFF conversion.")]
  Woff(wuff::WuffErr),
  /// Unsupported Font Format
  #[error("Unsupported font format")]
  UnsupportedFormat,
  /// Font index is invalid
  #[error("Font index is invalid")]
  InvalidFontIndex,
}

/// Supported font formats for loading and processing
#[derive(Copy, Clone)]
#[non_exhaustive]
pub enum FontFormat {
  #[cfg(feature = "woff")]
  /// Web Open Font Format (WOFF) - compressed web font format
  Woff,
  #[cfg(feature = "woff2")]
  /// Web Open Font Format 2 (WOFF2) - improved compression web font format
  Woff2,
  /// TrueType Font format - standard desktop font format
  Ttf,
  /// OpenType Font format - extended font format with advanced typography
  Otf,
  /// TrueType Collection - multiple fonts in one file
  Ttc,
}

fn load_font(source: Cow<'_, [u8]>, format_hint: Option<FontFormat>) -> Result<Vec<u8>, FontError> {
  let format = if let Some(format) = format_hint {
    format
  } else {
    guess_font_format(&source)?
  };

  match format {
    FontFormat::Ttf | FontFormat::Otf | FontFormat::Ttc => Ok(source.into_owned()),
    #[cfg(feature = "woff2")]
    FontFormat::Woff2 => {
      let ttf = wuff::decompress_woff2(&source).map_err(FontError::Woff)?;
      Ok(ttf)
    }
    #[cfg(feature = "woff")]
    FontFormat::Woff => {
      let ttf = wuff::decompress_woff1(&source).map_err(FontError::Woff)?;
      Ok(ttf)
    }
  }
}

fn guess_font_format(source: &[u8]) -> Result<FontFormat, FontError> {
  if source.len() < 4 {
    return Err(FontError::UnsupportedFormat);
  }

  match &source[0..4] {
    #[cfg(feature = "woff2")]
    b"wOF2" => Ok(FontFormat::Woff2),
    #[cfg(feature = "woff")]
    b"wOFF" => Ok(FontFormat::Woff),
    [0x00, 0x01, 0x00, 0x00] => Ok(FontFormat::Ttf),
    b"OTTO" => Ok(FontFormat::Otf),
    b"ttcf" => Ok(FontFormat::Ttc),
    _ => Err(FontError::UnsupportedFormat),
  }
}

thread_local! {
  static LAYOUT_CONTEXT: RefCell<LayoutContext<InlineBrush>> = RefCell::new(LayoutContext::new());
}

fn with_layout_context<R>(f: impl FnOnce(&mut LayoutContext<InlineBrush>) -> R) -> R {
  LAYOUT_CONTEXT.with(|cell| match cell.try_borrow_mut() {
    Ok(mut ctx) => f(&mut ctx),
    Err(_) => f(&mut LayoutContext::new()),
  })
}

/// A context for managing fonts in the rendering system.
#[derive(Clone)]
pub struct FontContext {
  inner: parley::FontContext,
}

impl Default for FontContext {
  fn default() -> Self {
    Self {
      inner: parley::FontContext {
        collection: Collection::new(CollectionOptions {
          system_fonts: false,
          shared: false,
        }),
        source_cache: Default::default(),
      },
    }
  }
}

impl Deref for FontContext {
  type Target = parley::FontContext;

  fn deref(&self) -> &Self::Target {
    &self.inner
  }
}

impl DerefMut for FontContext {
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.inner
  }
}

impl FontContext {
  pub(crate) fn resolve_glyphs(
    &self,
    run: &GlyphRun<'_, InlineBrush>,
    font_ref: FontRef,
    glyph_ids: impl Iterator<Item = u32> + Clone,
  ) -> HashMap<u32, ResolvedGlyph> {
    let unique_glyph_ids: HashSet<u32> = glyph_ids.collect();
    if unique_glyph_ids.is_empty() {
      return HashMap::new();
    }

    let has_emoji_cluster = run
      .run()
      .visual_clusters()
      .any(|cluster| cluster.is_emoji());
    let font_size = run.run().font_size();
    let normalized_coords = run
      .run()
      .normalized_coords()
      .iter()
      .copied()
      .map(F2Dot14::from_bits)
      .collect::<Vec<_>>();
    let resolver = GlyphResolveContext {
      font_ref: &font_ref,
      font_size,
      size: Size::new(font_size),
      location: LocationRef::new(&normalized_coords),
      embolden: (!has_emoji_cluster
        && run.run().synthesis().embolden()
        && run.style().brush.font_synthesis.weight.is_allowed())
      .then_some(synthesis_embolden_strength(font_size)),
      skew: run
        .run()
        .synthesis()
        .skew()
        .filter(|_| !has_emoji_cluster)
        .filter(|_| run.style().brush.font_synthesis.style.is_allowed())
        .map(|degrees| -degrees),
    };

    let mut result = HashMap::with_capacity(unique_glyph_ids.len());
    for glyph_id in unique_glyph_ids {
      if let Some(glyph) = resolver.resolve_glyph(glyph_id) {
        result.insert(glyph_id, glyph);
      }
    }

    result
  }

  /// Create an inline layout with the given root style and function
  pub(crate) fn tree_builder(
    &self,
    root_style: TextStyle<'_, '_, InlineBrush>,
    func: impl FnOnce(&mut TreeBuilder<'_, InlineBrush>),
  ) -> (InlineLayout, String) {
    let mut font_context = self.clone();

    with_layout_context(|layout_context| {
      let mut builder = layout_context.tree_builder(&mut font_context, 1.0, true, &root_style);
      func(&mut builder);
      builder.build()
    })
  }

  /// Loads font into internal font db with caching
  pub fn load_and_store(&mut self, font: FontResource) -> Result<(), FontError> {
    let FontResource {
      source,
      info_override,
      generic_family,
    } = font;

    let fonts = self
      .inner
      .collection
      .register_fonts(source.into_blob()?, info_override);

    for (family, _) in fonts {
      if let Some(generic_family) = generic_family {
        self
          .inner
          .collection
          .append_generic_families(generic_family, once(family));
      }

      for (script, _) in Script::all_samples() {
        self
          .inner
          .collection
          .append_fallbacks(FallbackKey::new(*script, None), once(family));
      }
    }

    Ok(())
  }
}

/// Represents a font source buffer.
#[derive(Debug)]
pub enum FontSource<'a> {
  /// Raw font buffer.
  Raw(Cow<'a, [u8]>),
  /// Recognized font blob.
  /// Woff2 and Woff should be decompressed into raw buffer before passing to this.
  Blob(Blob<u8>),
}

impl<'a, T> From<T> for FontSource<'a>
where
  T: Into<Cow<'a, [u8]>>,
{
  fn from(value: T) -> Self {
    Self::Raw(value.into())
  }
}

impl<'a> FontSource<'a> {
  fn into_blob_variant(self) -> Result<Self, FontError> {
    match self {
      Self::Raw(raw) => {
        let font = load_font(raw, None)?;
        Ok(Self::Blob(Blob::new(Arc::new(font))))
      }
      Self::Blob(_) => Ok(self),
    }
  }

  fn into_blob(self) -> Result<Blob<u8>, FontError> {
    match self {
      Self::Raw(raw) => {
        let font = load_font(raw, None)?;
        Ok(Blob::new(Arc::new(font)))
      }
      Self::Blob(blob) => Ok(blob),
    }
  }
}

impl<'a> AsRef<[u8]> for FontSource<'a> {
  fn as_ref(&self) -> &[u8] {
    match self {
      Self::Raw(raw) => raw,
      Self::Blob(blob) => blob.as_ref(),
    }
  }
}

#[derive(Debug)]
/// Information of a font resource
pub struct FontResource<'a> {
  /// Font source
  source: FontSource<'a>,
  /// Font information for override
  info_override: Option<FontInfoOverride<'a>>,
  /// Generic font family
  generic_family: Option<GenericFamily>,
}

impl<'a> FontResource<'a> {
  /// Create a new font to load
  pub fn new(source: impl Into<FontSource<'a>>) -> Self {
    Self {
      source: source.into(),
      info_override: None,
      generic_family: None,
    }
  }

  /// Set font information for override
  pub fn override_info(self, info_override: FontInfoOverride<'a>) -> Self {
    Self {
      info_override: Some(info_override),
      ..self
    }
  }

  /// Set generic family for the font
  pub fn generic_family(self, generic_family: GenericFamily) -> Self {
    Self {
      generic_family: Some(generic_family),
      ..self
    }
  }

  /// Convert to resolved font resource
  /// Woff2 and Woff should be decompressed into raw buffer.
  pub fn into_resolved(self) -> Result<Self, FontError> {
    let source = self.source.into_blob_variant()?;
    Ok(Self {
      source,
      info_override: self.info_override,
      generic_family: self.generic_family,
    })
  }
}
