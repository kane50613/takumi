use taffy::{Layout, Point, Size};

use crate::layout::style::BlendMode;
use crate::{
  BorderProperties, Canvas, RenderContext, Result, SamplingOptions,
  layout::style::{
    Affine, ObjectFit, PositionComponent, PositionKeywordX, PositionKeywordY, SizingContext,
  },
  pixmap_ref_from_buffer,
  resources::image::{ImageSource, RenderedImage},
};

pub(crate) struct PreparedImage<'a> {
  image: RenderedImage<'a>,
  logical_to_source: Affine,
}

fn resolve_object_position_axis(
  component: PositionComponent,
  sizing: &SizingContext,
  available_space: f32,
) -> f32 {
  match component {
    PositionComponent::KeywordX(keyword) => match keyword {
      PositionKeywordX::Left => 0.0,
      PositionKeywordX::Center => available_space * 0.5,
      PositionKeywordX::Right => available_space,
    },
    PositionComponent::KeywordY(keyword) => match keyword {
      PositionKeywordY::Top => 0.0,
      PositionKeywordY::Center => available_space * 0.5,
      PositionKeywordY::Bottom => available_space,
    },
    PositionComponent::Length(length) => length.to_px(sizing, available_space),
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
    ImageSource::Gif(gif) => {
      let rendered = gif.frame_at_time(context.time_ms);
      (rendered.width() as f32, rendered.height() as f32)
    }
    #[cfg(feature = "svg")]
    ImageSource::Svg(svg) => (svg.tree.size().width(), svg.tree.size().height()),
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

      let offset_x = resolve_object_position_axis(object_position.x, &context.sizing, available_x);
      let offset_y = resolve_object_position_axis(object_position.y, &context.sizing, available_y);

      Ok((
        PreparedImage {
          image: image.render_for_layout(
            new_width as u32,
            new_height as u32,
            context.style.image_rendering,
            context.time_ms,
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
        resolve_object_position_axis(object_position.x, &context.sizing, available_crop_x);
      let crop_y =
        resolve_object_position_axis(object_position.y, &context.sizing, available_crop_y);

      Ok((
        PreparedImage {
          image: image.render_for_layout(
            content_box.width as u32,
            content_box.height as u32,
            context.style.image_rendering,
            context.time_ms,
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
          context.time_ms,
        )?
      } else {
        image.render_for_layout(
          image_width as u32,
          image_height as u32,
          context.style.image_rendering,
          context.time_ms,
        )?
      };

      let available_x = content_box.width - new_width;
      let available_y = content_box.height - new_height;

      let offset_x = resolve_object_position_axis(object_position.x, &context.sizing, available_x);
      let offset_y = resolve_object_position_axis(object_position.y, &context.sizing, available_y);

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
          resolve_object_position_axis(object_position.x, &context.sizing, available_x);
        let offset_y =
          resolve_object_position_axis(object_position.y, &context.sizing, available_y);

        return Ok((
          PreparedImage {
            image: image.render_for_layout(
              image_width as u32,
              image_height as u32,
              context.style.image_rendering,
              context.time_ms,
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
        resolve_object_position_axis(object_position.x, &context.sizing, available_crop_x);
      let crop_y =
        resolve_object_position_axis(object_position.y, &context.sizing, available_crop_y);

      let crop_width = content_box.width.min(image_width);
      let crop_height = content_box.height.min(image_height);

      let offset_x = resolve_object_position_axis(
        object_position.x,
        &context.sizing,
        (content_box.width - crop_width).max(0.0),
      );
      let offset_y = resolve_object_position_axis(
        object_position.y,
        &context.sizing,
        (content_box.height - crop_height).max(0.0),
      );

      Ok((
        PreparedImage {
          image: image.render_for_layout(
            crop_width as u32,
            crop_height as u32,
            context.style.image_rendering,
            context.time_ms,
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
    RenderedImage::Borrowed {
      source,
      width,
      height,
      algorithm: algo,
    } => {
      if let Some(pixmap_ref) = pixmap_ref_from_buffer(source) {
        canvas.overlay_sampled_pixmap(
          pixmap_ref,
          Size { width, height },
          border,
          transform_with_content_offset,
          SamplingOptions {
            logical_to_source: image.logical_to_source,
            algorithm: algo,
          },
          BlendMode::Normal,
        );
      }
    }
  }

  Ok(())
}

#[cfg(test)]
mod tests {
  use std::rc::Rc;

  use super::resolve_object_position_axis;
  use crate::layout::{
    Viewport,
    style::{CalcArena, Length, PositionComponent, PositionKeywordX, SizingContext},
  };
  use taffy::Size;

  fn sizing() -> SizingContext {
    SizingContext {
      viewport: Viewport::new((1200, 630)),
      container_size: Size::NONE,
      font_size: 16.0,
      root_font_size: None,
      line_height: 0.0,
      root_line_height: None,
      calc_arena: Rc::new(CalcArena::default()),
    }
  }

  #[test]
  fn object_position_keyword_center_uses_half_free_space() {
    let resolved = resolve_object_position_axis(
      PositionComponent::KeywordX(PositionKeywordX::Center),
      &sizing(),
      120.0,
    );
    assert_eq!(resolved, 60.0);
  }

  #[test]
  fn object_position_length_is_not_scaled_by_container_size() {
    let resolved = resolve_object_position_axis(
      PositionComponent::Length(Length::Px(12.0)),
      &sizing(),
      120.0,
    );
    assert_eq!(resolved, 12.0);
  }

  #[test]
  fn object_position_percentage_supports_out_of_range_values() {
    let resolved = resolve_object_position_axis(
      PositionComponent::Length(Length::Percentage(150.0)),
      &sizing(),
      120.0,
    );
    assert_eq!(resolved, 180.0);
  }
}
