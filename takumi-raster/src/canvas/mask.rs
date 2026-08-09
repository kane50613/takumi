use std::{borrow::Cow, sync::Arc};

use takumi_core::geometry::{ComputedLayout as Layout, Point, Size, transformed_rect_extents};
use tiny_skia::{
  FillRule as TinyFillRule, IntSize, Mask as TinyMask, PathBuilder as TinyPathBuilder,
  Rect as TinyRect, Transform as TinyTransform,
};

use crate::{
  BorderProperties, Command, Fill, Placement, RenderContext, Result, Style, build_path,
  create_mask, fast_div_255,
  layout::clip::clip_shape_commands,
  style::{Affine, BasicShape, ComputedStyle, FillRule, Overflow},
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
) -> Result<NodeMaskAction> {
  if let Some(clip_path) = &style.clip_path {
    let (mask, placement) = render_clip_shape_mask(clip_path, context, layout.size);
    let end_x = placement.left + placement.width as i32;
    let end_y = placement.top + placement.height as i32;

    if end_x < 0 || end_y < 0 {
      return Ok(NodeMaskAction::SkipRendering);
    }

    let Some(mut full_mask) = TinyMask::new(viewport.size.width, viewport.size.height) else {
      return Ok(NodeMaskAction::SkipRendering);
    };
    copy_mask_into_canvas(&mut full_mask, viewport.origin, &mask, placement);
    return Ok(NodeMaskAction::Shell(full_mask));
  }

  let Some(inverse_transform) = transform.invert() else {
    return Ok(NodeMaskAction::SkipRendering);
  };

  if let Some(mask) = create_mask(context, layout.size)? {
    let Some(placement) = transformed_rect_placement(layout.size, transform) else {
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
      return Ok(NodeMaskAction::SkipRendering);
    };
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

    let (mask_data, local_placement) = render_mask(&paths, None, None);
    if local_placement.width == 0 || local_placement.height == 0 {
      return Ok(NodeMaskAction::SkipRendering);
    }

    let Some(placement) = transformed_local_placement(local_placement, transform) else {
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
      return Ok(NodeMaskAction::SkipRendering);
    };
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

  if transform.is_identity()
    && from.x <= viewport.origin.x
    && from.y <= viewport.origin.y
    && (to.x == u32::MAX || to.x as i32 >= viewport.right())
    && (to.y == u32::MAX || to.y as i32 >= viewport.bottom())
  {
    return Ok(NodeMaskAction::None);
  }

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
  let end_x = viewport_right.min(to.x.min(i32::MAX as u32) as i32);
  let end_y = viewport_bottom.min(to.y.min(i32::MAX as u32) as i32);
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
  let (original_x, original_y) = inverse_transform.transform_point(x as f32, y as f32);
  if original_x < 0.0 || original_y < 0.0 {
    return None;
  }

  let original_point = Point {
    x: original_x as u32,
    y: original_y as u32,
  };
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
      _ => Fill::NonZero,
    }
  }
}

pub(crate) fn render_clip_shape_mask(
  shape: &BasicShape,
  context: &RenderContext,
  size: Size<f32>,
) -> (Vec<u8>, Placement) {
  let paths = clip_shape_commands(shape, context, size).unwrap_or_default();
  render_mask(
    &paths,
    Some(context.transform),
    Some(Fill::from(shape.fill_rule().unwrap_or(context.style.clip_rule)).into()),
  )
}

pub(crate) fn render_mask(
  paths: &[Command],
  transform: Option<Affine>,
  style: Option<Style>,
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
  let buffer = vec![0; buffer_len];
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

#[derive(Clone)]
pub(crate) struct MaskStackEntry {
  pub(crate) mask: Arc<TinyMask>,
  pub(crate) origin: Point<u32>,
}

#[derive(Clone, Copy)]
pub(crate) struct MaskView<'a> {
  pub(crate) mask: &'a TinyMask,
  pub(crate) origin: Point<u32>,
  pub(crate) canvas_origin: Point<u32>,
}

