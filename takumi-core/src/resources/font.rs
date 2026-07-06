use std::{
  borrow::Cow,
  cell::RefCell,
  collections::{BTreeSet, HashMap, hash_map::Entry},
  iter::once,
  rc::Rc,
  sync::Arc,
};

use image::{Rgba, RgbaImage};
use parley::{
  GenericFamily as ParleyGenericFamily, GlyphRun, LayoutContext, TextStyle, TreeBuilder,
  fontique::{
    Attributes, Blob, Collection, CollectionOptions, FallbackKey, FontInfoOverride, FontStyle,
    FontWeight, FontWidth, QueryFamily, QueryStatus, Script, ScriptExt,
  },
};
use skrifa::{
  FontRef, GlyphId, MetadataProvider,
  bitmap::{BitmapData, BitmapGlyph, BitmapStrikes, Origin},
  color::{
    Brush, ColorGlyphCollection, ColorGlyphFormat, ColorPainter, CompositeMode,
    PaintCachedColorGlyph, Transform,
  },
  instance::{LocationRef, Size},
  outline::{DrawSettings, OutlineGlyphCollection, OutlinePen},
  raw::types::{BoundingBox, F2Dot14, Tag},
};
use thiserror::Error;

use crate::{
  context::RenderContext,
  geometry::{PathCommand as Command, Point},
  layout::inline::{InlineBrush, InlineLayout},
  resources::{image_buffer::ImageBuffer, image_decoder::decode_png},
  style::FontStyle as CssFontStyle,
};

/// A resolved glyph, either an embedded bitmap or a vector outline.
#[derive(Clone)]
pub enum ResolvedGlyph {
  /// Embedded bitmap glyph.
  Bitmap(ResolvedBitmapGlyph),
  /// Vector outline glyph.
  Outline(ResolvedOutlineGlyph),
}

/// A glyph backed by an embedded bitmap.
#[derive(Clone)]
pub struct ResolvedBitmapGlyph {
  /// Source bitmap.
  pub image: ImageBuffer,
  /// Horizontal scale from source to placement.
  pub scale_x: f32,
  /// Vertical scale from source to placement.
  pub scale_y: f32,
  /// Pixel placement of the glyph.
  pub placement: ResolvedGlyphPlacement,
}

