use takumi_core::geometry::{ComputedLayout as Layout, Point, Size};

use crate::{
  BorderProperties, Canvas, RenderContext, Result, SamplingOptions, pixmap_ref_from_buffer,
  resources::image::{ImageSource, RenderedImage},
  style::{Affine, BlendMode, ObjectFit},
};

pub(crate) struct PreparedImage {
  image: RenderedImage,
  logical_to_source: Affine,
}

/// Process an image according to the specified object-fit style.
///
/// This function handles resizing, cropping, and positioning of images
/// based on the ObjectFit property, returning the processed image and offset.
pub(crate) fn process_image_for_object_fit(
  image: &ImageSource,
  context: &RenderContext,
  content_box: Size<f32>,
) -> Result<(PreparedImage, Point<f32>)> {
  let (image_width, image_height) = image.size(&context.sizing);
  let (source_width, source_height) = match image {
    ImageSource::Bitmap(bitmap) => (bitmap.width() as f32, bitmap.height() as f32),
    ImageSource::Gif(gif) => {
      let (width, height) = gif.dimensions();
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
  let source_to_intrinsic = if image_width == 0.0 || image_height == 0.0 {
    Affine::IDENTITY
  } else {
    Affine::scale(source_width / image_width, source_height / image_height)
  };

  let object_position = context.style.object_position.0;

  match context.style.object_fit {
    ObjectFit::Fill => Ok((
      PreparedImage {
        image: image.render_for_layout(
          content_box.width as u32,
          content_box.height as u32,
          context.style.image_rendering,
          context.time_ms,
          context.current_color,
        )?,
        logical_to_source: if content_box.width == 0.0 || content_box.height == 0.0 {
          Affine::IDENTITY
        } else {
          Affine::scale(
            source_width / content_box.width,
            source_height / content_box.height,
          )
        },
      },
      Point::ZERO,
    )),
    ObjectFit::Contain => {
      let scale_x = content_box.width / image_width;
      let scale_y = content_box.height / image_height;
      let scale = scale_x.min(scale_y);

      let new_width = image_width * scale;
      let new_height = image_height * scale;

      let available_x = content_box.width - new_width;
      let available_y = content_box.height - new_height;

      let offset_x = object_position.x.resolve(context, available_x);
      let offset_y = object_position.y.resolve(context, available_y);

      Ok((
        PreparedImage {
          image: image.render_for_layout(
            new_width as u32,
            new_height as u32,
            context.style.image_rendering,
            context.time_ms,
            context.current_color,
          )?,
          logical_to_source: if new_width == 0.0 || new_height == 0.0 {
            Affine::IDENTITY
          } else {
            Affine::scale(source_width / new_width, source_height / new_height)
          },
        },
        Point {
          x: offset_x,
          y: offset_y,
        },
      ))
    }
    ObjectFit::Cover => {
      let scale_x = content_box.width / image_width;
      let scale_y = content_box.height / image_height;
      let scale = scale_x.max(scale_y);

      let new_width = image_width * scale;
      let new_height = image_height * scale;

      let available_crop_x = new_width - content_box.width;
      let available_crop_y = new_height - content_box.height;

      let crop_x = object_position.x.resolve(context, available_crop_x);
      let crop_y = object_position.y.resolve(context, available_crop_y);

      Ok((
        PreparedImage {
          image: image.render_for_layout(
            content_box.width as u32,
            content_box.height as u32,
            context.style.image_rendering,
            context.time_ms,
            context.current_color,
          )?,
          logical_to_source: if new_width == 0.0 || new_height == 0.0 {
            Affine::IDENTITY
          } else {
            Affine::scale(source_width / new_width, source_height / new_height)
              * Affine::translation(crop_x, crop_y)
          },
        },
        Point::ZERO,
      ))
    }
    ObjectFit::ScaleDown => {
      let scale_x = content_box.width / image_width;
      let scale_y = content_box.height / image_height;
      let scale = scale_x.min(scale_y).min(1.0);

      let new_width = image_width * scale;
      let new_height = image_height * scale;

      let processed_image = if scale < 1.0 {
        image.render_for_layout(
          new_width as u32,
          new_height as u32,
          context.style.image_rendering,
          context.time_ms,
          context.current_color,
        )?
      } else {
        image.render_for_layout(
          image_width as u32,
          image_height as u32,
          context.style.image_rendering,
          context.time_ms,
          context.current_color,
        )?
      };

      let available_x = content_box.width - new_width;
      let available_y = content_box.height - new_height;

      let offset_x = object_position.x.resolve(context, available_x);
      let offset_y = object_position.y.resolve(context, available_y);

      Ok((
        PreparedImage {
          image: processed_image,
          logical_to_source: if scale < 1.0 && new_width > 0.0 && new_height > 0.0 {
            Affine::scale(source_width / new_width, source_height / new_height)
          } else {
            source_to_intrinsic
          },
        },
        Point {
          x: offset_x,
          y: offset_y,
        },
      ))
    }
    ObjectFit::None => {
      // If the image is smaller than the content box, we don't need to crop
      if image_width <= content_box.width && image_height <= content_box.height {
        let available_x = (content_box.width - image_width).max(0.0);
        let available_y = (content_box.height - image_height).max(0.0);

        let offset_x = object_position.x.resolve(context, available_x);
        let offset_y = object_position.y.resolve(context, available_y);

        return Ok((
          PreparedImage {
            image: image.render_for_layout(
              image_width as u32,
              image_height as u32,
              context.style.image_rendering,
              context.time_ms,
              context.current_color,
            )?,
            logical_to_source: source_to_intrinsic,
          },
          Point {
            x: offset_x,
            y: offset_y,
          },
        ));
      }

      let available_crop_x = (image_width - content_box.width).max(0.0);
      let available_crop_y = (image_height - content_box.height).max(0.0);

      let crop_x = object_position.x.resolve(context, available_crop_x);
      let crop_y = object_position.y.resolve(context, available_crop_y);

      let crop_width = content_box.width.min(image_width);
      let crop_height = content_box.height.min(image_height);

      let offset_x = object_position
        .x
        .resolve(context, (content_box.width - crop_width).max(0.0));
      let offset_y = object_position
        .y
        .resolve(context, (content_box.height - crop_height).max(0.0));

      Ok((
        PreparedImage {
          image: image.render_for_layout(
            crop_width as u32,
            crop_height as u32,
            context.style.image_rendering,
            context.time_ms,
            context.current_color,
          )?,
          logical_to_source: source_to_intrinsic * Affine::translation(crop_x, crop_y),
        },
        Point {
          x: offset_x,
          y: offset_y,
        },
      ))
    }
  }
}

/// Draws an image on the canvas with the specified style and layout.
///
/// The image will be resized and positioned according to the object_fit style property.
/// Border radius will be applied if specified in the style.
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
