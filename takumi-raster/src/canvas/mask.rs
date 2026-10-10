use std::{borrow::Cow, sync::Arc};

use smallvec::SmallVec;

use takumi_core::{
  geometry::{ComputedLayout as Layout, Point, Size, transformed_rect_extents},
  paint_property::ClipNode,
  painter::{BoxPainter, FillShape},
  scene::SceneBounds,
};
use tiny_skia::{IntSize, Mask as TinyMask, Transform as TinyTransform};

use crate::{
  BoxMask, Command, Fill, Placement, RenderContext, Result, Style, build_path, checked_area,
  create_mask, fast_div_255, placement_overlap, style::Affine,
};

pub(crate) enum NodeMaskAction {
  Shell(TinyMask),
  SkipRendering,
}

#[derive(Clone, Copy)]
pub(crate) struct CanvasViewport {
  pub(crate) origin: Point<u32>,
  pub(crate) size: Size<u32>,
}

impl CanvasViewport {
  /// A viewport for coordinates already localized to a buffer's own origin.
  pub(crate) fn local(size: Size<u32>) -> Self {
    Self {
      origin: Point { x: 0, y: 0 },
      size,
    }
  }

  pub(crate) fn right(self) -> i32 {
    self.origin.x as i32 + self.size.width as i32
  }

  pub(crate) fn bottom(self) -> i32 {
    self.origin.y as i32 + self.size.height as i32
  }

  pub(crate) fn placement(self) -> Placement {
    Placement {
      left: self.origin.x as i32,
      top: self.origin.y as i32,
      width: self.size.width,
      height: self.size.height,
    }
  }

  pub(crate) fn intersects(self, bounds: SceneBounds) -> bool {
    !bounds.is_empty()
      && bounds.right as i32 > self.origin.x as i32
      && bounds.bottom as i32 > self.origin.y as i32
      && (bounds.left as i32) < self.right()
      && (bounds.top as i32) < self.bottom()
  }

  /// The part of `bounds`, grown by `padding` on every side, inside the viewport.
  pub(crate) fn clamp_bounds(self, bounds: SceneBounds, padding: i32) -> Option<Placement> {
    Placement::from_bounds(
      (bounds.left as i32 - padding).max(self.origin.x as i32),
      (bounds.top as i32 - padding).max(self.origin.y as i32),
      (bounds.right as i32 + padding).min(self.right()),
      (bounds.bottom as i32 + padding).min(self.bottom()),
    )
  }

  /// Grows the viewport for masks whose pixels move before landing on the
  /// canvas (shadow offset, blur bleed). Margins cap at `2^20` so the `i32`
  /// edge arithmetic cannot wrap.
  pub(crate) fn inflate(self, margin_x: f32, margin_y: f32) -> Self {
    const MAX_MARGIN: f32 = (1 << 20) as f32;
    let margin_x = margin_x.ceil().clamp(0.0, MAX_MARGIN) as u32;
    let margin_y = margin_y.ceil().clamp(0.0, MAX_MARGIN) as u32;

    Self {
      origin: Point {
        x: self.origin.x.saturating_sub(margin_x),
        y: self.origin.y.saturating_sub(margin_y),
      },
      size: Size {
        width: self.size.width.saturating_add(margin_x * 2),
        height: self.size.height.saturating_add(margin_y * 2),
      },
    }
  }
}

/// The masks a node's `clip-path` and `mask-image` lay over the box and its descendants.
#[derive(Default)]
pub(crate) struct NodeMasks {
  pub(crate) shell: SmallVec<[TinyMask; 2]>,
}

impl NodeMasks {
  /// The masks of the node `context` paints at `layout` under `transform`, or `None` when one of
  /// them hides the node entirely.
  pub(crate) fn of(
    context: &RenderContext,
    layout: Layout,
    transform: Affine,
    viewport: CanvasViewport,
  ) -> Result<Option<Self>> {
    let mut masks = Self::default();

    if let Some(shape) = BoxPainter::new(context, layout).clip_path()
      && !masks.add(clip_path_mask(&shape, context, viewport))
    {
      return Ok(None);
    }

    let Some(inverse_transform) = transform.invert() else {
      return Ok(None);
    };

    if let Some(mask) = create_mask(context, layout)?
      && !masks.add(mask_image_mask(
        &mask,
        transform,
        inverse_transform,
        viewport,
      ))
    {
      return Ok(None);
    }

    Ok(Some(masks))
  }