impl ResolvedBitmapGlyph {
  /// Write the glyph's alpha channel into `mask`, scaling to the placement size.
  pub fn write_alpha_mask(&self, mask: &mut [u8]) {
    let width = self.placement.width as usize;
    let height = self.placement.height as usize;
    if width == 0 || height == 0 {
      return;
    }

    let alpha_len = width.saturating_mul(height);
    let mask_len = mask.len();
    let write_len = alpha_len.min(mask_len);
    let mask = &mut mask[..write_len];
    let source_width = self.image.width() as usize;
    let source_height = self.image.height() as usize;
    let source_raw = self.image.data();

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

/// An outline glyph, either single-color or multi-layer color.
#[derive(Clone)]
pub enum ResolvedOutlineGlyph {
  /// Single-color outline.
  Plain {
    /// Outline path commands.
    paths: Vec<Command>,
    /// Synthetic bold amount, if any.
    embolden: Option<f32>,
    /// Hash identifying this outline for caching.
    cache_signature: u64,
  },
  /// Multi-layer color outline (COLR).
  Color {
    /// Combined outline path commands.
    paths: Vec<Command>,
    /// Per-layer colored outlines.
    layers: Vec<ResolvedColorLayer>,
    /// Hash identifying this outline for caching.
    cache_signature: u64,
  },
}

/// One palette-colored layer of a color glyph.
#[derive(Clone)]
pub struct ResolvedColorLayer {
  /// Outline path commands for this layer.
  pub paths: Vec<Command>,
  /// Index into the font's color palette.
  pub palette_index: u16,
  /// Layer opacity, 0..=1.
  pub alpha: f32,
}

/// Pixel placement of a rendered glyph.
#[derive(Clone, Copy)]
pub struct ResolvedGlyphPlacement {
  /// Left offset in pixels.
  pub left: i32,
  /// Top offset in pixels.
  pub top: i32,
  /// Width in pixels.
  pub width: u32,
  /// Height in pixels.
  pub height: u32,
}

impl ResolvedOutlineGlyph {
  /// Outline path commands for the glyph.
  pub fn paths(&self) -> &[Command] {
    match self {
      Self::Plain { paths, .. } | Self::Color { paths, .. } => paths,
    }
  }

  /// Hash identifying this resolved outline for caching.
  pub fn cache_signature(&self) -> u64 {
    match self {
      Self::Plain {
        cache_signature, ..
      }
      | Self::Color {
        cache_signature, ..
      } => *cache_signature,
    }
  }

  /// Synthetic bold amount, if any.
  pub fn embolden(&self) -> Option<f32> {
    match self {
      Self::Plain { embolden, .. } => *embolden,
      Self::Color { .. } => None,
    }
  }

  /// Color layers for a color glyph, else `None`.
  pub fn color_layers(&self) -> Option<&[ResolvedColorLayer]> {
    match self {
      Self::Plain { .. } => None,
      Self::Color { layers, .. } => Some(layers),
    }
  }
}

/// CSS `kBoldThreshold` — weights at or above this synthesize bold when no bolder face
/// exists; lighter weights do not. https://drafts.csswg.org/css-fonts-4/#font-weight-prop
const BOLD_THRESHOLD: f32 = 600.0;

/// Skia's fake-bold stroke width as a fraction of text size: `1/24` at 9px and below,
/// easing to `1/32` at 36px and above, linearly interpolated in between. A constant factor
/// over-emboldens large text. See Skia's `SkTextFormatParams.h`.
fn skia_fake_bold_factor(font_size: f32) -> f32 {
  const SMALL_SIZE: f32 = 9.0;
  const LARGE_SIZE: f32 = 36.0;
  const SMALL_FACTOR: f32 = 1.0 / 24.0;
  const LARGE_FACTOR: f32 = 1.0 / 32.0;

  let t = ((font_size - SMALL_SIZE) / (LARGE_SIZE - SMALL_SIZE)).clamp(0.0, 1.0);
  SMALL_FACTOR + t * (LARGE_FACTOR - SMALL_FACTOR)
}

/// Stroke width for synthesized (faux) bold — the emboldened glyph is the filled outline
/// plus a centered stroke of this width, matching Skia's fake bold.
pub(crate) fn synthesis_embolden_strength(font_size: f32) -> f32 {
  font_size * skia_fake_bold_factor(font_size)
}

fn hash_path_commands(paths: &[Command]) -> u64 {
  use xxhash_rust::xxh3::Xxh3;
  let mut h = Xxh3::new();
  for cmd in paths {
    match cmd {
      Command::MoveTo(p) => {
        h.update(&[0u8]);
        h.update(&p.x.to_le_bytes());
        h.update(&p.y.to_le_bytes());
      }
      Command::LineTo(p) => {
        h.update(&[1u8]);
        h.update(&p.x.to_le_bytes());
        h.update(&p.y.to_le_bytes());
      }
      Command::QuadTo(p1, p2) => {
        h.update(&[2u8]);
        h.update(&p1.x.to_le_bytes());
        h.update(&p1.y.to_le_bytes());
        h.update(&p2.x.to_le_bytes());
        h.update(&p2.y.to_le_bytes());
      }
      Command::CubicTo(p1, p2, p3) => {
        h.update(&[3u8]);
        h.update(&p1.x.to_le_bytes());
        h.update(&p1.y.to_le_bytes());
        h.update(&p2.x.to_le_bytes());
        h.update(&p2.y.to_le_bytes());
        h.update(&p3.x.to_le_bytes());
        h.update(&p3.y.to_le_bytes());
      }
      Command::Close => {
        h.update(&[4u8]);
      }
    }
  }
  h.digest()
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
    self.paths.push(Command::MoveTo(Point::new(x, -y)));
  }

  fn line_to(&mut self, x: f32, y: f32) {
    self.paths.push(Command::LineTo(Point::new(x, -y)));
  }

  fn quad_to(&mut self, cx0: f32, cy0: f32, x: f32, y: f32) {
    self
      .paths
      .push(Command::QuadTo(Point::new(cx0, -cy0), Point::new(x, -y)));
  }

  fn curve_to(&mut self, cx0: f32, cy0: f32, cx1: f32, cy1: f32, x: f32, y: f32) {
    self.paths.push(Command::CubicTo(
      Point::new(cx0, -cy0),
      Point::new(cx1, -cy1),
      Point::new(x, -y),
    ));
  }

  fn close(&mut self) {
    self.paths.push(Command::Close);
  }
}

struct ColorLayerCollector<'a, 'g> {
  outline_glyphs: &'g OutlineGlyphCollection<'a>,
  size: Size,
  location: LocationRef<'a>,
  layers: Vec<ResolvedColorLayer>,
}

impl<'a, 'g> ColorLayerCollector<'a, 'g> {
  fn new(
    outline_glyphs: &'g OutlineGlyphCollection<'a>,
    size: Size,
    location: LocationRef<'a>,
  ) -> Self {
    Self {
      outline_glyphs,
      size,
      location,
      layers: Vec::new(),
    }
  }

