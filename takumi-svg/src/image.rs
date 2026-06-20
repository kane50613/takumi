//! Image node → SVG `<image>` emission.
//!
//! Raster sources are embedded as `data:` URLs (original encoded bytes when
//! available, otherwise re-encoded PNG); SVG sources embed their original markup
//! as `data:image/svg+xml`. CSS `object-fit` maps onto SVG `preserveAspectRatio`.

use std::io;

use base64::{Engine, engine::general_purpose::STANDARD};
use takumi_base::{
  context::RenderContext,
  layout::{
    node::{ImageData, ImageSourceInput, resolve_image},
    style::{Length, ObjectFit, PositionComponent},
  },
  resources::image::ImageSource,
};

use crate::{SvgDocument, box_model::rect_path_data};

/// `preserveAspectRatio="none"` (stretch to fit), used when the destination rect
/// already carries the desired size.
pub(crate) fn preserve_aspect_none() -> &'static str {
  "none"
}

/// Resolves a `background-image: url(...)` reference to a `data:` URL, or `None`
/// if it cannot be resolved (usually no resource map was supplied).
pub(crate) fn data_url_for_url(url: &str, context: &RenderContext) -> Option<String> {
  resolve_image(url, context)
    .ok()
    .and_then(|s| loaded_data_url(&s))
}

/// Resolves one `object-position` axis to a destination offset within
/// `available` space, mirroring the raster backend's `resolve_object_position_axis`.
fn position_axis(component: PositionComponent, context: &RenderContext, available: f32) -> f32 {
  match Length::from(component) {
    Length::Auto => available * 0.5,
    length => length.to_px(&context.sizing, available),
  }
}

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
  let position = context.style.object_position;

  if matches!(context.style.object_fit, ObjectFit::Fill) {
    return doc.image(x, y, w, h, &href, Some("none"));
  }

  let Some((iw, ih)) = intrinsic_size(&image.src, context) else {
    return doc.image(x, y, w, h, &href, Some("xMidYMid meet"));
  };

  let scale = match context.style.object_fit {
    ObjectFit::Fill => 1.0,
    ObjectFit::Contain => (w / iw).min(h / ih),
    ObjectFit::Cover => (w / iw).max(h / ih),
    ObjectFit::None => 1.0,
    ObjectFit::ScaleDown => (w / iw).min(h / ih).min(1.0),
  };
  let (dw, dh) = (iw * scale, ih * scale);

  let off_x = position_axis(position.0.x, context, w - dw);
  let off_y = position_axis(position.0.y, context, h - dh);
  let (ix, iy) = (x + off_x, y + off_y);

  if dw > w + 0.5 || dh > h + 0.5 {
    let clip = doc.clip_path(&rect_path_data(x, y, w, h))?;
    let group = doc.begin_group(crate::IDENTITY, 1.0, Some(&clip), None)?;
    doc.image(ix, iy, dw, dh, &href, Some(preserve_aspect_none()))?;
    return doc.end_group(group);
  }
  doc.image(ix, iy, dw, dh, &href, Some(preserve_aspect_none()))
}

fn intrinsic_size(src: &ImageSourceInput, context: &RenderContext) -> Option<(f32, f32)> {
  let (width, height) = src.resolve(context).ok()?.size(&context.sizing);
  (width > 0.0 && height > 0.0).then_some((width, height))
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
    ImageSource::Bitmap(buffer) => buffer.encode_png().map(|png| encode("image/png", &png)),
    ImageSource::Gif(gif) => gif
      .frame_at_time(0)
      .encode_png()
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
