//! Image node and `background-image: url(...)` → SVG emission.
//!
//! Raster sources are embedded as `data:` URLs (original encoded bytes when
//! available, otherwise re-encoded PNG). SVG sources are written as their vector ops, or embed
//! their original markup as `data:image/svg+xml` where an op needs a raster. An image node draws
//! where takumi-core's replaced-content placement puts it. An image drawn again from the same
//! `src` reuses its encoding and, when stretched to its box, one `<defs>` entry.

use std::{io, rc::Rc, sync::Arc};

use takumi_core::{
  context::RenderContext,
  geometry::Size,
  layout::node::{ImageData, ImageSourceInput, resolve_image},
  painter::{BoxFrame, BoxPainter, PaintDevice},
  resources::image::{ImageSource, SvgOp, SvgSource, sniff_mime, to_data_url},
  style::{Affine, FillRule, ImageScalingAlgorithm},
};

use crate::{Frame, ImageKey, SvgDocument, render::DocumentDevice, svg_image::is_vector};

pub(crate) const PRESERVE_ASPECT_NONE: &str = "none";

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
  let vector = match (&source, intrinsic) {
    (Some(ImageSource::Svg(svg)), Some(_)) => vector_ops(svg, context, replaced.container),
    _ => None,
  };
  let Some(drawing) = vector.or_else(|| raster(&image.src, source.as_ref(), context, doc)) else {
    return Ok(());
  };

  DocumentDevice::paint(doc, |device| {
    if let Some(clip) = replaced.clip {
      device.push_clip(&clip.into(), frame.translation());
    }
    device.write(|doc| doc.draw(&drawing, rect, aspect));
    if replaced.clip.is_some() {
      device.pop_clip();
    }
  })
}

fn intrinsic_size(source: &ImageSource, context: &RenderContext) -> Option<(f32, f32)> {
  let (width, height) = source.size(&context.sizing);
  (width > 0.0 && height > 0.0).then_some((width, height))
}

/// What an image draws as in SVG output.
pub(crate) enum Drawing {
  /// An SVG image's vector ops, drawn on `canvas`.
  Ops { ops: Vec<SvgOp>, canvas: Frame },
  /// A `data:` URL for an `<image>`, with the key and pixel size it can be defined once under.
  Raster {
    href: Rc<str>,
    definition: Option<(ImageKey, (f32, f32))>,
  },
}

impl Drawing {
  /// What a `background-image: url(...)` draws in a `tile`, rendered at `image_size`, or `None`
  /// if it cannot be resolved (usually no resource map was supplied) or, as in raster output,
  /// drawn at that size.
  pub(crate) fn of_url(
    url: &Arc<str>,
    tile: Frame,
    image_size: Size<f32>,
    context: &RenderContext,
    doc: &mut SvgDocument,
  ) -> Option<Self> {
    let source = resolve_image(url, context).ok()?;

    if !source.is_drawable_at(tile.w.round() as u32, tile.h.round() as u32) {
      return None;
    }

    let vector = match &source {
      ImageSource::Svg(svg) => vector_ops(svg, context, image_size),
      _ => None,
    };

    vector.or_else(|| Self::raster_of(url, &source, context, doc))
  }

  /// `source`, resolved from `src`, as a `data:` URL encoded once per document.
  fn raster_of(
    src: &Arc<str>,
    source: &ImageSource,
    context: &RenderContext,
    doc: &mut SvgDocument,
  ) -> Option<Self> {
    let key = ImageKey::Src {
      src: Arc::clone(src),
      current_color: context.current_color.0,
    };
    let href = doc.image_href(&key, || loaded_data_url(source, context))?;

    Some(Self::Raster {
      href,
      definition: pixel_size(source).map(|size| (key, size)),
    })
  }
}

/// The image `src` names, keyed by its `src` when it has one so a repeat reuses its encoding.
fn raster(
  src: &ImageSourceInput,
  source: Option<&ImageSource>,
  context: &RenderContext,
  doc: &mut SvgDocument,
) -> Option<Drawing> {
  if let (ImageSourceInput::Url(url), Some(source)) = (src, source) {
    return Drawing::raster_of(url, source, context, doc);
  }

  data_url(src, context).map(|href| Drawing::Raster {
    href: href.into(),
    definition: None,
  })
}

/// The pixel size `source` is encoded at, which its `<defs>` entry takes.
fn pixel_size(source: &ImageSource) -> Option<(f32, f32)> {
  let (width, height) = match source {
    ImageSource::Bitmap(buffer) => (buffer.width() as f32, buffer.height() as f32),
    ImageSource::Encoded(encoded) => {
      let (width, height) = encoded.dimensions();

      (width as f32, height as f32)
    }
    ImageSource::Animated(animated) => {
      let (width, height) = animated.dimensions();

      (width as f32, height as f32)
    }
    ImageSource::Svg(svg) => svg.dimensions(),
    _ => return None,
  };

  (width > 0.0 && height > 0.0).then_some((width, height))
}

/// `svg`'s ops laid out at `viewport`, Blink's container size, when SVG elements can express every
/// op.
fn vector_ops(svg: &SvgSource, context: &RenderContext, viewport: Size<f32>) -> Option<Drawing> {
  let ops = svg.vector_ops(context, viewport);

  (viewport.width > 0.0 && viewport.height > 0.0 && is_vector(&ops)).then(|| Drawing::Ops {
    ops,
    canvas: Frame::new(0.0, 0.0, viewport.width, viewport.height),
  })
}

impl SvgDocument {
  /// Draws `drawing` stretched over `rect`, an `<image>` keeping `aspect`.
  pub(crate) fn draw(&mut self, drawing: &Drawing, rect: Frame, aspect: &str) -> io::Result<()> {
    match drawing {
      Drawing::Ops { ops, canvas } => self.svg_image(ops, *canvas, rect),
      Drawing::Raster {
        href,
        definition: Some((key, natural)),
      } if aspect == PRESERVE_ASPECT_NONE => self.image_use(key, href, *natural, rect),
      Drawing::Raster { href, .. } => self.image(rect, href, Some(aspect)),
    }
  }

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
    let isolated = self.begin_isolate_group()?;

    self.svg_ops(ops)?;
    self.end_group(isolated)?;
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
