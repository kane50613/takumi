//! Image node → SVG `<image>` emission.
//!
//! Raster sources are embedded as `data:` URLs (original encoded bytes when
//! available, otherwise re-encoded PNG); SVG sources embed their original markup
//! as `data:image/svg+xml`, drawn where takumi-core's replaced-content placement puts them.

use std::io;

use takumi_core::{
  context::RenderContext,
  geometry::Size,
  layout::node::{ImageData, ImageSourceInput, resolve_image},
  painter::{BoxFrame, BoxPainter, PaintDevice},
  resources::image::{ImageSource, to_data_url},
  style::ImageScalingAlgorithm,
};

use crate::{Frame, SvgDocument, render::DocumentDevice};

pub(crate) const PRESERVE_ASPECT_NONE: &str = "none";

/// Resolves a `background-image: url(...)` reference to a `data:` URL, or `None`
/// if it cannot be resolved (usually no resource map was supplied).
pub(crate) fn data_url_for_url(url: &str, context: &RenderContext) -> Option<String> {
  resolve_image(url, context)
    .ok()
    .and_then(|s| loaded_data_url(&s, context))
}

/// Emits an image node's content into the content box of the box `painter` paints at `frame`,
/// clipped to the content box's curve or edge where it has to be.
pub(crate) fn emit_image(
  image: &ImageData,
  painter: &BoxPainter,
  frame: BoxFrame,
  doc: &mut SvgDocument,
) -> io::Result<()> {
  let context = painter.context();
  let content = Frame::content_box(frame);

  if content.w <= 0.0 || content.h <= 0.0 {
    return Ok(());
  }
  let Some(href) = data_url(&image.src, context) else {
    return Ok(());
  };
  let intrinsic = intrinsic_size(&image.src, context);
  let replaced = painter.replaced_content(intrinsic.map_or(
    Size {
      width: content.w,
      height: content.h,
    },
    |(width, height)| Size { width, height },
  ));
  // Without an intrinsic size the image keeps its own ratio inside the box.
  let (rect, aspect) = match intrinsic {
    Some(_) => (
      Frame::new(
        content.x + replaced.placement.offset.x,
        content.y + replaced.placement.offset.y,
        replaced.placement.size.width,
        replaced.placement.size.height,
      ),
      PRESERVE_ASPECT_NONE,
    ),
    None => (content, "xMidYMid meet"),
  };

  DocumentDevice::paint(doc, |device| {
    if let Some(clip) = replaced.clip {
      device.push_clip(&clip.into(), frame.translation());
    }
    device.write(|doc| doc.image(rect, &href, Some(aspect)));
    if replaced.clip.is_some() {
      device.pop_clip();
    }
  })
}

fn intrinsic_size(src: &ImageSourceInput, context: &RenderContext) -> Option<(f32, f32)> {
  let (width, height) = src.resolve(context).ok()?.size(&context.sizing);
  (width > 0.0 && height > 0.0).then_some((width, height))
}

fn data_url(src: &ImageSourceInput, context: &RenderContext) -> Option<String> {
  match src {
    // Embed the original encoded bytes losslessly. SVG markup resolves into a
    // parsed source first so the host `color` can reach `currentColor`.
    ImageSourceInput::Buffer(bytes) => match sniff_mime(bytes) {
      "image/svg+xml" => src
        .resolve(context)
        .ok()
        .and_then(|s| loaded_data_url(&s, context)),
      mime => Some(to_data_url(mime, bytes)),
    },
    ImageSourceInput::Loaded(source) => loaded_data_url(source, context),
    // Only resolvable when the render supplied a resource map (usually empty).
    ImageSourceInput::Url(_) => src
      .resolve(context)
      .ok()
      .and_then(|s| loaded_data_url(&s, context)),
    _ => None,
  }
}

fn loaded_data_url(source: &ImageSource, context: &RenderContext) -> Option<String> {
  match source {
    ImageSource::Bitmap(buffer) => buffer
      .encode_png()
      .map(|png| to_data_url("image/png", &png)),
    ImageSource::Encoded(encoded) => {
      Some(to_data_url(sniff_mime(encoded.bytes()), encoded.bytes()))
    }
    ImageSource::Animated(animated) => {
      let (width, height) = animated.dimensions();
      animated
        .frame_at_time_covering(0, width, height, ImageScalingAlgorithm::Auto)
        .encode_png()
        .map(|png| to_data_url("image/png", &png))
    }
    ImageSource::Svg(svg) => Some(to_data_url(
      "image/svg+xml",
      svg
        .source_with_current_color(context.current_color)
        .as_bytes(),
    )),
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
