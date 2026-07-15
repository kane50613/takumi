//! Applies `filter: url(...)` SVG filters by delegating to the resvg pipeline
//! through [`apply_svg_filter`], which hands the layer over without a base64 /
//! data-URI roundtrip.

use takumi_core::{Error, Result, resources::image::apply_svg_filter, style::FilterReference};
use tiny_skia::PixmapMut;

pub(crate) fn apply_filter_reference(
  pixmap: &mut PixmapMut<'_>,
  reference: &FilterReference,
) -> Result<()> {
  let width = pixmap.width();
  let height = pixmap.height();
  if width == 0 || height == 0 {
    return Ok(());
  }

  apply_svg_filter(
    pixmap.data_mut(),
    width,
    height,
    &reference.markup,
    FilterReference::ID,
  )
  .map_err(Error::ImageResolveError)
}
