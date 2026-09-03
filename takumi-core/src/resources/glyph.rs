//! Glyph rasterization: resolving a shaped glyph id to an embedded bitmap or a
//! vector outline (including COLR color layers), independent of the font
//! registry. The [`Fonts`](super::font::Fonts) registry drives this through
//! [`GlyphResolveContext`], memoizing the results per worker thread.

use image::{Rgba, RgbaImage};
use skrifa::{
  GlyphId,
  bitmap::{BitmapData, BitmapGlyph, BitmapStrikes, Origin},
  color::{
    Brush, ColorGlyphCollection, ColorGlyphFormat, ColorPainter, CompositeMode,
    PaintCachedColorGlyph, Transform,
  },
  instance::{LocationRef, Size},
  outline::{DrawSettings, OutlineGlyphCollection, OutlinePen},
  raw::types::BoundingBox,
};

use crate::{
  geometry::{PathCommand as Command, Placement, Point},
  resources::{image_buffer::ImageBuffer, image_decoder::decode_png},
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
  pub placement: Placement,
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

impl ResolvedGlyph {
  /// Approximate retained size in bytes, for glyph-cache budgeting. Element
  /// sizes are exact; the constant covers map-slot and allocator overhead.
  pub(crate) fn estimated_bytes(&self) -> usize {
    const ENTRY_OVERHEAD: usize = 64;

    match self {
      Self::Bitmap(bitmap) => bitmap.image.data().len() + ENTRY_OVERHEAD,
      Self::Outline(ResolvedOutlineGlyph::Plain { paths, .. }) => {
        paths.capacity() * size_of::<Command>() + ENTRY_OVERHEAD
      }
      Self::Outline(ResolvedOutlineGlyph::Color { paths, layers, .. }) => {
        let layer_commands: usize = layers.iter().map(|layer| layer.paths.capacity()).sum();
        (paths.capacity() + layer_commands) * size_of::<Command>()
          + layers.capacity() * size_of::<ResolvedColorLayer>()
          + ENTRY_OVERHEAD
      }
    }
  }

  /// Conservative ink extents relative to the glyph's pen origin, in device
  /// pixels: `(min_x, min_y, max_x, max_y)`. Curve control points are included,
  /// so the box may slightly overestimate but never undercuts, and the
  /// faux-bold stroke outset is accounted for.
  pub(crate) fn ink_extents(&self) -> Option<(f32, f32, f32, f32)> {
    match self {
      Self::Bitmap(bitmap) => {
        let placement = bitmap.placement;
        (placement.width > 0 && placement.height > 0).then(|| {
          (
            placement.left as f32,
            placement.top as f32,
            (placement.left + placement.width as i32) as f32,
            (placement.top + placement.height as i32) as f32,
          )
        })
      }
      Self::Outline(outline) => {
        let mut min = Point {
          x: f32::INFINITY,
          y: f32::INFINITY,
        };
        let mut max = Point {
          x: f32::NEG_INFINITY,
          y: f32::NEG_INFINITY,
        };
        let mut include = |point: &Point<f32>| {
          min.x = min.x.min(point.x);
          min.y = min.y.min(point.y);
          max.x = max.x.max(point.x);
          max.y = max.y.max(point.y);
        };

        for command in outline.paths() {
          match command {
            Command::MoveTo(point) | Command::LineTo(point) => include(point),
            Command::QuadTo(control, point) => {
              include(control);
              include(point);
            }
            Command::CubicTo(control1, control2, point) => {
              include(control1);
              include(control2);
              include(point);
            }
            Command::Close => {}
          }
        }

        if !min.x.is_finite() {
          return None;
        }

        let outset = outline.embolden().map_or(0.0, |embolden| embolden / 2.0);
        Some((
          min.x - outset,
          min.y - outset,
          max.x + outset,
          max.y + outset,
        ))
      }
    }
  }
}

/// CSS `kBoldThreshold` — weights at or above this synthesize bold when no bolder face
/// exists; lighter weights do not. https://drafts.csswg.org/css-fonts-4/#font-weight-prop
pub(crate) const BOLD_THRESHOLD: f32 = 600.0;

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

/// Type-erased [`OutlinePen`]: skrifa monomorphizes its whole CFF/glyf
/// evaluator per pen type, which costs ~100KB of wasm per concrete pen.
/// Route every `OutlineGlyph::draw` call through this instead.
pub struct ErasedPen<'a>(pub &'a mut dyn OutlinePen);

impl OutlinePen for ErasedPen<'_> {
  fn move_to(&mut self, x: f32, y: f32) {
    self.0.move_to(x, y);
  }

  fn line_to(&mut self, x: f32, y: f32) {
    self.0.line_to(x, y);
  }

  fn quad_to(&mut self, cx0: f32, cy0: f32, x: f32, y: f32) {
    self.0.quad_to(cx0, cy0, x, y);
  }

  fn curve_to(&mut self, cx0: f32, cy0: f32, cx1: f32, cy1: f32, x: f32, y: f32) {
    self.0.curve_to(cx0, cy0, cx1, cy1, x, y);
  }

  fn close(&mut self) {
    self.0.close();
  }
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

pub(crate) struct GlyphResolveContext<'a> {
  pub(crate) outline_glyphs: OutlineGlyphCollection<'a>,
  pub(crate) color_glyphs: ColorGlyphCollection<'a>,
  pub(crate) bitmap_strikes: BitmapStrikes<'a>,
  pub(crate) font_size: f32,
  pub(crate) size: Size,
  pub(crate) location: LocationRef<'a>,
  pub(crate) skew: Option<f32>,
  pub(crate) embolden: Option<f32>,
}

impl<'a> GlyphResolveContext<'a> {
  pub(crate) fn resolve_glyph(&self, glyph_id: u32) -> Option<ResolvedGlyph> {
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
    .draw(
      DrawSettings::unhinted(size, location),
      &mut ErasedPen(&mut pen),
    )
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
      let expected = (bitmap.width as usize)
        .checked_mul(bitmap.height as usize)?
        .checked_mul(4)?;
      if bytes.len() < expected {
        return None;
      }

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
    placement: Placement {
      left: (bitmap.inner_bearing_x * scale_x).round() as i32,
      top: (top * scale_y).round() as i32,
      width,
      height,
    },
  })
}
