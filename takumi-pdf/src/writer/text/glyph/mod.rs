use std::ops::Range;

use skrifa::MetadataProvider;

use crate::writer::color::rgb;
use crate::writer::geom::Transform;
use crate::writer::surface::Surface;
use crate::writer::text::Font;

#[cfg(feature = "images")]
pub(crate) mod bitmap;
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
  let drawn = crate::color_glyph::draw(
    font.font_ref(),
    font.location_ref(),
    glyph.to_skrifa(),
    context_color,
    surface,
  )
  .or_else(|| svg::draw_glyph(font.clone(), context_color, glyph, surface))
  .or_else(|| {
    #[cfg(feature = "images")]
    let res = bitmap::draw_glyph(font.clone(), glyph, surface);

    #[cfg(not(feature = "images"))]
    let res = None;

    res
  });

  surface.pop();
  surface.pop();

  drawn
}

pub(crate) fn should_outline(font: &Font, glyph: GlyphId) -> bool {
  let has_svg = svg::has_svg_data(font, glyph);
  let has_colr = font
    .font_ref()
    .color_glyphs()
    .get(glyph.to_skrifa())
    .is_some();
  #[cfg(feature = "images")]
  let has_bitmap = bitmap::has_bitmap_data(font, glyph);
  #[cfg(not(feature = "images"))]
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
  /// A location identifying the glyph. If set, the writer will automatically call
  /// `set_location` before processing the glyph.
  fn location(&self) -> Option<crate::writer::surface::Location>;
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