  fn into_layers(self) -> Vec<ResolvedColorLayer> {
    self.layers
  }
}

struct GlyphResolveContext<'a> {
  outline_glyphs: OutlineGlyphCollection<'a>,
  color_glyphs: ColorGlyphCollection<'a>,
  bitmap_strikes: BitmapStrikes<'a>,
  font_size: f32,
  size: Size,
  location: LocationRef<'a>,
  skew: Option<f32>,
  embolden: Option<f32>,
}

impl<'a> GlyphResolveContext<'a> {
  fn resolve_glyph(&self, glyph_id: u32) -> Option<ResolvedGlyph> {
    let glyph_id = GlyphId::new(glyph_id);

    self
      .resolve_bitmap_glyph(glyph_id)
      .map(ResolvedGlyph::Bitmap)
      .or_else(|| {
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
    let bitmap = self.bitmap_strikes.glyph_for_size(self.size, glyph_id)?;
    scale_bitmap_glyph(bitmap, self.font_size)
  }

  fn resolve_color_outline_glyph(&self, glyph_id: GlyphId) -> Option<ResolvedOutlineGlyph> {
    let color_glyph = self
      .color_glyphs
      .get_with_format(glyph_id, ColorGlyphFormat::ColrV0)?;
    let mut collector = ColorLayerCollector::new(&self.outline_glyphs, self.size, self.location);
    color_glyph.paint(self.location, &mut collector).ok()?;
    let color_layers = collector.into_layers();
    if color_layers.is_empty() {
      return None;
    }

    let mut paths = Vec::new();
    for layer in &color_layers {
      paths.extend(layer.paths.iter().copied());
    }
    let cache_signature = hash_path_commands(&paths);

    Some(ResolvedOutlineGlyph::Color {
      paths,
      layers: color_layers,
      cache_signature,
    })
  }

  fn resolve_plain_outline_glyph(&self, glyph_id: GlyphId) -> Option<ResolvedOutlineGlyph> {
    let mut paths =
      resolve_outline_commands(&self.outline_glyphs, glyph_id, self.size, self.location)?;
    if let Some(skew_degrees) = self.skew {
      transform_commands(&mut paths, skew_degrees);
    }
    let cache_signature = hash_path_commands(&paths);

    Some(ResolvedOutlineGlyph::Plain {
      paths,
      embolden: self.embolden,
      cache_signature,
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

    let Some(paths) =
      resolve_outline_commands(self.outline_glyphs, glyph_id, self.size, self.location)
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
  outline_glyphs: &OutlineGlyphCollection<'_>,
  glyph_id: GlyphId,
  size: Size,
  location: LocationRef<'_>,
) -> Option<Vec<Command>> {
  let glyph = outline_glyphs.get(glyph_id)?;
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

fn decode_bitmap_image(bitmap: &BitmapGlyph<'_>) -> Option<(ImageBuffer, Origin)> {
  let image = match &bitmap.data {
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
      let (width, height) = (image.width(), image.height());
      ImageBuffer::from_rgba_bytes(image.into_raw(), width, height)?
    }
    BitmapData::Mask(_) => return None,
  };

  Some((image, bitmap.placement_origin))
}

fn scale_bitmap_glyph(bitmap: BitmapGlyph<'_>, font_size: f32) -> Option<ResolvedBitmapGlyph> {
  let (image, origin) = decode_bitmap_image(&bitmap)?;
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
  let width = ((image.width() as f32) * scale_x).round().max(1.0) as u32;
  let height = ((image.height() as f32) * scale_y).round().max(1.0) as u32;
  let top = match origin {
    Origin::TopLeft => bitmap.inner_bearing_y,
    Origin::BottomLeft => bitmap.inner_bearing_y + bitmap.height as f32,
  };

  Some(ResolvedBitmapGlyph {
    image,
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

/// Errors that can occur during font loading and conversion.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum FontError {
  /// Error occurred during WOFF conversion
  #[cfg(any(feature = "woff", feature = "woff2"))]
  #[error("WOFF conversion failed: {0}")]
  Woff(#[source] Box<dyn std::error::Error + Send + Sync + 'static>),
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
pub(crate) enum FontFormat {
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
      let ttf = wuff::decompress_woff2(&source).map_err(|e| FontError::Woff(Box::new(e)))?;
      Ok(ttf)
    }
    #[cfg(feature = "woff")]
    FontFormat::Woff => {
      let ttf = wuff::decompress_woff1(&source).map_err(|e| FontError::Woff(Box::new(e)))?;
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
  static SHARED_RESOLVED_GLYPH_CACHE: RefCell<HashMap<u64, ResolvedGlyph>> =
    RefCell::new(HashMap::new());
}

const RESOLVED_GLYPH_CACHE_MAX_ENTRIES: usize = 4096;

fn resolved_glyph_cache_key(
  font_id: u64,
  font_index: u32,
  font_size: f32,
  coords: &[F2Dot14],
  embolden: Option<f32>,
  skew: Option<f32>,
  glyph_id: u32,
) -> u64 {
  use xxhash_rust::xxh3::Xxh3;
  let mut h = Xxh3::new();
  h.update(&font_id.to_le_bytes());
  h.update(&font_index.to_le_bytes());
  h.update(&font_size.to_le_bytes());
  for c in coords {
    h.update(&c.to_bits().to_le_bytes());
  }
  match embolden {
    Some(e) => {
      h.update(&[1u8]);
      h.update(&e.to_le_bytes());
    }
    None => h.update(&[0u8]),
  }
  match skew {
    Some(s) => {
      h.update(&[1u8]);
      h.update(&s.to_le_bytes());
    }
    None => h.update(&[0u8]),
  }
  h.update(&glyph_id.to_le_bytes());
  h.digest()
}

fn with_layout_context<R>(f: impl FnOnce(&mut LayoutContext<InlineBrush>) -> R) -> R {
  LAYOUT_CONTEXT.with(|cell| match cell.try_borrow_mut() {
    Ok(mut ctx) => f(&mut ctx),
    Err(_) => f(&mut LayoutContext::new()),
  })
}

/// A font family produced by [`Fonts::register`], with the faces it contains.
#[derive(Clone, Debug, serde::Serialize)]
pub struct RegisteredFamily {
  /// Family name as stored by the font system (normalized; reflects any override).
  pub name: String,
  /// Faces registered under this family.
  pub faces: Vec<RegisteredFace>,
}

/// A single face within a [`RegisteredFamily`].
#[derive(Clone, Debug, serde::Serialize)]
pub struct RegisteredFace {
  /// Weight class, typically `1.0..=1000.0`.
  pub weight: f32,
  /// CSS `font-style` value (`normal`, `italic`, or `oblique [<angle>deg]`).
  pub style: String,
  /// Width as a percentage of normal (e.g. `100.0`).
  pub width: f32,
  /// Index of the face within its source collection.
  pub index: u32,
}

fn font_style_css(style: FontStyle) -> String {
  match style {
    FontStyle::Normal => "normal".to_string(),
    FontStyle::Italic => "italic".to_string(),
    FontStyle::Oblique(None) => "oblique".to_string(),
    FontStyle::Oblique(Some(angle)) => format!("oblique {angle}deg"),
  }
}

/// The registry of fonts available to a renderer.
///
/// Registration is the only mutation; afterwards the assembled parley context is immutable
/// and shared as `&self` across concurrent renders. Each render takes a cheap working clone
/// (see [`RenderContext`]) for parley's `&mut` query API, so no per-thread or global state
/// is needed.
#[derive(Clone)]
pub struct Fonts {
  inner: parley::FontContext,
  /// Maps a logical family name (the name authors write in `font-family`) to the
  /// unique internal names of the subset families registered under it. Populated by
  /// [`FontResource::subset_of`]; consulted when a render expands a `font-family` into
  /// its per-coverage subset stack. A `BTreeSet` so the stack is ordered by family name,
  /// not registration arrival order (callers often register concurrently) — selection
  /// among subsets that overlap a codepoint stays deterministic. Shared (immutable after
  /// registration) so a render can read it without borrowing the parley context.
  groups: Arc<HashMap<String, BTreeSet<String>>>,
  /// Every registered family name in registration order. The fallback bucket is built from
  /// this so its per-script priority is deterministic; `fontique`'s `family_names()` iterates
  /// a `HashMap` (hash order), which would otherwise make font selection vary per render.
  order: Vec<String>,
}

impl Default for Fonts {
  fn default() -> Self {
    Self {
      inner: parley::FontContext {
        collection: Collection::new(CollectionOptions {
          system_fonts: false,
          shared: false,
        }),
        source_cache: Default::default(),
      },
      groups: Arc::new(HashMap::new()),
      order: Vec::new(),
    }
  }
}

/// A render-local font handle. `groups` sits outside the `RefCell` so a render can expand
/// `font-family` without taking the parley-context borrow that building the tree holds.
#[derive(Clone)]
pub struct FontsSnapshot {
  context: Rc<RefCell<Fonts>>,
  pub(crate) groups: Arc<HashMap<String, BTreeSet<String>>>,
}

impl FontsSnapshot {
  /// Mutable access to the render-local parley context. Callers must not re-enter while the
  /// borrow is held (layout measures inline boxes before building the parley tree).
  pub(crate) fn with_context<R>(&self, f: impl FnOnce(&mut Fonts) -> R) -> R {
    f(&mut self.context.borrow_mut())
  }
}

impl Fonts {
  /// Render-local snapshot with no extra fallbacks.
  pub fn snapshot(&self) -> FontsSnapshot {
    self.snapshot_with_fallbacks(None)
  }

  /// Render-local snapshot whose fallback bucket carries the given families.
  pub fn snapshot_with_fallbacks(&self, fallbacks: Option<&[String]>) -> FontsSnapshot {
    let mut cloned = self.inner.clone();

    if let Some(names) = fallbacks {
      // A name may be a logical subset family; expand it to its registered subset names so
      // the fallback bucket carries the whole stack, matching `font-family` expansion.
      let mut family_ids = Vec::new();
      for name in names {
        match self.groups.get(name) {
          Some(subsets) => {
            family_ids.extend(
              subsets
                .iter()
                .filter_map(|n| cloned.collection.family_id(n)),
            );
          }
          None => family_ids.extend(cloned.collection.family_id(name)),
        }
      }

      for (script, _) in Script::all_samples() {
        cloned.collection.set_fallbacks(
          FallbackKey::new(*script, None),
          family_ids.clone().into_iter(),
        );
      }
    } else {
      // Registration order, not `family_names()` (hash order), so font selection is stable.
      let family_ids = self
        .order
        .iter()
        .filter_map(|name| cloned.collection.family_id(name))
        .collect::<Vec<_>>();

      for (script, _) in Script::all_samples() {
        cloned.collection.set_fallbacks(
          FallbackKey::new(*script, None),
          family_ids.clone().into_iter(),
        );
      }
    }

    FontsSnapshot {
      context: Rc::new(RefCell::new(Self {
        inner: cloned,
        groups: self.groups.clone(),
        order: self.order.clone(),
      })),
      groups: self.groups.clone(),
    }
  }

  pub(crate) fn resolve_glyphs(
    &self,
    run: &GlyphRun<'_, InlineBrush>,
    font_ref: FontRef,
    glyph_ids: impl Iterator<Item = u32> + Clone,
  ) -> HashMap<u32, ResolvedGlyph> {
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
    // Only synthesize bold at the CSS bold threshold (>= 600), matching browsers; a lighter
    // requested weight keeps the regular face rather than faux-bolding it.
    let embolden = (!has_emoji_cluster
      && run.run().synthesis().embolden()
      && run.run().font_attrs().weight.value() >= BOLD_THRESHOLD
      && run.style().brush.font_synthesis.weight.is_allowed())
    .then_some(synthesis_embolden_strength(font_size));
    let skew = run
      .run()
      .synthesis()
      .skew()
      .filter(|_| !has_emoji_cluster)
      .filter(|_| run.style().brush.font_synthesis.style.is_allowed())
      .map(|degrees| -degrees);

    let font_id = run.run().font().data.id();
    let font_index = run.run().font().index;
    let resolver = GlyphResolveContext {
      outline_glyphs: font_ref.outline_glyphs(),
      color_glyphs: font_ref.color_glyphs(),
      bitmap_strikes: font_ref.bitmap_strikes(),
      font_size,
      size: Size::new(font_size),
      location: LocationRef::new(&normalized_coords),
      embolden,
      skew,
    };

    let mut result: HashMap<u32, ResolvedGlyph> = HashMap::new();
    for glyph_id in glyph_ids {
      if let Entry::Vacant(slot) = result.entry(glyph_id) {
        let key = resolved_glyph_cache_key(
          font_id,
          font_index,
          font_size,
          &normalized_coords,
          embolden,
          skew,
          glyph_id,
        );
        let cached = SHARED_RESOLVED_GLYPH_CACHE.with(|c| c.borrow().get(&key).cloned());
        let glyph = if let Some(g) = cached {
          Some(g)
        } else {
          let resolved = resolver.resolve_glyph(glyph_id);
          if let Some(g) = resolved.as_ref() {
            SHARED_RESOLVED_GLYPH_CACHE.with(|c| {
              let mut cache = c.borrow_mut();
              if cache.len() > RESOLVED_GLYPH_CACHE_MAX_ENTRIES {
                cache.clear();
              }
              cache.insert(key, g.clone());
            });
          }
          resolved
        };
        if let Some(g) = glyph {
          slot.insert(g);
        }
      }
    }

    result
  }

  /// Registers a font, decoding it and skipping fonts already registered in this
  /// context (deduped by content + family name). Returns the families it produced.
  pub fn register(&mut self, font: FontResource) -> Result<Vec<RegisteredFamily>, FontError> {
    let FontResource {
      source,
      info_override,
      generic_family,
      subset_of,
    } = font;

    let blob = source.into_blob()?;
    let axes = info_override.as_ref().map(FontOverride::resolved_axes);
    let info_override = info_override
      .as_ref()
      .zip(axes.as_deref())
      .map(|(info, axes)| info.to_parley(axes));
    let registered_fonts = self.inner.collection.register_fonts(blob, info_override);

    let mut families = Vec::with_capacity(registered_fonts.len());
    for (family, faces) in registered_fonts {
      let faces = faces
        .iter()
        .map(|face| RegisteredFace {
          weight: face.weight().value(),
          style: font_style_css(face.style()),
          width: face.width().percentage(),
          index: face.index(),
        })
        .collect();
      let name = self
        .inner
        .collection
        .family_name(family)
        .unwrap_or_default()
        .to_string();

      if !self.order.contains(&name) {
        self.order.push(name.clone());
      }

      if let Some(logical) = &subset_of {
        Arc::make_mut(&mut self.groups)
          .entry(logical.clone())
          .or_default()
          .insert(name.clone());
      }

      families.push(RegisteredFamily { name, faces });

      if let Some(generic_family) = generic_family {
        self
          .inner
          .collection
          .append_generic_families(generic_family.into_parlance(), once(family));
      }
    }

    Ok(families)
  }
}

impl RenderContext {
  /// First available font's line spacing for `families`/`attributes`, scaled to `font_size`.
  pub(crate) fn first_font_line_spacing<'a>(
    &self,
    families: impl IntoIterator<Item = QueryFamily<'a>>,
    attributes: Attributes,
    font_size: f32,
  ) -> Option<f32> {
    self.fonts.with_context(|fonts| {
      let mut query = fonts.inner.collection.query(&mut fonts.inner.source_cache);
      let mut result = None;

      query.set_families(families);
      query.set_attributes(attributes);

      query.matches_with(|font| {
        let Ok(font_ref) = FontRef::from_index(font.blob.data(), font.index) else {
          return QueryStatus::Continue;
        };
        let metrics = font_ref.metrics(Size::new(font_size), LocationRef::default());
        result = Some(metrics.ascent + metrics.descent + metrics.leading);
        QueryStatus::Stop
      });

      result
    })
  }

  /// Builds an inline layout with the given root style.
  pub(crate) fn tree_builder(
    &self,
    root_style: TextStyle<'_, '_, InlineBrush>,
    func: impl FnOnce(&mut TreeBuilder<'_, InlineBrush>),
  ) -> (InlineLayout, String) {
    self.fonts.with_context(|fonts| {
      with_layout_context(|layout| {
        let mut builder = layout.tree_builder(&mut fonts.inner, 1.0, true, &root_style);
        func(&mut builder);
        builder.build()
      })
    })
  }
}

/// A font source buffer. Construct from raw bytes via `From`; woff/woff2 are
/// decompressed internally when the font is registered.
#[derive(Debug)]
pub struct FontSource<'a> {
  bytes: Cow<'a, [u8]>,
  /// Whether `bytes` is already decompressed (woff/woff2 expanded to raw sfnt).
  is_decoded: bool,
}

impl<'a, T> From<T> for FontSource<'a>
where
  T: Into<Cow<'a, [u8]>>,
{
  fn from(value: T) -> Self {
    Self {
      bytes: value.into(),
      is_decoded: false,
    }
  }
}

impl<'a> FontSource<'a> {
  fn into_decoded(self) -> Result<Self, FontError> {
    if self.is_decoded {
      return Ok(self);
    }

    Ok(Self {
      bytes: Cow::Owned(load_font(self.bytes, None)?),
      is_decoded: true,
    })
  }

  fn into_blob(self) -> Result<Blob<u8>, FontError> {
    let decoded = if self.is_decoded {
      self.bytes.into_owned()
    } else {
      load_font(self.bytes, None)?
    };

    Ok(Blob::new(Arc::new(decoded)))
  }
}

impl<'a> AsRef<[u8]> for FontSource<'a> {
  fn as_ref(&self) -> &[u8] {
    &self.bytes
  }
}

/// A CSS generic font family a registered font fulfills, exposed as named
/// constants so callers need not depend on `parley`.
/// <https://drafts.csswg.org/css-fonts/#generic-font-families>
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GenericFamily(ParleyGenericFamily);

impl GenericFamily {
  /// `serif`
  pub const SERIF: Self = Self(ParleyGenericFamily::Serif);
  /// `sans-serif`
  pub const SANS_SERIF: Self = Self(ParleyGenericFamily::SansSerif);
  /// `monospace`
  pub const MONOSPACE: Self = Self(ParleyGenericFamily::Monospace);
  /// `cursive`
  pub const CURSIVE: Self = Self(ParleyGenericFamily::Cursive);
  /// `fantasy`
  pub const FANTASY: Self = Self(ParleyGenericFamily::Fantasy);
  /// `system-ui`
  pub const SYSTEM_UI: Self = Self(ParleyGenericFamily::SystemUi);
  /// `ui-serif`
  pub const UI_SERIF: Self = Self(ParleyGenericFamily::UiSerif);
  /// `ui-sans-serif`
  pub const UI_SANS_SERIF: Self = Self(ParleyGenericFamily::UiSansSerif);
  /// `ui-monospace`
  pub const UI_MONOSPACE: Self = Self(ParleyGenericFamily::UiMonospace);
  /// `ui-rounded`
  pub const UI_ROUNDED: Self = Self(ParleyGenericFamily::UiRounded);
  /// `emoji`
  pub const EMOJI: Self = Self(ParleyGenericFamily::Emoji);
  /// `math`
  pub const MATH: Self = Self(ParleyGenericFamily::Math);
  /// `fangsong`
  pub const FANG_SONG: Self = Self(ParleyGenericFamily::FangSong);

  pub(crate) fn into_parlance(self) -> ParleyGenericFamily {
    self.0
  }
}

/// Overrides for a registered font's metadata, letting callers rename its
/// family or pin weight/style/width/axes regardless of what the font file
/// itself declares.
#[derive(Debug, Default, Clone)]
pub struct FontOverride {
  /// Family name to register the font under, instead of its embedded name.
  pub family_name: Option<Arc<str>>,
  /// Font weight (CSS numeric, e.g. `400.0`) to use instead of the embedded one.
  pub weight: Option<f32>,
  /// Font style (slant) to use instead of the embedded one.
  pub style: Option<CssFontStyle>,
  /// Font width as a percentage (e.g. `100.0` for normal) to use instead of the
  /// embedded one.
  pub width: Option<f32>,
  /// Default values for named variation axes (four-byte OpenType tags). Axes not
  /// present in the font are ignored, as are tags that are not valid.
  pub axes: Vec<(String, f32)>,
}

impl FontOverride {
  fn resolved_axes(&self) -> Vec<(Tag, f32)> {
    self
      .axes
      .iter()
      .filter_map(|(tag, value)| {
        Tag::new_checked(tag.as_bytes())
          .ok()
          .map(|tag| (tag, *value))
      })
      .collect()
  }

  fn to_parley<'a>(&'a self, axes: &'a [(Tag, f32)]) -> FontInfoOverride<'a> {
    FontInfoOverride {
      family_name: self.family_name.as_deref(),
      width: self.width.map(FontWidth::from_percentage),
      style: self.style.map(CssFontStyle::into_parlance),
      weight: self.weight.map(FontWeight::new),
      axes: (!axes.is_empty()).then_some(axes),
    }
  }
}

#[derive(Debug)]
/// Information of a font resource
pub struct FontResource<'a> {
  source: FontSource<'a>,
  info_override: Option<FontOverride>,
  generic_family: Option<GenericFamily>,
  /// Logical family this font is a coverage subset of (see [`FontResource::subset_of`]).
  subset_of: Option<String>,
}

impl<'a> FontResource<'a> {
  /// Create a new font to load
  pub fn new(source: impl Into<FontSource<'a>>) -> Self {
    Self {
      source: source.into(),
      info_override: None,
      generic_family: None,
      subset_of: None,
    }
  }

  /// Set font metadata overrides
  pub fn override_info(self, info_override: FontOverride) -> Self {
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

  /// Marks this font as a coverage subset of the logical family `logical`.
  ///
  /// Subsets sharing a logical family must each register under a UNIQUE family name
  /// (via [`FontResource::override_info`]) so the font system keeps them as distinct
  /// families — same-named faces collapse into one and never fall through on coverage.
  /// A render then expands `font-family: {logical}` into all its subsets, in
  /// registration order, letting the shaper pick the subset that covers each cluster.
  pub fn subset_of(self, logical: impl Into<String>) -> Self {
    Self {
      subset_of: Some(logical.into()),
      ..self
    }
  }

  /// Convert to resolved font resource, decompressing woff2/woff into a raw buffer.
  pub fn into_resolved(self) -> Result<Self, FontError> {
    let source = self.source.into_decoded()?;
    Ok(Self {
      source,
      info_override: self.info_override,
      generic_family: self.generic_family,
      subset_of: self.subset_of,
    })
  }
}
