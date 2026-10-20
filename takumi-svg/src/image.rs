//! Image node → SVG `<image>` emission.
//!
//! Raster sources are embedded as `data:` URLs (original encoded bytes when
//! available, otherwise re-encoded PNG). SVG sources are written as their vector ops, or embed
//! their original markup as `data:image/svg+xml` where an op needs a raster. Both are drawn where
//! takumi-core's replaced-content placement puts them.

use std::io;

use takumi_core::{
  context::RenderContext,
  geometry::Size,
  layout::node::{ImageData, ImageSourceInput, resolve_image},
  painter::{BoxFrame, BoxPainter, PaintDevice},
  resources::image::{ImageSource, SvgOp, SvgSource, sniff_mime, to_data_url},
  style::{Affine, FillRule, ImageScalingAlgorithm},
};

use crate::{Frame, SvgDocument, render::DocumentDevice, svg_image::is_vector};

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
  let source = image.src.resolve(context).ok();
  let intrinsic = source
    .as_ref()
    .and_then(|source| intrinsic_size(source, context));
  let vector = match (&source, intrinsic) {
    (Some(ImageSource::Svg(svg)), Some(_)) => vector_ops(svg, context),
    _ => None,
  };
  let Some(drawing) = vector.or_else(|| data_url(&image.src, context).map(Drawing::Href)) else {
    return Ok(());
  };
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
    device.write(|doc| match &drawing {
      Drawing::Ops { ops, canvas } => doc.svg_image(ops, *canvas, rect),
      Drawing::Href(href) => doc.image(rect, href, Some(aspect)),
    });
    if replaced.clip.is_some() {
      device.pop_clip();
    }
  })
}

fn intrinsic_size(source: &ImageSource, context: &RenderContext) -> Option<(f32, f32)> {
  let (width, height) = source.size(&context.sizing);
  (width > 0.0 && height > 0.0).then_some((width, height))
}

/// What an image node draws.
enum Drawing {
  /// An SVG image's vector ops, drawn on `canvas`.
  Ops { ops: Vec<SvgOp>, canvas: Frame },
  /// A `data:` URL for an `<image>`.
  Href(String),
}

/// `svg`'s ops, when SVG elements can express every op.
fn vector_ops(svg: &SvgSource, context: &RenderContext) -> Option<Drawing> {
  let (width, height) = svg.dimensions();
  // Ops that need a raster discard it for the original markup, so its scale does not matter.
  let ops = svg.vector_ops(1.0, context.current_color, Some(context.fonts()));

  (width > 0.0 && height > 0.0 && is_vector(&ops)).then(|| Drawing::Ops {
    ops,
    canvas: Frame::new(0.0, 0.0, width, height),
  })
}

impl SvgDocument {
  /// Writes an SVG image's `ops`, drawn on `canvas`, stretched over `rect`.
  fn svg_image(&mut self, ops: &[SvgOp], canvas: Frame, rect: Frame) -> io::Result<()> {
    let clip = self.clip_path(&canvas.path_data(), FillRule::NonZero, None)?;
    let transform = Affine {
      a: rect.w / canvas.w,
      b: 0.0,
      c: 0.0,
      d: rect.h / canvas.h,
      x: rect.x,
      y: rect.y,
    };
    let group = self.begin_group(transform, 1.0, Some(&clip), None)?;

    self.svg_ops(ops)?;
    self.end_group(group)
  }
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
