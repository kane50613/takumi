//! Applies `filter: url(...)` SVG filters by delegating to the resvg pipeline:
//! the layer is wrapped in an SVG document that applies the referenced
//! `<filter>` to it, then rasterized through the core SVG image path.

use takumi_core::{
  Error, Result,
  resources::{
    image::{ImageError, ImageSource, RenderedImage},
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
    r#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" width="{width}" height="{height}">{markup}<image width="{width}" height="{height}" href="data:image/png;base64,{png}" filter="url(#{id})"/></svg>"#,
    markup = reference.markup,
    id = FilterReference::ID,
    png = base64(&png),
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

fn base64(bytes: &[u8]) -> String {
  const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
  let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
  for chunk in bytes.chunks(3) {
    let b = [
      chunk[0],
      *chunk.get(1).unwrap_or(&0),
      *chunk.get(2).unwrap_or(&0),
    ];
    let n = u32::from_be_bytes([0, b[0], b[1], b[2]]);
    let mut encoded = [
      ALPHABET[(n >> 18) as usize & 63],
      ALPHABET[(n >> 12) as usize & 63],
      ALPHABET[(n >> 6) as usize & 63],
      ALPHABET[n as usize & 63],
    ];
    if chunk.len() < 3 {
      encoded[3] = b'=';
    }
    if chunk.len() < 2 {
      encoded[2] = b'=';
    }
    out.push_str(std::str::from_utf8(&encoded).unwrap_or_default());
  }
  out
}

#[cfg(test)]
mod tests {
  use super::base64;

  #[test]
  fn base64_matches_rfc4648_vectors() {
    assert_eq!(base64(b""), "");
    assert_eq!(base64(b"f"), "Zg==");
    assert_eq!(base64(b"fo"), "Zm8=");
    assert_eq!(base64(b"foo"), "Zm9v");
    assert_eq!(base64(b"foob"), "Zm9vYg==");
    assert_eq!(base64(b"fooba"), "Zm9vYmE=");
    assert_eq!(base64(b"foobar"), "Zm9vYmFy");
  }
}
