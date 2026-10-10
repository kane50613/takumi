use takumi_core::{
  geometry::{ComputedLayout as Layout, Point, Size},
  layout::{decoration::ClipBox, replaced::ReplacedPlacement},
  painter::{FillShape, PaintDevice},
};

use crate::{
  BorderProperties, Canvas, CanvasDevice, PaintSource, RenderContext, Result, SamplingOptions,
  painter::BoxPainter,
  pixmap_ref_from_buffer,
  resources::image::{ImageSource, RenderedImage},
  style::{Affine, BlendMode, ImageScalingAlgorithm},
};

struct PreparedImage {
  image: RenderedImage,
  logical_to_source: Affine,
  offset: Point<f32>,
}

/// Renders the part of an image at `placement`, relative to the content box, that lands inside
/// the `crop` rectangle at `crop_origin` from the content box.
fn process_image_for_object_fit(
  image: &ImageSource,
  context: &RenderContext,
  placement: ReplacedPlacement,
  crop_origin: Point<f32>,
  crop: Size<f32>,
) -> Result<PreparedImage> {
  let (image_width, image_height) = image.size(&context.sizing);
  let (source_width, source_height) = match image {
    ImageSource::Bitmap(bitmap) => (bitmap.width() as f32, bitmap.height() as f32),
    #[cfg(any(feature = "png", feature = "gif", feature = "webp"))]
    ImageSource::Animated(animated) => {
      let (width, height) = animated.dimensions();
      (width as f32, height as f32)
    }
    ImageSource::Encoded(encoded) => {
      let (width, height) = encoded.dimensions();
      (width as f32, height as f32)
    }
    #[cfg(feature = "svg")]
    ImageSource::Svg(svg) => svg.dimensions(),
    _ => (image_width, image_height),
  };
  let clipped = ReplacedPlacement {
    offset: placement.offset - crop_origin,
    size: placement.size,
  }
  .clipped(crop);
  let rendered = image.render_for_layout(
    clipped.size.width as u32,
    clipped.size.height as u32,
    context.style.rare_inherited_data.image_rendering,
    context.time_ms(),
    context.current_color,
    Some(context.fonts()),
  )?;
  let logical_to_source = if placement.size.width == 0.0 || placement.size.height == 0.0 {
    Affine::IDENTITY
  } else {
    Affine::scale(
      source_width / placement.size.width,
      source_height / placement.size.height,
    ) * Affine::translation(clipped.crop.x, clipped.crop.y)
  };

  Ok(PreparedImage {
    image: rendered,
    logical_to_source,
    offset: crop_origin + clipped.origin,
  })
}

/// Draws an image into its box's content box, clipped to the content box's curve when the box
/// is rounded. Content past the box never renders, so a square box needs no clip.
pub(crate) fn draw_image(
  image: &ImageSource,
  context: &RenderContext,
  canvas: &mut Canvas,
  layout: Layout,
) -> Result<()> {
  let (width, height) = image.size(&context.sizing);
  let replaced = BoxPainter::new(context, layout).replaced_content(Size { width, height });
  let content_offset = layout.content_box_offset();
  let (crop_origin, crop) = match replaced.clip {
    Some(clip) => (clip.offset - content_offset, clip.size),
    None => (replaced.placement.offset, replaced.placement.size),
  };
  let prepared =
    process_image_for_object_fit(image, context, replaced.placement, crop_origin, crop)?;
  let offset = content_offset + prepared.offset;
  let image_to_box = Affine::translation(offset.x, offset.y);
  let rounded_clip = replaced.clip.filter(|clip| !clip.border.is_zero());

  match prepared.image {
    RenderedImage::Rasterized(rendered) => {
      let Some(pixmap) = pixmap_ref_from_buffer(rendered.as_ref()) else {
        return Ok(());
      };

      if let Some(clip) = rounded_clip {
        ImageRect::new(offset, pixmap.width() as f32, pixmap.height() as f32).draw_clipped(
          canvas,
          context,
          clip,
          pixmap.into(),
          Affine::IDENTITY,
          context.style.rare_inherited_data.image_rendering,
        )?;
        return Ok(());
      }

      canvas.overlay_image(
        pixmap,
        BorderProperties::default(),
        context.transform * image_to_box,
        context.style.rare_inherited_data.image_rendering,
        // The node's blend mode applies when its layer composites.
        BlendMode::Normal,
      );
    }
    RenderedImage::Sampled {
      source,
      width,
      height,
      algorithm,
      source_scale,
    } => {
      let Some(pixmap) = pixmap_ref_from_buffer(source.as_ref()) else {
        return Ok(());
      };
      let logical_to_source =
        Affine::scale(source_scale.0, source_scale.1) * prepared.logical_to_source;

      if let Some(clip) = rounded_clip {
        ImageRect::new(offset, width as f32, height as f32).draw_clipped(
          canvas,
          context,
          clip,
          pixmap.into(),
          logical_to_source,
          algorithm,
        )?;
        return Ok(());
      }

      canvas.overlay_sampled_pixmap(
        pixmap,
        Size { width, height },
        BorderProperties::default(),
        context.transform * image_to_box,
        SamplingOptions {
          logical_to_source,
          algorithm,
        },
        BlendMode::Normal,
      );
    }
  }

  Ok(())
}

/// Where an image draws in its box: its top-left and its drawn size.
struct ImageRect {
  offset: Point<f32>,
  size: Size<f32>,
}

impl ImageRect {
  fn new(offset: Point<f32>, width: f32, height: f32) -> Self {
    Self {
      offset,
      size: Size { width, height },
    }
  }

  /// Draws `source` over the rect, clipped to `clip`, finding each drawn pixel's source through
  /// `logical_to_source` from the image's own coordinates.
  fn draw_clipped(
    &self,
    canvas: &mut Canvas,
    context: &RenderContext,
    clip: ClipBox,
    source: PaintSource<'_>,
    logical_to_source: Affine,
    algorithm: ImageScalingAlgorithm,
  ) -> Result<()> {
    let image = FillShape::RoundedRect {
      border: BorderProperties::default(),
      size: self.size,
      offset: self.offset,
    };

    CanvasDevice::paint(canvas, context, |device| {
      device.push_clip(&clip.into(), Affine::IDENTITY);
      device.fill_shape_with_source(
        &image,
        Affine::IDENTITY,
        source,
        logical_to_source * Affine::translation(-self.offset.x, -self.offset.y),
        algorithm,
      );
      device.pop_clip();
    })
  }
}
