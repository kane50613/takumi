use std::ops::Range;

use crate::krilla::color::rgb;
use crate::krilla::geom::Transform;
use crate::krilla::surface::Surface;
use crate::krilla::text::Font;

#[cfg(feature = "raster-images")]
pub(crate) mod bitmap;
pub(crate) mod colr;
pub(crate) mod outline;
pub(crate) mod svg;

/// A glyph ID.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, PartialOrd, Ord)]
pub struct GlyphId(u32);

impl GlyphId {
  /// Create a new glyph ID.
  pub fn new(id: u32) -> Self {
    Self(id)
  }

  /// Get the glyph ID as a u32.
  pub fn to_u32(&self) -> u32 {
    self.0
  }

  pub(crate) fn to_skrifa(self) -> skrifa::GlyphId {
    skrifa::GlyphId::new(self.0)
  }
}

/// Draw a color glyph to a surface.
pub(crate) fn draw_color_glyph(
  font: Font,
  context_color: rgb::Color,
  glyph: GlyphId,
  base_transform: Transform,
  surface: &mut Surface,
) -> Option<()> {
  surface.push_transform(&base_transform);
  surface.push_transform(&Transform::from_scale(1.0, -1.0));

  // Note: See the comment in krilla-svg in `render_svg_glyph` for why we prefer
  // COLR over SVG. For Noto Color Emoji specifically, using SVG will be slow when
  // rendering multiple glyphs.
  let drawn = colr::draw_glyph(font.clone(), context_color, glyph, surface)
    .or_else(|| svg::draw_glyph(font.clone(), context_color, glyph, surface))
    .or_else(|| {
      #[cfg(feature = "raster-images")]
      let res = bitmap::draw_glyph(font.clone(), glyph, surface);

      #[cfg(not(feature = "raster-images"))]
      let res = None;

      res
    });

  surface.pop();
  surface.pop();

  drawn
}

pub(crate) fn should_outline(font: &Font, glyph: GlyphId) -> bool {
  let has_svg = svg::has_svg_data(font, glyph);
  let has_colr = colr::has_colr_data(font, glyph);
  #[cfg(feature = "raster-images")]
  let has_bitmap = bitmap::has_bitmap_data(font, glyph);
  #[cfg(not(feature = "raster-images"))]
  let has_bitmap = false;
  font.font_info().can_be_cid_font() && !has_svg && !has_colr && !has_bitmap
}

/// Draw a color glyph or outline glyph to a surface.
pub(crate) fn draw_glyph(
  font: Font,
  context_color: rgb::Color,
  glyph: GlyphId,
  base_transform: Transform,
  surface: &mut Surface,
) -> Option<()> {
  draw_color_glyph(font.clone(), context_color, glyph, base_transform, surface)
    .or_else(|| outline::draw_glyph(font, glyph, base_transform, surface))
}

/// A glyph with certain properties.
pub trait Glyph {
  /// The glyph ID of the glyph.
  fn glyph_id(&self) -> GlyphId;
  /// The range of bytes in the original text covered by the cluster that the glyph
  /// belongs to.
  fn text_range(&self) -> Range<usize>;
  /// The advance in the x direction of the glyph, at the given font size.
  fn x_advance(&self, size: f32) -> f32;
  /// The offset in the x direction of the glyph, at the given font size.
  fn x_offset(&self, size: f32) -> f32;
  /// The offset in the y direction of the glyph, at the given font size.
  fn y_offset(&self, size: f32) -> f32;
  /// The advance in the y direction of the glyph, at the given font size.
  fn y_advance(&self, size: f32) -> f32;
  /// A location identifying the glyph. If set, `krilla` will automatically call
  /// `set_location` before processing the glyph.
  fn location(&self) -> Option<crate::krilla::surface::Location>;
}

/// A glyph type that implements `Glyph`.
///
/// You can use it if you don't  have your own type of glyph that you want to use.
#[derive(Debug, Clone)]
pub struct KrillaGlyph {
  /// The glyph ID of the glyph.
  pub glyph_id: GlyphId,
  /// The range in the original text that corresponds to the
  /// cluster of the glyph.
  pub text_range: Range<usize>,
  /// The advance of the glyph.
  pub x_advance: f32,
  /// The x offset of the glyph.
  pub x_offset: f32,
  /// The y offset of the glyph.
  pub y_offset: f32,
  /// The y advance of the glyph.
  pub y_advance: f32,
  /// The location of the glyph.
  pub location: Option<crate::krilla::surface::Location>,
}

impl Glyph for KrillaGlyph {
  fn glyph_id(&self) -> GlyphId {
    self.glyph_id
  }

  fn text_range(&self) -> Range<usize> {
    self.text_range.clone()
  }

  fn x_advance(&self, size: f32) -> f32 {
    self.x_advance * size
  }

  fn x_offset(&self, size: f32) -> f32 {
    self.x_offset * size
  }

  fn y_offset(&self, size: f32) -> f32 {
    self.y_offset * size
  }

  fn y_advance(&self, size: f32) -> f32 {
    self.y_advance * size
  }

  fn location(&self) -> Option<crate::krilla::surface::Location> {
    self.location
  }
}

impl KrillaGlyph {
  /// Create a new krilla glyph.
  ///
  /// Important: `x_advance`, `x_offset`, `y_offset` and `y_advance`
  /// need to be normalized, i.e. divided by the units per em!
  pub fn new(
    glyph_id: GlyphId,
    x_advance: f32,
    x_offset: f32,
    y_offset: f32,
    y_advance: f32,
    range: Range<usize>,
    location: Option<crate::krilla::surface::Location>,
  ) -> Self {
    Self {
      glyph_id,
      x_advance,
      x_offset,
      y_offset,
      y_advance,
      text_range: range,
      location,
    }
  }
}

/// A glyph that belongs either to a CID font or a Type 3 font.
#[derive(Clone, Copy)]
pub(crate) enum PDFGlyph {
  Type3(u8),
  Cid(u16),
}

impl PDFGlyph {
  /// Encode the glyph into a content stream.
  #[inline(always)]
  pub(crate) fn encode_into(&self, slice: &mut Vec<u8>) {
    match self {
      PDFGlyph::Type3(cg) => slice.push(*cg),
      PDFGlyph::Cid(cid) => {
        slice.push((cid >> 8) as u8);
        slice.push((cid & 0xff) as u8);
      }
    }
  }
}
