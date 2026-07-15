//! Applies `filter: url(...)` SVG filters by delegating to the resvg pipeline:
//! the layer is wrapped in an SVG document that applies the referenced
//! `<filter>` to it, then rasterized through the core SVG image path.

use takumi_core::{
  Error, Result,
  resources::{
    image::{ImageError, ImageSource, RenderedImage, to_data_url},
    image_buffer::ImageBuffer,
  },
  style::{FilterReference, ImageScalingAlgorithm},
};
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

  let layer = ImageBuffer::from_premultiplied_rgba(pixmap.data_mut().to_vec(), width, height)
    .ok_or(Error::ImageResolveError(ImageError::InvalidPixmapSize))?;
  let png = layer
    .encode_png()
    .ok_or(Error::ImageResolveError(ImageError::InvalidPixmapSize))?;

  let document = format!(
    r#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" width="{width}" height="{height}">{markup}<image width="{width}" height="{height}" href="{png}" filter="url(#{id})"/></svg>"#,
    markup = reference.markup,
    id = FilterReference::ID,
    png = to_data_url("image/png", &png),
  );

  let filtered = ImageSource::from_bytes(document.as_bytes())
    .map_err(Error::from)?
    .render_for_layout(width, height, ImageScalingAlgorithm::Auto, 0)?;

  let RenderedImage::Rasterized(buffer) = filtered else {
    return Ok(());
  };

  pixmap.data_mut().copy_from_slice(buffer.data());
  Ok(())
}