  /// Adds the mask `action` makes, reporting false when it hides the node.
  fn add(&mut self, action: NodeMaskAction) -> bool {
    match action {
      NodeMaskAction::Shell(mask) => self.shell.push(mask),
      NodeMaskAction::SkipRendering => return false,
    }

    true
  }

  /// The `clip-path` and `mask-image` masks as one: their product, which bounds what the node
  /// shows of its filtered backdrop.
  pub(crate) fn into_shell_mask(self) -> Option<TinyMask> {
    let mut masks = self.shell.into_iter();
    let mut combined = masks.next()?;

    for mask in masks {
      for (alpha, other) in combined.data_mut().iter_mut().zip(mask.data()) {
        *alpha = fast_div_255(*alpha as u32 * *other as u32);
      }
    }
    Some(combined)
  }

  /// How many masks the node pushes.
  pub(crate) fn len(&self) -> usize {
    self.shell.len()
  }
}

/// The region `clip` keeps, as a viewport mask.
pub(crate) fn clip_node_mask(clip: &ClipNode, viewport: CanvasViewport) -> Option<TinyMask> {
  let (mask, placement) = render_mask(
    &clip.shape.to_commands(),
    Some(clip.transform),
    Some(Fill::from(clip.shape.rule()).into()),
    Some(viewport),
  );

  viewport_mask(viewport, mask.into(), placement)
}

/// The `clip-path` shape as a viewport mask over the box and its descendants.
fn clip_path_mask(
  shape: &FillShape,
  context: &RenderContext,
  viewport: CanvasViewport,
) -> NodeMaskAction {
  let (mask, placement) = render_mask(
    &shape.to_commands(),
    Some(context.transform),
    Some(Fill::from(shape.rule()).into()),
    Some(viewport),
  );

  if placement.width == 0 || placement.height == 0 {
    return NodeMaskAction::SkipRendering;
  }

  viewport_mask(viewport, mask.into(), placement)
    .map_or(NodeMaskAction::SkipRendering, NodeMaskAction::Shell)
}

