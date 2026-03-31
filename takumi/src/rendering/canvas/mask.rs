use taffy::{Layout, Point, Size};
use tiny_skia::{
  FillRule as TinyFillRule, Mask as TinyMask, PathBuilder as TinyPathBuilder, Rect as TinyRect,
  Transform as TinyTransform,
};

use super::{BufferPool, mask_index_from_coord};
use crate::{
  Result,
  layout::style::{Affine, ComputedStyle, Overflow},
  rendering::{
    BorderProperties, Command, Placement, RenderContext, Style, build_path, create_mask,
  },
};

pub(crate) enum NodeMaskAction {
  Shell(TinyMask),
  Content(TinyMask),
  None,
  SkipRendering,
}

#[derive(Clone, Copy)]
pub(crate) struct CanvasViewport {
  pub(crate) origin: Point<u32>,
  pub(crate) size: Size<u32>,
}

impl CanvasViewport {
  pub(crate) fn right(self) -> i32 {
    self.origin.x as i32 + self.size.width as i32
  }

  pub(crate) fn bottom(self) -> i32 {
    self.origin.y as i32 + self.size.height as i32
  }
}

impl NodeMaskAction {
  pub(crate) fn is_some(&self) -> bool {
    matches!(self, Self::Shell(_) | Self::Content(_))
  }
}

pub(crate) fn prepare_node_mask(
  context: &RenderContext,
  style: &ComputedStyle,
  layout: Layout,
  transform: Affine,
  viewport: CanvasViewport,
  buffer_pool: &mut BufferPool,
) -> Result<NodeMaskAction> {
  if let Some(clip_path) = &style.clip_path {
    let (mask, placement) = clip_path.render_mask(context, layout.size, buffer_pool);
    let end_x = placement.left + placement.width as i32;
    let end_y = placement.top + placement.height as i32;

    if end_x < 0 || end_y < 0 {
      buffer_pool.release(mask);
      return Ok(NodeMaskAction::SkipRendering);
    }

    let Some(mut full_mask) = TinyMask::new(viewport.size.width, viewport.size.height) else {
      buffer_pool.release(mask);
      return Ok(NodeMaskAction::SkipRendering);
    };
    full_mask.data_mut().fill(0);
    copy_mask_into_canvas(&mut full_mask, viewport.origin, &mask, placement);
    buffer_pool.release(mask);
    return Ok(NodeMaskAction::Shell(full_mask));
  }

  let Some(inverse_transform) = transform.invert() else {
    return Ok(NodeMaskAction::SkipRendering);
  };

  if let Some(mask) = create_mask(context, layout.size, buffer_pool)? {
    let Some(placement) = transformed_rect_placement(layout.size, transform) else {
      buffer_pool.release(mask);
      return Ok(NodeMaskAction::SkipRendering);
    };
    let Some(full_mask) = rasterize_constraint_mask(viewport, placement, |x, y| {
      sample_mask_image_alpha(
        &mask,
        Point { x: 0, y: 0 },
        Point {
          x: layout.size.width as u32,
          y: layout.size.height as u32,
        },
        inverse_transform,
        x,
        y,
      )
    }) else {
      buffer_pool.release(mask);
      return Ok(NodeMaskAction::SkipRendering);
    };
    buffer_pool.release(mask);
    return Ok(NodeMaskAction::Shell(full_mask));
  }

  let overflow = style.resolve_overflows();
  let clip_x = overflow.x != Overflow::Visible;
  let clip_y = overflow.y != Overflow::Visible;

  if !overflow.should_clip_content() {
    return Ok(NodeMaskAction::None);
  }

  if (clip_x && layout.content_box_width() < f32::EPSILON)
    || (clip_y && layout.content_box_height() < f32::EPSILON)
  {
    return Ok(NodeMaskAction::SkipRendering);
  }

  let border_props = BorderProperties::from_context(context, layout.size, layout.border);
  if !border_props.is_zero() {
    let padding_box = Size {
      width: (layout.size.width - layout.border.left - layout.border.right).max(0.0),
      height: (layout.size.height - layout.border.top - layout.border.bottom).max(0.0),
    };

    let mut inner_props = border_props;
    inner_props.inset_by_border_width();

    let mut paths = Vec::with_capacity(10);
    let padding_origin = Point {
      x: layout.border.left,
      y: layout.border.top,
    };
    inner_props.append_mask_commands(&mut paths, padding_box, padding_origin);

    let (mask_data, placement) = render_mask(&paths, None, None, buffer_pool);
    if placement.width == 0 || placement.height == 0 {
      buffer_pool.release(mask_data);
      return Ok(NodeMaskAction::SkipRendering);
    }

    let from = Point {
      x: placement.left.max(0) as u32,
      y: placement.top.max(0) as u32,
    };
    let to = Point {
      x: from.x + placement.width,
      y: from.y + placement.height,
    };
    let Some(full_mask) = rasterize_constraint_mask(viewport, placement, |x, y| {
      sample_overflow_alpha(
        from,
        to,
        inverse_transform,
        Some((&mask_data, placement.width)),
        x,
        y,
      )
    }) else {
      buffer_pool.release(mask_data);
      return Ok(NodeMaskAction::SkipRendering);
    };
    buffer_pool.release(mask_data);
    return Ok(NodeMaskAction::Content(full_mask));
  }

  let from = Point {
    x: if clip_x {
      (layout.padding.left + layout.border.left) as u32
    } else {
      0
    },
    y: if clip_y {
      (layout.padding.top + layout.border.top) as u32
    } else {
      0
    },
  };
  let to = Point {
    x: if clip_x {
      from.x + layout.content_box_width() as u32
    } else {
      u32::MAX
    },
    y: if clip_y {
      from.y + layout.content_box_height() as u32
    } else {
      u32::MAX
    },
  };

  if to.x != u32::MAX
    && to.y != u32::MAX
    && let Some(rect) = TinyRect::from_ltrb(from.x as f32, from.y as f32, to.x as f32, to.y as f32)
    && let Some(forward_transform) = inverse_transform.invert()
    && let Some(mut mask) = TinyMask::new(viewport.size.width, viewport.size.height)
  {
    mask.data_mut().fill(u8::MAX);
    let path = TinyPathBuilder::from_rect(rect);
    let localized_transform =
      Affine::translation(-(viewport.origin.x as f32), -(viewport.origin.y as f32))
        * forward_transform;
    mask.intersect_path(
      &path,
      TinyFillRule::Winding,
      true,
      TinyTransform::from(localized_transform),
    );
    return Ok(NodeMaskAction::Content(mask));
  }

  let Some(placement) = overflow_mask_placement(layout.size, transform, viewport, clip_x, clip_y)
  else {
    return Ok(NodeMaskAction::SkipRendering);
  };
  let Some(mask) = rasterize_constraint_mask(viewport, placement, |x, y| {
    sample_overflow_alpha(from, to, inverse_transform, None, x, y)
  }) else {
    return Ok(NodeMaskAction::SkipRendering);
  };
  Ok(NodeMaskAction::Content(mask))
}

