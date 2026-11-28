use skrifa::outline::DrawSettings;
use takumi_core::{
  path::{Path, PathBuilder},
  resources::glyph::draw_outline,
  style::Affine,
};

use crate::writer::surface::Surface;
use crate::writer::text::Font;
use crate::writer::text::GlyphId;

pub(crate) fn glyph_path(font: Font, glyph: GlyphId) -> Option<Path> {
  let outline_glyphs = font.outline_glyphs();
  let mut path_builder = PathBuilder::new();

  if let Some(outline_glyph) = outline_glyphs.get(glyph.to_skrifa()) {
    draw_outline(
      &outline_glyph,
      DrawSettings::unhinted(skrifa::instance::Size::unscaled(), font.location_ref()),
      &mut path_builder,
    )
    .ok()?;
  }

  path_builder.finish()
}

/// Draw an outline-based glyph on a surface.
pub(crate) fn draw_glyph(
  font: Font,
  glyph: GlyphId,
  base_transform: Affine,
  surface: &mut Surface,
) -> Option<()> {
  let path = glyph_path(font, glyph).and_then(|p| p.transform(base_transform))?;

  surface.draw_path(&path);

  Some(())
}