/// The `mask-image` layers as a viewport mask over the box and its descendants.
fn mask_image_mask(
  mask: &BoxMask,
  transform: Affine,
  inverse_transform: Affine,
  viewport: CanvasViewport,
) -> NodeMaskAction {
  if mask.alpha.is_empty() {
    return NodeMaskAction::SkipRendering;
  }

  let size = mask.size.map(|size| size as f32);
  let Some(placement) = transformed_placement(mask.offset, size, transform) else {
    return NodeMaskAction::SkipRendering;
  };
  let full_mask = if transform.is_identity() && mask.offset == Point::ZERO {
    viewport_mask(
      viewport,
      Cow::Borrowed(&mask.alpha),
      Placement {
        left: 0,
        top: 0,
        width: mask.size.width,
        height: mask.size.height,
      },
    )
  } else {
    let inverse_transform = Affine::translation(-mask.offset.x, -mask.offset.y) * inverse_transform;

    rasterize_constraint_mask(viewport, placement, |x, y| {
      sample_overflow_alpha(
        Point { x: 0, y: 0 },
        Point {
          x: mask.size.width,
          y: mask.size.height,
        },
        inverse_transform,
        Some((&mask.alpha, mask.size.width)),
        x,
        y,
      )
    })
  };

  full_mask.map_or(NodeMaskAction::SkipRendering, NodeMaskAction::Shell)
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
    let overlap = placement_overlap(lhs, rhs)?;
    let lhs_stride = lhs.width as usize;
    let rhs_stride = rhs.width as usize;
    Some(Self {
      lhs_origin: overlap.lhs_offset.y as usize * lhs_stride + overlap.lhs_offset.x as usize,
      rhs_origin: overlap.rhs_offset.y as usize * rhs_stride + overlap.rhs_offset.x as usize,
      lhs_stride,
      rhs_stride,
      placement: overlap.placement,
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

    for (alpha, &mask_alpha) in dst_row.iter_mut().zip(mask_row) {
      *alpha = fast_div_255(*alpha as u32 * (255 - mask_alpha as u32));
    }
  }
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

/// `mask` at `placement` as a mask covering `viewport`, reusing its buffer when they coincide.
fn viewport_mask(
  viewport: CanvasViewport,
  mask: Cow<'_, [u8]>,
  placement: Placement,
) -> Option<TinyMask> {
  if placement == viewport.placement() {
    let size = IntSize::from_wh(placement.width, placement.height)?;

    return TinyMask::from_vec(mask.into_owned(), size);
  }

  let mut full_mask = TinyMask::new(viewport.size.width, viewport.size.height)?;
  copy_mask_into_canvas(&mut full_mask, viewport.origin, &mask, placement);
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
    return mask[(mask_y * mask_width + mask_x) as usize];
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

/// `cull` bounds the rasterized area for masks the caller only ever reads
/// through the canvas. Blink clips the same way in `ClipPathClipper::PaintClipPath`
/// ("we clip by the cull rect here. Visually, this should be a NOP").
pub(crate) fn render_mask(
  paths: &[Command],
  transform: Option<Affine>,
  style: Option<Style>,
  cull: Option<CanvasViewport>,
) -> (Vec<u8>, Placement) {
  rasterize_mask(paths, transform, style.unwrap_or_default(), cull).unwrap_or_default()
}

/// `[left, top, right, bottom]` cut to `cull` once they cover too many pixels to rasterize whole,
/// or `None` when they lie wholly outside it.
pub(crate) fn cull_bounds(
  [mut left, mut top, mut right, mut bottom]: [i32; 4],
  cull: Option<CanvasViewport>,
) -> Option<Placement> {
  // Cutting a mask that overlaps the cull rect is a protection layer, not an
  // optimization: it moves the buffer origin, which shifts anti-aliasing by a
  // float-rounding hair, so it only engages once the full mask is too large to
  // be worth rasterizing.
  const CULL_THRESHOLD_PIXELS: u64 = 1 << 24;
  // The halo keeps the rasterizer's clip edge away from the visible pixels:
  // clipping a contour exactly on the cull edge shifts the anti-aliasing of
  // the boundary row.
  const CULL_HALO: i32 = 8;

  let full_pixels = (right.saturating_sub(left).max(0) as u64)
    .saturating_mul(bottom.saturating_sub(top).max(0) as u64);

  if let Some(cull) = cull {
    if right <= cull.origin.x as i32
      || bottom <= cull.origin.y as i32
      || left >= cull.right()
      || top >= cull.bottom()
    {
      return None;
    }

    if full_pixels > CULL_THRESHOLD_PIXELS {
      left = left.max((cull.origin.x as i32).saturating_sub(CULL_HALO));
      top = top.max((cull.origin.y as i32).saturating_sub(CULL_HALO));
      right = right.min(cull.right().saturating_add(CULL_HALO));
      bottom = bottom.min(cull.bottom().saturating_add(CULL_HALO));
    }
  }

  Placement::from_bounds(left, top, right, bottom)
}

fn rasterize_mask(
  paths: &[Command],
  transform: Option<Affine>,
  style: Style,
  cull: Option<CanvasViewport>,
) -> Option<(Vec<u8>, Placement)> {
  let mut path = build_path(paths)?;

  if let Some(stroke) = style.stroke() {
    if let Some(dash) = &stroke.dash
      && let Some(dashed_path) = path.dash(dash, 1.0)
    {
      path = dashed_path;
    }

    path = path.stroke(&stroke, 1.0)?;
  }

  if let Some(transform) = transform {
    path = path.transform(transform.into())?;
  }

  let bounds = path.compute_tight_bounds()?;
  let Placement {
    left,
    top,
    width,
    height,
  } = cull_bounds(
    [
      bounds.left().floor() as i32,
      bounds.top().floor() as i32,
      bounds.right().ceil() as i32,
      bounds.bottom().ceil() as i32,
    ],
    cull,
  )?;
  let size = IntSize::from_wh(width, height)?;
  let buffer = vec![0; checked_area(width, height, 1)?];
  let mut mask = TinyMask::from_vec(buffer, size)?;
  let local_path = path.transform(TinyTransform::from_translate(-(left as f32), -(top as f32)))?;
  mask.fill_path(
    &local_path,
    style.fill_rule(),
    true,
    TinyTransform::identity(),
  );

  Some((
    mask.take(),
    Placement {
      left,
      top,
      width,
      height,
    },
  ))
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

  /// Resolves the combined constraint mask against the pixmap it clips: borrowed
  /// directly when the stored mask already matches the canvas viewport, cropped
  /// into a scratch buffer otherwise.
  pub(crate) fn resolve(self, size: Size<u32>) -> Option<Cow<'a, TinyMask>> {
    if self.origin == self.canvas_origin
      && self.mask.width() == size.width
      && self.mask.height() == size.height
    {
      return Some(Cow::Borrowed(self.mask));
    }

    self.materialize(size).map(Cow::Owned)
  }

  fn materialize(self, size: Size<u32>) -> Option<TinyMask> {
    let mut cropped = TinyMask::from_vec(
      vec![0; (size.width as usize) * (size.height as usize)],
      IntSize::from_wh(size.width, size.height)?,
    )?;

    let offset = Point {
      x: self.canvas_origin.x as i32 - self.origin.x as i32,
      y: self.canvas_origin.y as i32 - self.origin.y as i32,
    };
    let src_width = self.mask.width() as i32;
    let src_height = self.mask.height() as i32;
    let start_x = offset.x.max(0);
    let start_y = offset.y.max(0);
    let end_x = (offset.x + size.width as i32).min(src_width);
    let end_y = (offset.y + size.height as i32).min(src_height);
    if start_x >= end_x || start_y >= end_y {
      return Some(cropped);
    }

    let src = self.mask.data();
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

#[cfg(test)]
mod tests {
  use super::*;

  fn attenuate_reference(alpha: u8, mask_alpha: u8) -> u8 {
    if mask_alpha == 0 {
      return alpha;
    }

    fast_div_255(alpha as u32 * (255 - mask_alpha as u32))
  }

  #[test]
  fn attenuate_matches_the_branching_formula_for_every_pair() {
    let placement = Placement {
      left: 0,
      top: 0,
      width: 256,
      height: 256,
    };

    let mut dst: Vec<u8> = (0..256 * 256).map(|i| (i / 256) as u8).collect();
    let mask: Vec<u8> = (0..256 * 256).map(|i| (i % 256) as u8).collect();
    attenuate_alpha_by_mask(&mut dst, placement, &mask, placement);

    for (i, &alpha) in dst.iter().enumerate() {
      assert_eq!(
        alpha,
        attenuate_reference((i / 256) as u8, (i % 256) as u8),
        "index {i}"
      );
    }
  }

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

  #[test]
  fn a_mask_outside_the_cull_rect_is_skipped() {
    let cull = CanvasViewport {
      origin: Point { x: 100, y: 100 },
      size: Size {
        width: 50,
        height: 50,
      },
    };

    assert_eq!(cull_bounds([0, 0, 100, 200], Some(cull)), None);
    assert_eq!(cull_bounds([150, 120, 160, 130], Some(cull)), None);
    assert_eq!(
      cull_bounds([99, 99, 101, 101], Some(cull)),
      Placement::from_bounds(99, 99, 101, 101)
    );
    assert_eq!(
      cull_bounds([0, 0, 100, 200], None),
      Placement::from_bounds(0, 0, 100, 200)
    );
  }
}
