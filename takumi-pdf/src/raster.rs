//! Encoded raster bytes go into the PDF as they came in. The decoders live in
//! the vendored krilla, and a JPEG is embedded without being decoded at all.

use crate::krilla::image::Image as KrillaImage;

pub(crate) fn embedded_image(bytes: &[u8]) -> Option<KrillaImage> {
  let data = bytes.to_vec().into();
  let image = match bytes {
    [0x89, b'P', b'N', b'G', ..] => KrillaImage::from_png(data, false),
    [0xFF, 0xD8, 0xFF, ..] => KrillaImage::from_jpeg(data, false),
    _ if is_webp(bytes) => KrillaImage::from_webp(data, false),
    _ => return None,
  };

  image.ok()
}

fn is_webp(bytes: &[u8]) -> bool {
  bytes.len() >= 12 && bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WEBP"
}
