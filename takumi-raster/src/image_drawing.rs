use takumi_core::{
  geometry::{ComputedLayout as Layout, Point, Size},
  layout::replaced::place_replaced,
};

use crate::{
  BorderProperties, Canvas, RenderContext, Result, SamplingOptions, pixmap_ref_from_buffer,
  resources::image::{ImageSource, RenderedImage},
  style::{Affine, BlendMode},
};

pub(crate) struct PreparedImage {
  image: RenderedImage,
  logical_to_source: Affine,
}

/// Sizes and places an image for `object-fit`/`object-position`, rendering
/// only the part that lands inside the content box.
pub(crate) fn process_image_for_object_fit(
  image: &ImageSource,
  context: &RenderContext,
  content_box: Size<f32>,
) -> Result<(PreparedImage, Point<f32>)> {
  let (image_width, image_height) = image.size(&context.sizing);
  let (source_width, source_height) = match image {
    ImageSource::Bitmap(bitmap) => (bitmap.width() as f32, bitmap.height() as f32),
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
  let placement = place_replaced(
    context,
    content_box,
    Size {
      width: image_width,
      height: image_height,
    },
  );
  let clipped = placement.clipped(content_box);
  let rendered = image.render_for_layout(
    clipped.size.width as u32,
    clipped.size.height as u32,
    context.style.image_rendering,
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

  Ok((
    PreparedImage {
      image: rendered,
      logical_to_source,
    },
    clipped.origin,
  ))
}

/// Draws an image on the canvas with the specified style and layout.
pub(crate) fn draw_image(
  image: &ImageSource,
  context: &RenderContext,
  canvas: &mut Canvas,
  layout: Layout,
) -> Result<()> {
  let (image, offset) = process_image_for_object_fit(image, context, layout.content_box_size())?;

  // manually apply the border and padding to ensure rotation with origin is applied correctly
  let transform_with_content_offset = context.transform
    * Affine::translation(
      layout.border.left + layout.padding.left + offset.x,
      layout.border.top + layout.padding.top + offset.y,
    );

  let mut border = BorderProperties::from_context(context, layout.size, layout.border);
  border.inset_by_border_width();

  match image.image {
    RenderedImage::Rasterized(image) => {
      if let Some(pixmap_ref) = pixmap_ref_from_buffer(image.as_ref()) {
        canvas.overlay_image(
          pixmap_ref,
          border,
          transform_with_content_offset,
          context.style.image_rendering,
          // blend mode will be applied in main render function,
          // therefore we should not apply it here to avoid double application
          BlendMode::Normal,
        );
      }
    }
    RenderedImage::Sampled {
      source,
      width,
      height,
      algorithm: algo,
      source_scale,
    } => {
      if let Some(pixmap_ref) = pixmap_ref_from_buffer(source.as_ref()) {
        canvas.overlay_sampled_pixmap(
          pixmap_ref,
          Size { width, height },
          border,
          transform_with_content_offset,
          SamplingOptions {
            logical_to_source: Affine::scale(source_scale.0, source_scale.1)
              * image.logical_to_source,
            algorithm: algo,
          },
          BlendMode::Normal,
        );
      }
    }
  }

  Ok(())
}