fn overflow_mask_placement(
  size: Size<f32>,
  transform: Affine,
  viewport: CanvasViewport,
  clip_x: bool,
  clip_y: bool,
) -> Option<Placement> {
  let mut placement = transformed_rect_placement(size, transform)?;

  if clip_x == clip_y {
    return Some(placement);
  }

  if !transform.only_translation() {
    return Some(Placement {
      left: viewport.origin.x as i32,
      top: viewport.origin.y as i32,
      width: viewport.size.width,
      height: viewport.size.height,
    });
  }

  if !clip_x {
    placement.left = viewport.origin.x as i32;
    placement.width = viewport.size.width;
  }

  if !clip_y {
    placement.top = viewport.origin.y as i32;
    placement.height = viewport.size.height;
  }

  Some(placement)
}

fn copy_mask_into_canvas(
  canvas_mask: &mut TinyMask,
  canvas_origin: Point<u32>,
  mask: &[u8],
  placement: Placement,
) {
  let canvas_left = canvas_origin.x as i32;
  let canvas_top = canvas_origin.y as i32;
  let canvas_right = canvas_left + canvas_mask.width() as i32;
  let canvas_bottom = canvas_top + canvas_mask.height() as i32;
  let start_x = placement.left.max(canvas_left);
  let start_y = placement.top.max(canvas_top);
  let end_x = (placement.left + placement.width as i32).min(canvas_right);
  let end_y = (placement.top + placement.height as i32).min(canvas_bottom);

  if start_x >= end_x || start_y >= end_y {
    return;
  }

  let stride = canvas_mask.width() as usize;
  let data = canvas_mask.data_mut();
  for global_y in start_y..end_y {
    let src_y = (global_y - placement.top) as usize;
    let canvas_row = (global_y - canvas_top) as usize * stride;
    let src_row = src_y * placement.width as usize;
    for global_x in start_x..end_x {
      let src_x = (global_x - placement.left) as usize;
      data[canvas_row + (global_x - canvas_left) as usize] = mask[src_row + src_x];
    }
  }
}

fn rasterize_constraint_mask(
  viewport: CanvasViewport,
  placement: Placement,
  alpha_at: impl Fn(u32, u32) -> u8,
) -> Option<TinyMask> {
  let mut mask = TinyMask::new(viewport.size.width, viewport.size.height)?;
  mask.data_mut().fill(0);

  let start_x = placement.left.max(viewport.origin.x as i32);
  let start_y = placement.top.max(viewport.origin.y as i32);
  let end_x = (placement.left + placement.width as i32).min(viewport.right());
  let end_y = (placement.top + placement.height as i32).min(viewport.bottom());
  if start_x >= end_x || start_y >= end_y {
    return Some(mask);
  }

  let data = mask.data_mut();
  let stride = viewport.size.width as usize;
  for global_y in start_y..end_y {
    let row = (global_y - viewport.origin.y as i32) as usize * stride;
    for global_x in start_x..end_x {
      data[row + (global_x - viewport.origin.x as i32) as usize] =
        alpha_at(global_x as u32, global_y as u32);
    }
  }
  Some(mask)
}

