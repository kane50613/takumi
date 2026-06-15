use std::{
  borrow::Cow,
  cell::RefCell,
  collections::{HashMap, hash_map::Entry},
  iter::once,
  ops::{Deref, DerefMut},
  sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
  },
};

use image::{Rgba, RgbaImage};
use parley::{
  GenericFamily, GlyphRun, LayoutContext, TextStyle, TreeBuilder,
  fontique::{
    Attributes, Blob, Collection, CollectionOptions, FallbackKey, FontInfoOverride, QueryFamily,
    QueryStatus, Script, ScriptExt,
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
  layout::inline::{InlineBrush, InlineLayout},
  resources::{image_buffer::ImageBuffer, image_decoder::decode_png},
};

fn pixmap_from_image_buffer(buffer: ImageBuffer) -> Option<Pixmap> {
  let size = IntSize::from_wh(buffer.width(), buffer.height())?;
  Pixmap::from_vec(buffer.into_data(), size)
}

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
    cache_signature: u64,
  },
  Color {
    paths: Vec<Command>,
    layers: Vec<ResolvedColorLayer>,
    cache_signature: u64,
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

  pub(crate) fn cache_signature(&self) -> u64 {
    match self {
      Self::Plain {
        cache_signature, ..
      }
      | Self::Color {
        cache_signature, ..
      } => *cache_signature,
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
  static FONT_CONTEXT_CACHE: RefCell<Option<(u64, u64, parley::FontContext)>> =
    const { RefCell::new(None) };
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

static FONT_CONTEXT_NEXT_ID: AtomicU64 = AtomicU64::new(1);

/// A context for managing fonts in the rendering system.
pub struct FontContext {
  id: u64,
  version: AtomicU64,
  inner: parley::FontContext,
}

impl Clone for FontContext {
  fn clone(&self) -> Self {
    Self {
      id: FONT_CONTEXT_NEXT_ID.fetch_add(1, Ordering::Relaxed),
      version: AtomicU64::new(self.version.load(Ordering::Relaxed)),
      inner: self.inner.clone(),
    }
  }
}

impl Default for FontContext {
  fn default() -> Self {
    Self {
      id: FONT_CONTEXT_NEXT_ID.fetch_add(1, Ordering::Relaxed),
      version: AtomicU64::new(0),
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
  #[inline]
  fn with_inner_mut<R>(&self, f: impl FnOnce(&mut parley::FontContext) -> R) -> R {
    let current_version = self.version.load(Ordering::Relaxed);
    let id = self.id;
    let outcome = FONT_CONTEXT_CACHE.with(|cell| {
      let Ok(mut borrow) = cell.try_borrow_mut() else {
        return Err(f);
      };
      let needs_clone = match &*borrow {
        Some((cached_id, cached_version, _)) => {
          *cached_id != id || *cached_version != current_version
        }
        None => true,
      };
      if needs_clone {
        *borrow = Some((id, current_version, self.inner.clone()));
      }
      let Some((_, _, inner)) = borrow.as_mut() else {
        return Err(f);
      };
      Ok(f(inner))
    });
    match outcome {
      Ok(r) => r,
      Err(f) => f(&mut self.inner.clone()),
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
    let embolden = (!has_emoji_cluster
      && run.run().synthesis().embolden()
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

  /// Line spacing (ascent + descent + leading) of the first available font
  /// matching the given families and attributes, scaled to `font_size`.
  /// Used as the `lh`/`rlh` basis when `line-height: normal`.
  pub(crate) fn first_font_line_spacing<'a>(
    &self,
    families: impl IntoIterator<Item = QueryFamily<'a>>,
    attributes: Attributes,
    font_size: f32,
  ) -> Option<f32> {
    self.with_inner_mut(|ctx| {
      let parley::FontContext {
        collection,
        source_cache,
      } = ctx;
      let mut query = collection.query(source_cache);
      query.set_families(families);
      query.set_attributes(attributes);
      let mut result = None;
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

  /// Create an inline layout with the given root style and function
  pub(crate) fn tree_builder(
    &self,
    root_style: TextStyle<'_, '_, InlineBrush>,
    func: impl FnOnce(&mut TreeBuilder<'_, InlineBrush>),
  ) -> (InlineLayout, String) {
    self.with_inner_mut(|font_context| {
      with_layout_context(|layout_context| {
        let mut builder = layout_context.tree_builder(font_context, 1.0, true, &root_style);
        func(&mut builder);
        builder.build()
      })
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

    self.version.fetch_add(1, Ordering::Relaxed);

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
