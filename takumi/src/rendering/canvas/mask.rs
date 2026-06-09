use taffy::{AbsoluteAxis, Layout, Point, Rect, Size};
use tiny_skia::{
  FillRule as TinyFillRule, IntSize, Mask as TinyMask, PathBuilder as TinyPathBuilder,
  Rect as TinyRect, Transform as TinyTransform,
};

use crate::{
  Result,
  layout::style::{
    Affine, Axis, BasicShape, BorderStyle, Color, ComputedStyle, FillRule, ImageScalingAlgorithm,
    Overflow, ShapeRadius, Sides, SizingContext, SpacePair,
  },
  rendering::{
    BorderProperties, Command, Fill, PathBuilder, PathData, Placement, RenderContext, Style,
    build_path,
    canvas::{BufferPool, mask_index_from_coord},
    create_mask, fast_div_255, transformed_rect_extents,
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
    let (mask, placement) = render_clip_shape_mask(clip_path, context, layout.size, buffer_pool);
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
    let mask_placement = Placement {
      left: 0,
      top: 0,
      width: layout.size.width as u32,
      height: layout.size.height as u32,
    };
    let full_mask = if transform.is_identity() {
      copy_mask_to_viewport(viewport, &mask, mask_placement)
    } else {
      rasterize_constraint_mask(viewport, placement, |x, y| {
        sample_mask_image_alpha(
          &mask,
          Point { x: 0, y: 0 },
          Point {
            x: mask_placement.width,
            y: mask_placement.height,
          },
          inverse_transform,
          x,
          y,
        )
      })
    };
    let Some(full_mask) = full_mask else {
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

    let (mask_data, local_placement) = render_mask(&paths, None, None, buffer_pool);
    if local_placement.width == 0 || local_placement.height == 0 {
      buffer_pool.release(mask_data);
      return Ok(NodeMaskAction::SkipRendering);
    }

    let Some(placement) = transformed_local_placement(local_placement, transform) else {
      buffer_pool.release(mask_data);
      return Ok(NodeMaskAction::SkipRendering);
    };

    let from = Point {
      x: local_placement.left.max(0) as u32,
      y: local_placement.top.max(0) as u32,
    };
    let to = Point {
      x: from.x + local_placement.width,
      y: from.y + local_placement.height,
    };
    let full_mask = if transform.is_identity() {
      copy_mask_to_viewport(viewport, &mask_data, local_placement)
    } else {
      rasterize_constraint_mask(viewport, placement, |x, y| {
        sample_overflow_alpha(
          from,
          to,
          inverse_transform,
          Some((&mask_data, local_placement.width)),
          x,
          y,
        )
      })
    };
    let Some(full_mask) = full_mask else {
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
  let mask = if transform.is_identity() {
    fill_rect_mask(viewport, from, to)
  } else {
    rasterize_constraint_mask(viewport, placement, |x, y| {
      sample_overflow_alpha(from, to, inverse_transform, None, x, y)
    })
  };
  let Some(mask) = mask else {
    return Ok(NodeMaskAction::SkipRendering);
  };
  Ok(NodeMaskAction::Content(mask))
}

fn fill_rect_mask(viewport: CanvasViewport, from: Point<u32>, to: Point<u32>) -> Option<TinyMask> {
  let mut mask = TinyMask::new(viewport.size.width, viewport.size.height)?;
  let viewport_right = viewport.right();
  let viewport_bottom = viewport.bottom();
  let start_x = (from.x as i32).max(viewport.origin.x as i32);
  let start_y = (from.y as i32).max(viewport.origin.y as i32);
  let end_x = (to.x as i32).min(viewport_right);
  let end_y = (to.y as i32).min(viewport_bottom);
  if start_x >= end_x || start_y >= end_y {
    return Some(mask);
  }
  let stride = viewport.size.width as usize;
  let data = mask.data_mut();
  let span = (end_x - start_x) as usize;
  for global_y in start_y..end_y {
    let row = (global_y - viewport.origin.y as i32) as usize * stride
      + (start_x - viewport.origin.x as i32) as usize;
    data[row..row + span].fill(u8::MAX);
  }
  Some(mask)
}

struct AlphaOverlap {
  placement: Placement,
  lhs_stride: usize,
  rhs_stride: usize,
  lhs_origin: usize,
  rhs_origin: usize,
}

impl AlphaOverlap {
  fn new(lhs: Placement, rhs: Placement) -> Option<Self> {
    let placement = Placement::from_bounds(
      lhs.left.max(rhs.left),
      lhs.top.max(rhs.top),
      lhs.right().min(rhs.right()),
      lhs.bottom().min(rhs.bottom()),
    )?;
    let lhs_stride = lhs.width as usize;
    let rhs_stride = rhs.width as usize;
    Some(Self {
      lhs_origin: (placement.top - lhs.top) as usize * lhs_stride
        + (placement.left - lhs.left) as usize,
      rhs_origin: (placement.top - rhs.top) as usize * rhs_stride
        + (placement.left - rhs.left) as usize,
      lhs_stride,
      rhs_stride,
      placement,
    })
  }
}

pub(crate) fn intersect_alpha_masks(
  lhs: &[u8],
  lhs_placement: Placement,
  rhs: &[u8],
  rhs_placement: Placement,
) -> Option<(Vec<u8>, Placement)> {
  let overlap = AlphaOverlap::new(lhs_placement, rhs_placement)?;
  let width = overlap.placement.width as usize;
  let height = overlap.placement.height as usize;

  let mut mask = vec![0; width * height];
  for (row_index, mask_row) in mask.chunks_exact_mut(width).enumerate() {
    let lhs_row_start = overlap.lhs_origin + row_index * overlap.lhs_stride;
    let rhs_row_start = overlap.rhs_origin + row_index * overlap.rhs_stride;
    let lhs_row = &lhs[lhs_row_start..lhs_row_start + width];
    let rhs_row = &rhs[rhs_row_start..rhs_row_start + width];

    if lhs_row.iter().all(|&alpha| alpha == 0) || rhs_row.iter().all(|&alpha| alpha == 0) {
      continue;
    }

    for index in 0..width {
      mask_row[index] = fast_div_255(lhs_row[index] as u32 * rhs_row[index] as u32);
    }
  }

  Some((mask, overlap.placement))
}

pub(crate) fn attenuate_alpha_by_mask(
  dst: &mut [u8],
  dst_placement: Placement,
  mask: &[u8],
  mask_placement: Placement,
) {
  let Some(overlap) = AlphaOverlap::new(dst_placement, mask_placement) else {
    return;
  };
  let width = overlap.placement.width as usize;

  for row_index in 0..overlap.placement.height as usize {
    let dst_row_start = overlap.lhs_origin + row_index * overlap.lhs_stride;
    let mask_row_start = overlap.rhs_origin + row_index * overlap.rhs_stride;
    let dst_row = &mut dst[dst_row_start..dst_row_start + width];
    let mask_row = &mask[mask_row_start..mask_row_start + width];

    if mask_row.iter().all(|&alpha| alpha == 0) {
      continue;
    }

    for index in 0..width {
      let mask_alpha = mask_row[index] as u32;
      if mask_alpha == 0 {
        continue;
      }
      let factor = 255 - mask_alpha;
      dst_row[index] = fast_div_255(dst_row[index] as u32 * factor);
    }
  }
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
  let copy_width = (end_x - start_x) as usize;
  for global_y in start_y..end_y {
    let src_y = (global_y - placement.top) as usize;
    let dst_start = (global_y - canvas_top) as usize * stride + (start_x - canvas_left) as usize;
    let src_start = src_y * placement.width as usize + (start_x - placement.left) as usize;
    data[dst_start..dst_start + copy_width]
      .copy_from_slice(&mask[src_start..src_start + copy_width]);
  }
}

fn copy_mask_to_viewport(
  viewport: CanvasViewport,
  mask: &[u8],
  placement: Placement,
) -> Option<TinyMask> {
  let mut full_mask = TinyMask::new(viewport.size.width, viewport.size.height)?;
  copy_mask_into_canvas(&mut full_mask, viewport.origin, mask, placement);
  Some(full_mask)
}

fn rasterize_constraint_mask(
  viewport: CanvasViewport,
  placement: Placement,
  alpha_at: impl Fn(u32, u32) -> u8,
) -> Option<TinyMask> {
  let mut mask = TinyMask::new(viewport.size.width, viewport.size.height)?;

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

fn transformed_placement(
  origin: Point<f32>,
  size: Size<f32>,
  transform: Affine,
) -> Option<Placement> {
  let (left, top, right, bottom) = transformed_rect_extents(origin, size, transform)?;
  Placement::from_bounds(
    left.floor() as i32,
    top.floor() as i32,
    right.ceil() as i32,
    bottom.ceil() as i32,
  )
}

fn transformed_rect_placement(size: Size<f32>, transform: Affine) -> Option<Placement> {
  transformed_placement(Point::ZERO, size, transform)
}

fn transformed_local_placement(local_placement: Placement, transform: Affine) -> Option<Placement> {
  transformed_placement(
    Point {
      x: local_placement.left as f32,
      y: local_placement.top as f32,
    },
    Size {
      width: local_placement.width as f32,
      height: local_placement.height as f32,
    },
    transform,
  )
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

impl From<FillRule> for Fill {
  fn from(value: FillRule) -> Self {
    match value {
      FillRule::EvenOdd => Fill::EvenOdd,
      FillRule::NonZero => Fill::NonZero,
    }
  }
}

fn resolve_radius(
  radius: ShapeRadius,
  distance: Size<f32>,
  sizing: &SizingContext,
  full: f32,
) -> f32 {
  match radius {
    ShapeRadius::ClosestSide => distance.width.min(distance.height),
    ShapeRadius::FarthestSide => distance.width.max(distance.height),
    ShapeRadius::Length(length) => length.to_px(sizing, full),
  }
}

pub(crate) fn render_clip_shape_mask(
  shape: &BasicShape,
  context: &RenderContext,
  size: Size<f32>,
  buffer_pool: &mut BufferPool,
) -> (Vec<u8>, Placement) {
  let mut paths = Vec::new();

  match shape {
    BasicShape::Inset(shape) => {
      let inset: Rect<f32> = shape
        .inset
        .map_axis(|value, axis| {
          value.to_px(
            &context.sizing,
            match axis {
              Axis::Horizontal => size.width,
              Axis::Vertical => size.height,
            },
          )
        })
        .into();

      let border = BorderProperties {
        width: Rect::zero(),
        color: Rect {
          top: Color::transparent(),
          right: Color::transparent(),
          bottom: Color::transparent(),
          left: Color::transparent(),
        },
        radius: shape
          .border_radius
          .map(|radius| {
            Sides(
              radius
                .0
                .map(|corner| SpacePair::from_single(corner.to_px(&context.sizing, size.width))),
            )
          })
          .unwrap_or_default(),
        image_rendering: ImageScalingAlgorithm::Auto,
        style: Rect {
          top: BorderStyle::Solid,
          right: BorderStyle::Solid,
          bottom: BorderStyle::Solid,
          left: BorderStyle::Solid,
        },
      };

      border.append_mask_commands(
        &mut paths,
        Size {
          width: size.width - inset.grid_axis_sum(AbsoluteAxis::Horizontal),
          height: size.height - inset.grid_axis_sum(AbsoluteAxis::Vertical),
        },
        Point {
          x: inset.left,
          y: inset.top,
        },
      );
    }
    BasicShape::Ellipse(shape) => {
      let distance = Size {
        width: shape.position.0.x.to_px(&context.sizing, size.width),
        height: shape.position.0.y.to_px(&context.sizing, size.height),
      };

      paths.add_ellipse(
        (distance.width, distance.height),
        resolve_radius(shape.radius_x, distance, &context.sizing, size.width),
        resolve_radius(shape.radius_y, distance, &context.sizing, size.height),
      );
    }
    BasicShape::Polygon(shape) => {
      if !shape.coordinates.is_empty() {
        let first = &shape.coordinates[0];
        let first_x = first.x.to_px(&context.sizing, size.width);
        let first_y = first.y.to_px(&context.sizing, size.height);

        paths.move_to((first_x, first_y));

        for coord in &shape.coordinates[1..] {
          let x = coord.x.to_px(&context.sizing, size.width);
          let y = coord.y.to_px(&context.sizing, size.height);
          paths.line_to((x, y));
        }

        paths.close();
      }
    }
    BasicShape::Path(shape) => {
      paths.extend(shape.path.as_ref().commands());
    }
  }

  render_mask(
    &paths,
    Some(context.transform),
    Some(Fill::from(shape.fill_rule().unwrap_or(context.style.clip_rule)).into()),
    buffer_pool,
  )
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
    if let Some(dash) = &stroke.dash
      && let Some(dashed_path) = path.dash(dash, 1.0)
    {
      path = dashed_path;
    }

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
  let Some(size) = IntSize::from_wh(width, height) else {
    return (Vec::new(), Placement::default());
  };
  let buffer_len = (width as usize) * (height as usize);
  let buffer = buffer_pool.acquire(buffer_len);
  let Some(mut mask) = TinyMask::from_vec(buffer, size) else {
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

  (
    mask.take(),
    Placement {
      left,
      top,
      width,
      height,
    },
  )
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn intersect_alpha_masks_respects_overlap_placement() {
    let lhs = vec![
      0, 64, 0, 0, //
      0, 255, 0, 0, //
      0, 0, 0, 0, //
    ];
    let rhs = vec![
      0, 0, 0, //
      255, 255, 128, //
      0, 0, 0, //
    ];

    let lhs_placement = Placement {
      left: 0,
      top: 0,
      width: 4,
      height: 3,
    };
    let rhs_placement = Placement {
      left: 1,
      top: 0,
      width: 3,
      height: 3,
    };

    let Some((mask, placement)) = intersect_alpha_masks(&lhs, lhs_placement, &rhs, rhs_placement)
    else {
      unreachable!("should overlap");
    };
    assert_eq!(
      placement,
      Placement {
        left: 1,
        top: 0,
        width: 3,
        height: 3
      }
    );
    assert_eq!(mask[0], 0);
    assert_eq!(mask[3], 255);
    assert_eq!(mask[4], 0);
  }

  #[test]
  fn attenuate_alpha_by_mask_applies_overlap_only() {
    let mut dst = vec![
      255, 255, 255, //
      255, 255, 255, //
      255, 255, 255, //
    ];
    let mask = vec![
      0, 128, 0, //
      255, 0, 0, //
    ];

    let dst_placement = Placement {
      left: 0,
      top: 0,
      width: 3,
      height: 3,
    };
    let mask_placement = Placement {
      left: 1,
      top: 1,
      width: 3,
      height: 2,
    };

    attenuate_alpha_by_mask(&mut dst, dst_placement, &mask, mask_placement);

    assert_eq!(dst[0], 255);
    assert_eq!(dst[4], 255);
    assert_eq!(dst[5], fast_div_255(255 * (255 - 128)));
    assert_eq!(dst[7], 0);
  }
}
