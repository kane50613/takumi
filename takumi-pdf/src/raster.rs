//! Encoded raster bytes go into the PDF as they came in, decoded by the
//! vendored writer or, for a JPEG, not decoded at all.

use crate::writer::image::Image as PdfImage;

pub(crate) fn embedded_image(bytes: &[u8]) -> Option<PdfImage> {
  let embed = match bytes {
    [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, ..] => PdfImage::from_png,
    [0xFF, 0xD8, 0xFF, ..] => PdfImage::from_jpeg,
    _ if is_webp(bytes) => PdfImage::from_webp,
    _ => return None,
  };

  embed(bytes.to_vec().into(), false).ok()
}

fn is_webp(bytes: &[u8]) -> bool {
  bytes.len() >= 12 && bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WEBP"
}