fn transformed_rect_placement(size: Size<f32>, transform: Affine) -> Option<Placement> {
  let corners = [
    transform.transform_point(Point::ZERO),
    transform.transform_point(Point {
      x: size.width,
      y: 0.0,
    }),
    transform.transform_point(Point {
      x: 0.0,
      y: size.height,
    }),
    transform.transform_point(Point {
      x: size.width,
      y: size.height,
    }),
  ];

  let mut left = f32::INFINITY;
  let mut top = f32::INFINITY;
  let mut right = f32::NEG_INFINITY;
  let mut bottom = f32::NEG_INFINITY;
  for point in corners {
    left = left.min(point.x);
    top = top.min(point.y);
    right = right.max(point.x);
    bottom = bottom.max(point.y);
  }

  let left = left.floor() as i32;
  let top = top.floor() as i32;
  let right = right.ceil() as i32;
  let bottom = bottom.ceil() as i32;
  if right <= left || bottom <= top {
    return None;
  }

  Some(Placement {
    left,
    top,
    width: (right - left) as u32,
    height: (bottom - top) as u32,
  })
}

fn sample_mask_image_alpha(
  mask: &[u8],
  from: Point<u32>,
  to: Point<u32>,
  inverse_transform: Affine,
  x: u32,
  y: u32,
) -> u8 {
  let Some(original_point) = transformed_mask_point(inverse_transform, from, to, x, y) else {
    return 0;
  };
  mask[mask_index_from_coord(original_point.x, original_point.y, to.x - from.x)]
}

fn sample_overflow_alpha(
  from: Point<u32>,
  to: Point<u32>,
  inverse_transform: Affine,
  border_radius_mask: Option<(&[u8], u32)>,
  x: u32,
  y: u32,
) -> u8 {
  let Some(original_point) = transformed_mask_point(inverse_transform, from, to, x, y) else {
    return 0;
  };

  if let Some((mask, mask_width)) = border_radius_mask {
    let mask_x = original_point.x - from.x;
    let mask_y = original_point.y - from.y;
    return mask[mask_index_from_coord(mask_x, mask_y, mask_width)];
  }

  u8::MAX
}

fn transformed_mask_point(
  inverse_transform: Affine,
  from: Point<u32>,
  to: Point<u32>,
  x: u32,
  y: u32,
) -> Option<Point<u32>> {
  let original_point = inverse_transform.transform_point(Point {
    x: x as f32,
    y: y as f32,
  });
  if original_point.x < 0.0 || original_point.y < 0.0 {
    return None;
  }

  let original_point = original_point.map(|point| point as u32);
  let is_contained = original_point.x >= from.x
    && original_point.x < to.x
    && original_point.y >= from.y
    && original_point.y < to.y;
  is_contained.then_some(original_point)
}

pub(crate) fn render_mask(
  paths: &[Command],
  transform: Option<Affine>,
  style: Option<Style>,
  buffer_pool: &mut BufferPool,
) -> (Vec<u8>, Placement) {
  let style = style.unwrap_or_default();
  let Some(mut path) = build_path(paths) else {
    return (Vec::new(), Placement::default());
  };

  if let Some(stroke) = style.stroke() {
    let Some(stroked_path) = path.stroke(&stroke, 1.0) else {
      return (Vec::new(), Placement::default());
    };
    path = stroked_path;
  }

  if let Some(transform) = transform {
    let Some(transformed) = path.transform(transform.into()) else {
      return (Vec::new(), Placement::default());
    };
    path = transformed;
  }

  let Some(bounds) = path.compute_tight_bounds() else {
    return (Vec::new(), Placement::default());
  };
  let left = bounds.left().floor() as i32;
  let top = bounds.top().floor() as i32;
  let right = bounds.right().ceil() as i32;
  let bottom = bounds.bottom().ceil() as i32;

  if right <= left || bottom <= top {
    return (Vec::new(), Placement::default());
  }

  let width = (right - left) as u32;
  let height = (bottom - top) as u32;
  let Some(mut mask) = TinyMask::new(width, height) else {
    return (Vec::new(), Placement::default());
  };
  let Some(local_path) =
    path.transform(TinyTransform::from_translate(-(left as f32), -(top as f32)))
  else {
    return (Vec::new(), Placement::default());
  };
  mask.fill_path(
    &local_path,
    style.fill_rule(),
    true,
    TinyTransform::identity(),
  );

  let mut buffer = buffer_pool.acquire(mask.data().len());
  buffer.copy_from_slice(mask.data());
  (
    buffer,
    Placement {
      left,
      top,
      width,
      height,
    },
  )
}
