//! Image node → SVG `<image>` emission.
//!
//! Raster sources are embedded as `data:` URLs (original encoded bytes when
//! available, otherwise re-encoded PNG); SVG sources embed their original markup
//! as `data:image/svg+xml`. CSS `object-fit` maps onto SVG `preserveAspectRatio`.

use std::io;

use takumi_core::{
  context::RenderContext,
  geometry::Size,
  layout::{
    node::{ImageData, ImageSourceInput, resolve_image},
    replaced::place_replaced,
  },
  resources::image::{ImageSource, to_data_url},
  style::{ImageScalingAlgorithm, ObjectFit},
};

use crate::{Frame, SvgDocument, box_model::rect_path_data};

pub(crate) const PRESERVE_ASPECT_NONE: &str = "none";

/// Resolves a `background-image: url(...)` reference to a `data:` URL, or `None`
/// if it cannot be resolved (usually no resource map was supplied).
pub(crate) fn data_url_for_url(url: &str, context: &RenderContext) -> Option<String> {
  resolve_image(url, context)
    .ok()
    .and_then(|s| loaded_data_url(&s))
}

/// Emits an image node's content into the given content-box rectangle.
pub(crate) fn emit_image(
  image: &ImageData,
  context: &RenderContext,
  content: Frame,
  doc: &mut SvgDocument,
) -> io::Result<()> {
  let Frame { x, y, w, h } = content;
  if w <= 0.0 || h <= 0.0 {
    return Ok(());
  }
  let Some(href) = data_url(&image.src, context) else {
    return Ok(());
  };
  if matches!(context.style.object_fit, ObjectFit::Fill) {
    return doc.image(x, y, w, h, &href, Some("none"));
  }

  let Some((iw, ih)) = intrinsic_size(&image.src, context) else {
    return doc.image(x, y, w, h, &href, Some("xMidYMid meet"));
  };
  let content = Size {
    width: w,
    height: h,
  };
  let placement = place_replaced(
    context,
    content,
    Size {
      width: iw,
      height: ih,
    },
  );
  let (dw, dh) = (placement.size.width, placement.size.height);
  let (ix, iy) = (x + placement.offset.x, y + placement.offset.y);

  if placement.overflows(content) {
    let clip = doc.clip_path(&rect_path_data(x, y, w, h))?;
    let group = doc.begin_group(crate::IDENTITY, 1.0, Some(&clip), None)?;
    doc.image(ix, iy, dw, dh, &href, Some(PRESERVE_ASPECT_NONE))?;
    return doc.end_group(group);
  }
  doc.image(ix, iy, dw, dh, &href, Some(PRESERVE_ASPECT_NONE))
}

fn intrinsic_size(src: &ImageSourceInput, context: &RenderContext) -> Option<(f32, f32)> {
  let (width, height) = src.resolve(context).ok()?.size(&context.sizing);
  (width > 0.0 && height > 0.0).then_some((width, height))
}

fn data_url(src: &ImageSourceInput, context: &RenderContext) -> Option<String> {
  match src {
    // Embed the original encoded bytes losslessly.
    ImageSourceInput::Buffer(bytes) => Some(to_data_url(sniff_mime(bytes), bytes)),
    ImageSourceInput::Loaded(source) => loaded_data_url(source),
    // Only resolvable when the render supplied a resource map (usually empty).
    ImageSourceInput::Url(_) => src.resolve(context).ok().and_then(|s| loaded_data_url(&s)),
    _ => None,
  }
}

fn loaded_data_url(source: &ImageSource) -> Option<String> {
  match source {
    ImageSource::Bitmap(buffer) => buffer
      .encode_png()
      .map(|png| to_data_url("image/png", &png)),
    ImageSource::Encoded(encoded) => {
      Some(to_data_url(sniff_mime(encoded.bytes()), encoded.bytes()))
    }
    ImageSource::Gif(gif) => {
      let (width, height) = gif.dimensions();
      gif
        .frame_at_time_covering(0, width, height, ImageScalingAlgorithm::Auto)
        .encode_png()
        .map(|png| to_data_url("image/png", &png))
    }
    ImageSource::Svg(svg) => Some(to_data_url("image/svg+xml", svg.source().as_bytes())),
    _ => None,
  }
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
    assert_eq!(
      to_data_url("image/png", b"AB"),
      "data:image/png;base64,QUI="
    );
  }
}
