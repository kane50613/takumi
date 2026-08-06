use skrifa::raw::TableProvider;

use crate::krilla::color::rgb;
use crate::krilla::surface::Surface;
use crate::krilla::text::Font;
use crate::krilla::text::GlyphId;

pub(crate) fn has_svg_data(font: &Font, glyph: GlyphId) -> bool {
  font
    .font_ref()
    .svg()
    .map(|svg_table| svg_table.glyph_data(glyph.to_skrifa()).is_ok())
    .is_ok()
}

/// Draw an SVG-based glyph on a surface.
pub(crate) fn draw_glyph(
  font: Font,
  context_color: rgb::Color,
  glyph: GlyphId,
  surface: &mut Surface,
) -> Option<()> {
  let svg_data = font
    .font_ref()
    .svg()
    .and_then(|svg_table| svg_table.glyph_data(glyph.to_skrifa()))
    .ok()??;

  let upem = font.units_per_em();

  let fn_ = surface.sc.serialize_settings().render_svg_glyph_fn;
  fn_(svg_data, context_color, glyph, (upem, upem), surface)?;

  Some(())
}
