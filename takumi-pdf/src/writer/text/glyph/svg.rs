use skrifa::raw::TableProvider;

use crate::writer::color::rgb;
use crate::writer::surface::Surface;
use crate::writer::text::Font;
use crate::writer::text::GlyphId;

pub(crate) fn has_svg_data(font: &Font, glyph: GlyphId) -> bool {
  font
    .font_ref()
    .svg()
    .is_ok_and(|svg_table| svg_table.glyph_data(glyph.to_skrifa()).is_some())
}

/// Draw an SVG-based glyph on a surface.
pub(crate) fn draw_glyph(
  font: Font,
  context_color: rgb::Color,
  glyph: GlyphId,
  surface: &mut Surface,
) -> Option<()> {
  let svg_data = font.font_ref().svg().ok()?.glyph_data(glyph.to_skrifa())?;

  let upem = font.units_per_em();

  let old_fill = surface.get_fill().cloned();
  let old_stroke = surface.get_stroke().cloned();

  let fn_ = surface.sc.serialize_settings().render_svg_glyph_fn;
  let result = fn_(svg_data, context_color, glyph, (upem, upem), surface);

  surface.set_fill(old_fill);
  surface.set_stroke(old_stroke);

  result
}
