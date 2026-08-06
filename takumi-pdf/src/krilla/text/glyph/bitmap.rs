use skrifa::MetadataProvider;
use skrifa::bitmap::{BitmapData, BitmapFormat, BitmapGlyph, Origin};

use crate::krilla::geom::{Size, Transform};
use crate::krilla::graphics::image::Image;
use crate::krilla::surface::Surface;
use crate::krilla::text::Font;
use crate::krilla::text::GlyphId;

pub(crate) fn has_bitmap_data(font: &Font, glyph: GlyphId) -> bool {
  // We only support PNG.
  get_bitmap_glyph(font, glyph).is_some_and(|b| matches!(b.data, BitmapData::Png(_)))
}

pub(crate) fn get_bitmap_glyph(font: &Font, glyph: GlyphId) -> Option<BitmapGlyph<'_>> {
  let bitmap_strikes = font.font_ref().bitmap_strikes();
  bitmap_strikes.glyph_for_size(skrifa::instance::Size::unscaled(), glyph.to_skrifa())
}

/// Draw a bitmap-based glyph on a surface.
pub(crate) fn draw_glyph(font: Font, glyph: GlyphId, surface: &mut Surface) -> Option<()> {
  let metrics = font
    .font_ref()
    .metrics(skrifa::instance::Size::unscaled(), font.location_ref());

  let bitmap_strikes = font.font_ref().bitmap_strikes();
  let bitmap_glyph =
    bitmap_strikes.glyph_for_size(skrifa::instance::Size::unscaled(), glyph.to_skrifa())?;

  let upem = metrics.units_per_em as f32;

  match bitmap_glyph.data {
    BitmapData::Png(data) => {
      let image = Image::from_png(data.to_vec().into(), false).ok()?;
      let size = Size::from_wh(image.size().0 as f32, image.size().1 as f32).unwrap();

      // Adapted from vello.
      let scale_factor = upem / (bitmap_glyph.ppem_y);
      let mut transform =
        Transform::from_translate(-bitmap_glyph.bearing_x, bitmap_glyph.bearing_y)
          .pre_concat(Transform::from_scale(scale_factor, scale_factor))
          .pre_concat(Transform::from_translate(
            -bitmap_glyph.inner_bearing_x,
            -bitmap_glyph.inner_bearing_y,
          ));

      transform = match bitmap_glyph.placement_origin {
        Origin::TopLeft => transform,
        Origin::BottomLeft => {
          transform.pre_concat(Transform::from_translate(0.0, -(image.size().1 as f32)))
        }
      };

      transform = if let Some(format) = bitmap_strikes.format() {
        if format == BitmapFormat::Sbix {
          // For unknown reasons, using Apple Color Emoji will lead to a vertical shift on MacOS, but this shift
          // doesn't seem to be coming from the font and most likely is somehow hardcoded. On Windows,
          // this shift will not be applied. However, if this shift is not applied the emojis are a bit
          // too high up when being together with other text, so we try to imitate this.
          // See also https://github.com/harfbuzz/harfbuzz/issues/2679#issuecomment-1345595425
          // We approximate this vertical shift that seems to be produced by it.
          // This value seems to be pretty close to what is happening on MacOS.
          transform.pre_concat(Transform::from_translate(0.0, 0.128 * upem / scale_factor))
        } else {
          transform
        }
      } else {
        transform
      };

      surface.push_transform(&transform);
      surface.draw_image(image, size);
      surface.pop();

      Some(())
    }
    BitmapData::Bgra(_) => None,
    BitmapData::Mask(_) => None,
  }
}
