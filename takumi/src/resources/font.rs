use std::{
  borrow::Cow,
  collections::{HashMap, HashSet},
  iter::once,
  ops::{Deref, DerefMut},
  sync::Arc,
};

use image::{
  Rgba, RgbaImage,
  imageops::{FilterType, resize},
};
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
use zeno::{Angle as ZenoAngle, Command, Transform as ZenoTransform};

use crate::{
  layout::inline::{InlineBrush, InlineLayout},
  resources::image_decoder::decode_png,
};

#[derive(Clone)]
pub(crate) enum ResolvedGlyph {
  Bitmap(ResolvedBitmapGlyph),
  Outline(ResolvedOutlineGlyph),
}

#[derive(Clone)]
pub(crate) struct ResolvedBitmapGlyph {
  pub(crate) image: RgbaImage,
  pub(crate) placement: ResolvedGlyphPlacement,
}

#[derive(Clone)]
pub(crate) struct ResolvedOutlineGlyph {
  pub(crate) paths: Vec<Command>,
  pub(crate) color_layers: Option<Vec<ResolvedColorLayer>>,
  pub(crate) embolden: Option<f32>,
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
    self.paths.push(Command::MoveTo((x, -y).into()));
  }

  fn line_to(&mut self, x: f32, y: f32) {
    self.paths.push(Command::LineTo((x, -y).into()));
  }

  fn quad_to(&mut self, cx0: f32, cy0: f32, x: f32, y: f32) {
    self
      .paths
      .push(Command::QuadTo((cx0, -cy0).into(), (x, -y).into()));
  }

  fn curve_to(&mut self, cx0: f32, cy0: f32, cx1: f32, cy1: f32, x: f32, y: f32) {
    self.paths.push(Command::CurveTo(
      (cx0, -cy0).into(),
      (cx1, -cy1).into(),
      (x, -y).into(),
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

fn transform_commands(paths: &mut [Command], transform: &ZenoTransform) {
  for command in paths {
    *command = command.transform(transform);
  }
}

fn decode_bitmap_image(bitmap: BitmapGlyph<'_>) -> Option<(RgbaImage, Origin)> {
  let image = match bitmap.data {
    BitmapData::Png(bytes) => decode_png(bytes).ok()?,
    BitmapData::Bgra(bytes) => RgbaImage::from_fn(bitmap.width, bitmap.height, |x, y| {
      let index = ((y * bitmap.width + x) * 4) as usize;
      Rgba([
        bytes[index + 2],
        bytes[index + 1],
        bytes[index],
        bytes[index + 3],
      ])
    }),
    BitmapData::Mask(_) => return None,
  };

  Some((image, bitmap.placement_origin))
}

fn scale_bitmap_glyph(bitmap: BitmapGlyph<'_>, font_size: f32) -> Option<ResolvedBitmapGlyph> {
  let (image, origin) = decode_bitmap_image(bitmap.clone())?;
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
  let image = if width == image.width() && height == image.height() {
    image
  } else {
    resize(&image, width, height, FilterType::Triangle)
  };
  let top = match origin {
    Origin::TopLeft => bitmap.inner_bearing_y,
    Origin::BottomLeft => bitmap.inner_bearing_y + bitmap.height as f32,
  };

  Some(ResolvedBitmapGlyph {
    image,
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

  Some(ResolvedOutlineGlyph {
    paths,
    color_layers: Some(color_layers),
    embolden: None,
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
    // Collect unique glyph IDs to avoid duplicate work
    let unique_glyph_ids: HashSet<u32> = glyph_ids.collect();

    let mut result = HashMap::new();

    if unique_glyph_ids.is_empty() {
      return result;
    }

    let font_size = run.run().font_size();
    let size = Size::new(font_size);
    let normalized_coords = run
      .run()
      .normalized_coords()
      .iter()
      .copied()
      .map(F2Dot14::from_bits)
      .collect::<Vec<_>>();
    let location = LocationRef::new(&normalized_coords);
    let has_emoji_cluster = run
      .run()
      .visual_clusters()
      .any(|cluster| cluster.is_emoji());
    let embolden = if !has_emoji_cluster
      && run.run().synthesis().embolden()
      && run.style().brush.font_synthesis.weight.is_allowed()
    {
      Some(synthesis_embolden_strength(font_size))
    } else {
      None
    };
    let skew = run
      .run()
      .synthesis()
      .skew()
      .filter(|_| !has_emoji_cluster)
      .filter(|_| run.style().brush.font_synthesis.style.is_allowed())
      .map(|degrees| ZenoTransform::skew(ZenoAngle::from_degrees(-degrees), ZenoAngle::ZERO));

    // Process each unique glyph ID
    for &glyph_id in &unique_glyph_ids {
      let skrifa_glyph_id = GlyphId::new(glyph_id);
      let resolved = if let Some(bitmap) = font_ref
        .bitmap_strikes()
        .glyph_for_size(size, skrifa_glyph_id)
      {
        scale_bitmap_glyph(bitmap, font_size).map(ResolvedGlyph::Bitmap)
      } else if let Some(color_outline) =
        resolve_color_outline_glyph(&font_ref, skrifa_glyph_id, size, location)
      {
        Some(ResolvedGlyph::Outline(color_outline))
      } else {
        let Some(mut paths) = resolve_outline_commands(&font_ref, skrifa_glyph_id, size, location)
        else {
          continue;
        };
        if let Some(skew_transform) = &skew {
          transform_commands(&mut paths, skew_transform);
        }

        Some(ResolvedGlyph::Outline(ResolvedOutlineGlyph {
          paths,
          color_layers: None,
          embolden,
        }))
      };

      if let Some(glyph) = resolved {
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
    let mut layout_context = LayoutContext::new();

    let mut builder = layout_context.tree_builder(&mut font_context, 1.0, true, &root_style);

    func(&mut builder);

    builder.build()
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
