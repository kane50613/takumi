use std::{
  borrow::Cow,
  cell::RefCell,
  collections::{HashMap, hash_map::Entry},
  iter::once,
  rc::Rc,
  sync::Arc,
};

use image::{Rgba, RgbaImage};
use parley::{
  GenericFamily, GlyphRun, LayoutContext, TextStyle, TreeBuilder,
  fontique::{
    Attributes, Blob, Collection, CollectionOptions, FallbackKey, FontInfoOverride, FontStyle,
    QueryFamily, QueryStatus, Script, ScriptExt,
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
  raw::types::{BoundingBox, F2Dot14},
};
use thiserror::Error;
use tiny_skia::{IntSize, PathSegment as Command, Pixmap};

use crate::{
  context::RenderContext,
  layout::inline::{InlineBrush, InlineLayout},
  resources::{image_buffer::ImageBuffer, image_decoder::decode_png},
};

fn pixmap_from_image_buffer(buffer: ImageBuffer) -> Option<Pixmap> {
  let size = IntSize::from_wh(buffer.width(), buffer.height())?;
  Pixmap::from_vec(buffer.into_data(), size)
}

#[derive(Clone)]
pub enum ResolvedGlyph {
  Bitmap(ResolvedBitmapGlyph),
  Outline(ResolvedOutlineGlyph),
}

#[derive(Clone)]
pub struct ResolvedBitmapGlyph {
  pub pixmap: Pixmap,
  pub scale_x: f32,
  pub scale_y: f32,
  pub placement: ResolvedGlyphPlacement,
}

impl ResolvedBitmapGlyph {
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
pub enum ResolvedOutlineGlyph {
  Plain {
    paths: Vec<Command>,
    embolden: Option<f32>,
    cache_signature: u64,
  },
  Color {
    paths: Vec<Command>,
    layers: Vec<ResolvedColorLayer>,
    cache_signature: u64,
  },
}

#[derive(Clone)]
pub struct ResolvedColorLayer {
  pub paths: Vec<Command>,
  pub palette_index: u16,
  pub alpha: f32,
}

#[derive(Clone, Copy)]
pub struct ResolvedGlyphPlacement {
  pub left: i32,
  pub top: i32,
  pub width: u32,
  pub height: u32,
}

impl ResolvedOutlineGlyph {
  pub fn paths(&self) -> &[Command] {
    match self {
      Self::Plain { paths, .. } | Self::Color { paths, .. } => paths,
    }
  }

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

  pub fn embolden(&self) -> Option<f32> {
    match self {
      Self::Plain { embolden, .. } => *embolden,
      Self::Color { .. } => None,
    }
  }

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

fn decode_bitmap_image(bitmap: &BitmapGlyph<'_>) -> Option<(Pixmap, Origin)> {
  let pixmap = match &bitmap.data {
    BitmapData::Png(bytes) => pixmap_from_image_buffer(decode_png(bytes).ok()?)?,
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
      pixmap_from_image_buffer(ImageBuffer::from_rgba(Cow::Owned(image))?)?
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
  static SHARED_RESOLVED_GLYPH_CACHE: RefCell<HashMap<u64, ResolvedGlyph>> =
    RefCell::new(HashMap::new());
}

const RESOLVED_GLYPH_CACHE_MAX_ENTRIES: usize = 4096;

fn resolved_glyph_cache_key(
  font_data_ptr: usize,
  font_index: u32,
  font_size: f32,
  coords: &[F2Dot14],
  embolden: Option<f32>,
  skew: Option<f32>,
  glyph_id: u32,
) -> u64 {
  use xxhash_rust::xxh3::Xxh3;
  let mut h = Xxh3::new();
  h.update(&font_data_ptr.to_le_bytes());
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
  /// unique internal names of the subset families registered under it, in registration
  /// order. Populated by [`FontResource::subset_of`]; consulted when a render expands a
  /// `font-family` into its per-coverage subset stack.
  groups: HashMap<String, Vec<String>>,
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
      groups: HashMap::new(),
    }
  }
}

pub type FontsSnapshot = Rc<RefCell<Fonts>>;

impl Fonts {
  pub fn snapshot(&self) -> FontsSnapshot {
    self.snapshot_with_fallbacks(None)
  }

  pub fn snapshot_with_fallbacks(&self, fallbacks: Option<&[String]>) -> FontsSnapshot {
    let mut cloned = self.inner.clone();

    if let Some(names) = fallbacks {
      let family_ids = names
        .iter()
        .filter_map(|name| cloned.collection.family_id(name))
        .collect::<Vec<_>>();

      for (script, _) in Script::all_samples() {
        cloned.collection.set_fallbacks(
          FallbackKey::new(*script, None),
          family_ids.clone().into_iter(),
        );
      }
    } else {
      let registered_families = cloned
        .collection
        .family_names()
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();

      let family_ids = registered_families
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

    Rc::new(RefCell::new(Self {
      inner: cloned,
      groups: self.groups.clone(),
    }))
  }

  /// The subset family names registered under `logical` via [`FontResource::subset_of`],
  /// in registration order, or `None` if `logical` names no subset group.
  pub(crate) fn subset_group(&self, logical: &str) -> Option<&[String]> {
    self.groups.get(logical).map(Vec::as_slice)
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

    let font_data_ptr = run.run().font().data.as_ref().as_ptr() as usize;
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
          font_data_ptr,
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

      if let Some(logical) = &subset_of {
        let names = self.groups.entry(logical.clone()).or_default();
        if !names.contains(&name) {
          names.push(name.clone());
        }
      }

      families.push(RegisteredFamily { name, faces });

      if let Some(generic_family) = generic_family {
        self
          .inner
          .collection
          .append_generic_families(generic_family, once(family));
      }
    }

    Ok(families)
  }
}

impl RenderContext {
  /// Mutable access to the render-local fonts. Callers must not re-enter while the
  /// borrow is held (layout measures inline boxes before building the parley tree).
  pub(crate) fn with_fonts<R>(&self, f: impl FnOnce(&mut Fonts) -> R) -> R {
    f(&mut self.fonts.borrow_mut())
  }

  /// First available font's line spacing for `families`/`attributes`, scaled to `font_size`.
  pub(crate) fn first_font_line_spacing<'a>(
    &self,
    families: impl IntoIterator<Item = QueryFamily<'a>>,
    attributes: Attributes,
    font_size: f32,
  ) -> Option<f32> {
    self.with_fonts(|fonts| {
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
    self.with_fonts(|fonts| {
      with_layout_context(|layout| {
        let mut builder = layout.tree_builder(&mut fonts.inner, 1.0, true, &root_style);
        func(&mut builder);
        builder.build()
      })
    })
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
      Self::Raw(raw) => Ok(Self::Blob(decode_font(raw)?)),
      Self::Blob(_) => Ok(self),
    }
  }

  fn into_blob(self) -> Result<Blob<u8>, FontError> {
    match self {
      Self::Raw(raw) => decode_font(raw),
      Self::Blob(blob) => Ok(blob),
    }
  }
}

/// Decodes raw font bytes (decompressing woff2/woff) into a blob.
fn decode_font(raw: Cow<'_, [u8]>) -> Result<Blob<u8>, FontError> {
  Ok(Blob::new(Arc::new(load_font(raw, None)?)))
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
    let source = self.source.into_blob_variant()?;
    Ok(Self {
      source,
      info_override: self.info_override,
      generic_family: self.generic_family,
      subset_of: self.subset_of,
    })
  }
}
