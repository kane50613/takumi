//! Draws an SVG's flattened ops onto pixels and runs its filters.

mod drawing;
mod filter;

pub use drawing::SvgPixmapRasterizer;
use takumi_core::resources::image::{ImageError, parse_svg_filter};

/// Applies the SVG `<filter>` with `id="{filter_id}"` in `markup` to a premultiplied RGBA `layer`
/// in place. An invalid filter reference hides the element.
pub(crate) fn apply_svg_filter(
  layer: &mut [u8],
  width: u32,
  height: u32,
  markup: &str,
  filter_id: &str,
) -> Result<(), ImageError> {
  let Some(filter) = parse_svg_filter(markup, filter_id, width, height)? else {
    layer.fill(0);
    return Ok(());
  };

  filter::apply_to_layer(&filter, layer, width, height).ok_or(ImageError::InvalidPixmapSize)
}