impl<'a> MaskView<'a> {
  #[inline]
  pub(crate) fn row(&self, canvas_y: i32, canvas_x_start: i32) -> MaskRow<'a> {
    let local_y = canvas_y + self.canvas_origin.y as i32 - self.origin.y as i32;
    let mask_width = self.mask.width() as i32;
    let mask_height = self.mask.height() as i32;
    if local_y < 0 || local_y >= mask_height {
      return MaskRow::EMPTY;
    }
    let local_x_start = canvas_x_start + self.canvas_origin.x as i32 - self.origin.x as i32;
    let row_offset = local_y as usize * self.mask.width() as usize;
    MaskRow {
      data: self.mask.data(),
      row_offset,
      local_x_start,
      mask_width,
    }
  }
}

#[derive(Clone, Copy)]
pub(crate) struct MaskRow<'a> {
  data: &'a [u8],
  row_offset: usize,
  local_x_start: i32,
  mask_width: i32,
}

impl<'a> MaskRow<'a> {
  const EMPTY: Self = Self {
    data: &[],
    row_offset: 0,
    local_x_start: 0,
    mask_width: 0,
  };

  #[inline]
  pub(crate) fn alpha_at_offset(&self, offset: usize) -> u8 {
    let local_x = self.local_x_start + offset as i32;
    if local_x < 0 || local_x >= self.mask_width {
      return 0;
    }
    self.data[self.row_offset + local_x as usize]
  }

  #[inline]
  pub(crate) fn is_empty(&self) -> bool {
    self.data.is_empty()
  }
}

/// Resolves the combined constraint mask against the pixmap it clips: borrowed
/// directly when the stored mask already matches the canvas viewport, cropped
/// into a scratch buffer otherwise.
pub(crate) fn resolve_mask<'a>(mask: MaskView<'a>, size: Size<u32>) -> Option<Cow<'a, TinyMask>> {
  if mask.origin == mask.canvas_origin
    && mask.mask.width() == size.width
    && mask.mask.height() == size.height
  {
    return Some(Cow::Borrowed(mask.mask));
  }
  materialize_mask(mask, size).map(Cow::Owned)
}

#[inline(always)]
fn mask_index_from_coord(x: u32, y: u32, width: u32) -> usize {
  (y * width + x) as usize
}

pub(crate) fn materialize_mask(mask: MaskView<'_>, size: Size<u32>) -> Option<TinyMask> {
  let mut cropped = TinyMask::from_vec(
    vec![0; (size.width as usize) * (size.height as usize)],
    IntSize::from_wh(size.width, size.height)?,
  )?;

  let offset = Point {
    x: mask.canvas_origin.x as i32 - mask.origin.x as i32,
    y: mask.canvas_origin.y as i32 - mask.origin.y as i32,
  };
  let src_width = mask.mask.width() as i32;
  let src_height = mask.mask.height() as i32;
  let start_x = offset.x.max(0);
  let start_y = offset.y.max(0);
  let end_x = (offset.x + size.width as i32).min(src_width);
  let end_y = (offset.y + size.height as i32).min(src_height);
  if start_x >= end_x || start_y >= end_y {
    return Some(cropped);
  }

  let src = mask.mask.data();
  let dst = cropped.data_mut();
  if start_x == 0
    && start_y == 0
    && end_x == src_width
    && end_y == src_height
    && src_width as u32 == size.width
    && src_height as u32 == size.height
  {
    dst.copy_from_slice(src);
    return Some(cropped);
  }

  let dst_width = size.width as usize;
  let src_width = src_width as usize;
  let copy_width = (end_x - start_x) as usize;
  let dst_x_start = (start_x - offset.x) as usize;
  for src_y in start_y..end_y {
    let dst_y = (src_y - offset.y) as usize;
    let src_row = src_y as usize * src_width + start_x as usize;
    let dst_row = dst_y * dst_width + dst_x_start;
    dst[dst_row..dst_row + copy_width].copy_from_slice(&src[src_row..src_row + copy_width]);
  }

  Some(cropped)
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
