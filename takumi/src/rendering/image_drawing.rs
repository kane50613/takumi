use taffy::{Layout, Point, Size};

use crate::layout::style::BlendMode;
use crate::{
  Result,
  layout::style::{Affine, Length, ObjectFit},
  rendering::{BorderProperties, Canvas, RenderContext},
  resources::image::{ImageSource, RenderedImage},
};

pub(crate) struct PreparedImage<'a> {
  image: RenderedImage<'a>,
  logical_to_source: Affine,
}

/// Calculate offset for object-position within available space.
/// Position values are resolved to px relative to content_box, so we need to
/// adjust them to be relative to the available space for proper positioning
fn calculate_object_position_offset(
  available_space: f32,
  total_space: f32,
  position_value: f32,
) -> f32 {
  if total_space > 0.0 {
    // Convert position from content-box-relative to available-space-relative
    // Clamp the ratio to [0, 1] to handle edge cases
    ((position_value / total_space).clamp(0.0, 1.0) * available_space).max(0.0)
  } else {
    0.0
  }
}

/// Process an image according to the specified object-fit style.
///
/// This function handles resizing, cropping, and positioning of images
/// based on the ObjectFit property, returning the processed image and offset.
pub fn process_image_for_object_fit<'i>(
  image: &'i ImageSource,
  context: &RenderContext,
  content_box: Size<f32>,
) -> Result<(PreparedImage<'i>, Point<f32>)> {
  let (image_width, image_height) = image.size(&context.sizing);
  let (source_width, source_height) = match image {
    ImageSource::Bitmap(bitmap) => (bitmap.width() as f32, bitmap.height() as f32),
    #[cfg(feature = "svg")]
    ImageSource::Svg(svg) => (svg.tree.size().width(), svg.tree.size().height()),
  };
  let source_to_intrinsic = if image_width == 0.0 || image_height == 0.0 {
    Affine::IDENTITY
  } else {
    Affine::scale(source_width / image_width, source_height / image_height)
  };

  let object_position_x =
    Length::from(context.style.object_position.0.x).to_px(&context.sizing, content_box.width);
  let object_position_y =
    Length::from(context.style.object_position.0.y).to_px(&context.sizing, content_box.height);

  match context.style.object_fit {
    ObjectFit::Fill => Ok((
      PreparedImage {
        image: image.render_for_layout(
          content_box.width as u32,
          content_box.height as u32,
          context.style.image_rendering,
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
      Point::zero(),
    )),
    ObjectFit::Contain => {
      let scale_x = content_box.width / image_width;
      let scale_y = content_box.height / image_height;
      let scale = scale_x.min(scale_y);

      let new_width = image_width * scale;
      let new_height = image_height * scale;

      let available_x = content_box.width - new_width;
      let available_y = content_box.height - new_height;

      let offset_x =
        calculate_object_position_offset(available_x, content_box.width, object_position_x);
      let offset_y =
        calculate_object_position_offset(available_y, content_box.height, object_position_y);

      Ok((
        PreparedImage {
          image: image.render_for_layout(
            new_width as u32,
            new_height as u32,
            context.style.image_rendering,
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

      let crop_x =
        calculate_object_position_offset(available_crop_x, content_box.width, object_position_x);
      let crop_y =
        calculate_object_position_offset(available_crop_y, content_box.height, object_position_y);

      Ok((
        PreparedImage {
          image: image.render_for_layout(
            content_box.width as u32,
            content_box.height as u32,
            context.style.image_rendering,
            context.current_color,
          )?,
          logical_to_source: if new_width == 0.0 || new_height == 0.0 {
            Affine::IDENTITY
          } else {
            Affine::scale(source_width / new_width, source_height / new_height)
              * Affine::translation(crop_x, crop_y)
          },
        },
        Point::zero(),
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
          context.current_color,
        )?
      } else {
        image.render_for_layout(
          image_width as u32,
          image_height as u32,
          context.style.image_rendering,
          context.current_color,
        )?
      };

      let available_x = content_box.width - new_width;
      let available_y = content_box.height - new_height;

      let offset_x =
        calculate_object_position_offset(available_x, content_box.width, object_position_x);
      let offset_y =
        calculate_object_position_offset(available_y, content_box.height, object_position_y);

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

        let offset_x =
          calculate_object_position_offset(available_x, content_box.width, object_position_x);
        let offset_y =
          calculate_object_position_offset(available_y, content_box.height, object_position_y);

        return Ok((
          PreparedImage {
            image: image.render_for_layout(
              image_width as u32,
              image_height as u32,
              context.style.image_rendering,
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

      let crop_x =
        calculate_object_position_offset(available_crop_x, content_box.width, object_position_x);
      let crop_y =
        calculate_object_position_offset(available_crop_y, content_box.height, object_position_y);

      let crop_width = content_box.width.min(image_width);
      let crop_height = content_box.height.min(image_height);

      let offset_x = calculate_object_position_offset(
        (content_box.width - crop_width).max(0.0),
        content_box.width,
        object_position_x,
      );
      let offset_y = calculate_object_position_offset(
        (content_box.height - crop_height).max(0.0),
        content_box.height,
        object_position_y,
      );

      Ok((
        PreparedImage {
          image: image.render_for_layout(
            crop_width as u32,
            crop_height as u32,
            context.style.image_rendering,
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
pub fn draw_image(
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
    RenderedImage::Rasterized(image) => canvas.overlay_image(
      image.as_ref(),
      border,
      transform_with_content_offset,
      context.style.image_rendering,
      // blend mode will be applied in main render function,
      // therefore we should not apply it here to avoid double application
      BlendMode::Normal,
    ),
    RenderedImage::Borrowed {
      source,
      width,
      height,
      algorithm: algo,
    } => canvas.overlay_sampled_image(
      source,
      width,
      height,
      border,
      transform_with_content_offset,
      image.logical_to_source,
      algo,
      BlendMode::Normal,
    ),
  }

  Ok(())
}
