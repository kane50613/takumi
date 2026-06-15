//! Image node → SVG `<image>` emission.
//!
//! Raster sources are embedded as `data:` URLs (original encoded bytes when
//! available, otherwise re-encoded PNG); SVG sources embed their original markup
//! as `data:image/svg+xml`. CSS `object-fit` maps onto SVG `preserveAspectRatio`.

use std::io;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use takumi_core::context::RenderContext;
use takumi_core::layout::node::{ImageData, ImageSourceInput};
use takumi_core::layout::style::ObjectFit;
use takumi_core::resources::image::ImageSource;

use crate::SvgDocument;

/// Emits an image node's content into the given content-box rectangle.
pub(crate) fn emit_image(
  image: &ImageData,
  context: &RenderContext,
  x: f32,
  y: f32,
  w: f32,
  h: f32,
  doc: &mut SvgDocument,
) -> io::Result<()> {
  if w <= 0.0 || h <= 0.0 {
    return Ok(());
  }
  let Some(href) = data_url(&image.src, context) else {
    return Ok(());
  };
  let preserve_aspect_ratio = match context.style.object_fit {
    ObjectFit::Fill => "none",
    ObjectFit::Cover => "xMidYMid slice",
    // contain / scale-down / none all fit within the box without cropping.
    _ => "xMidYMid meet",
  };
  doc.image(x, y, w, h, &href, Some(preserve_aspect_ratio))
}

fn data_url(src: &ImageSourceInput, context: &RenderContext) -> Option<String> {
  match src {
    // Embed the original encoded bytes losslessly.
    ImageSourceInput::Buffer(bytes) => Some(encode(sniff_mime(bytes), bytes)),
    ImageSourceInput::Loaded(source) => loaded_data_url(source),
    // Only resolvable when the render supplied a resource map (usually empty).
    ImageSourceInput::Url(_) => src.resolve(context).ok().and_then(|s| loaded_data_url(&s)),
  }
}

fn loaded_data_url(source: &ImageSource) -> Option<String> {
  match source {
    ImageSource::Bitmap(buffer) => buffer.to_png().map(|png| encode("image/png", &png)),
    ImageSource::Gif(gif) => gif
      .frame_at_time(0)
      .to_png()
      .map(|png| encode("image/png", &png)),
    ImageSource::Svg(svg) => Some(encode("image/svg+xml", svg.source().as_bytes())),
  }
}

pub(crate) fn encode(mime: &str, bytes: &[u8]) -> String {
  format!("data:{mime};base64,{}", STANDARD.encode(bytes))
}

fn sniff_mime(bytes: &[u8]) -> &'static str {
  match bytes {
    [0x89, b'P', b'N', b'G', ..] => "image/png",
    [0xFF, 0xD8, 0xFF, ..] => "image/jpeg",
    [b'G', b'I', b'F', b'8', ..] => "image/gif",
    [
      b'R',
      b'I',
      b'F',
      b'F',
      _,
      _,
      _,
      _,
      b'W',
      b'E',
      b'B',
      b'P',
      ..,
    ] => "image/webp",
    _ => {
      let head = &bytes[..bytes.len().min(256)];
      if head.starts_with(b"<?xml") || head.windows(4).any(|w| w == b"<svg") {
        "image/svg+xml"
      } else {
        "application/octet-stream"
      }
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn sniffs_common_formats() {
    assert_eq!(
      sniff_mime(&[0x89, b'P', b'N', b'G', 0, 0, 0, 0]),
      "image/png"
    );
    assert_eq!(sniff_mime(&[0xFF, 0xD8, 0xFF, 0xE0]), "image/jpeg");
    assert_eq!(sniff_mime(b"GIF89a"), "image/gif");
    assert_eq!(sniff_mime(br#"<svg xmlns="...">"#), "image/svg+xml");
    assert_eq!(sniff_mime(b"\0\0"), "application/octet-stream");
  }

  #[test]
  fn encodes_data_url() {
    assert_eq!(encode("image/png", b"AB"), "data:image/png;base64,QUI=");
  }
}
