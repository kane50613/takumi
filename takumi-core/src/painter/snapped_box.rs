//! A border box pixel-snapped where it sits in the space paint snaps in, as Blink's box painters
//! take `ToPixelSnappedRect` of it before painting decorations.

use crate::{
  geometry::{Point, Rect, Size},
  layout_unit::{
    BoxStrut, LayoutUnit, UnitOffset, UnitRect, UnitSize, snap_size_to_pixel_allowing_zero,
  },
};

/// A border box and its pixel-snapped rectangle.
#[derive(Debug, Clone, Copy)]
pub struct SnappedBox {
  paint_offset: Point<f32>,
  unsnapped: UnitRect,
  snapped: UnitRect,
}

impl SnappedBox {
  /// The border box of `size` at `paint_offset`.
  pub fn new(paint_offset: Point<f32>, size: Size<f32>) -> Self {
    let unsnapped = UnitRect::nearest(paint_offset, size);

    Self {
      paint_offset,
      unsnapped,
      snapped: unsnapped.pixel_snapped(),
    }
  }

  /// The snapped box's top-left, relative to the border box.
  pub fn offset(&self) -> Point<f32> {
    self.relative(self.snapped.offset)
  }

  /// The snapped box's size.
  pub fn size(&self) -> Size<f32> {
    self.snapped.size.to_size()
  }

  /// Blink's `PixelSnappedContouredBorderWithOutsets` rectangle `insets` in from the snapped box
  /// when `of_snapped`, else from the border box, relative to the snapped box.
  pub fn contoured_inset(&self, insets: Rect<f32>, of_snapped: bool) -> (Point<f32>, Size<f32>) {
    let from = if of_snapped {
      self.snapped
    } else {
      self.unsnapped
    };
    let rect_with_outsets = from.contract(strut(insets)).clamp_negative_size_to_zero();

    self.relative_rect(UnitRect {
      offset: UnitOffset {
        left: LayoutUnit::from_int(rect_with_outsets.x().round()),
        top: LayoutUnit::from_int(rect_with_outsets.y().round()),
      },
      size: UnitSize {
        width: LayoutUnit::from_int(snap_size_to_pixel_allowing_zero(
          rect_with_outsets.width(),
          rect_with_outsets.x(),
        )),
        height: LayoutUnit::from_int(snap_size_to_pixel_allowing_zero(
          rect_with_outsets.height(),
          rect_with_outsets.y(),
        )),
      },
    })
  }

  /// Blink's `ToPixelSnappedRect` of the border box `insets` in, relative to the snapped box.
  pub fn inset(&self, insets: Rect<f32>) -> (Point<f32>, Size<f32>) {
    self.relative_rect(self.unsnapped.contract(strut(insets)).pixel_snapped())
  }

  /// Blink's `ToPixelSnappedRect` of a rectangle `offset` from the border box, relative to the
  /// snapped box.
  pub fn snap(&self, offset: Point<f32>, size: Size<f32>) -> (Point<f32>, Size<f32>) {
    self.relative_rect(UnitRect::nearest(self.paint_offset + offset, size).pixel_snapped())
  }

  /// `rect`, in paint-offset space, relative to the snapped box.
  fn relative_rect(&self, rect: UnitRect) -> (Point<f32>, Size<f32>) {
    (
      (rect.offset - self.snapped.offset).to_point(),
      rect.size.to_size(),
    )
  }

  /// `offset` in paint-offset space, relative to the border box.
  fn relative(&self, offset: UnitOffset) -> Point<f32> {
    Point {
      x: offset.left.to_f32() - self.paint_offset.x,
      y: offset.top.to_f32() - self.paint_offset.y,
    }
  }
}

/// `insets` in the nearest layout units.
fn strut(insets: Rect<f32>) -> BoxStrut {
  BoxStrut {
    top: LayoutUnit::from_f32_round(insets.top),
    right: LayoutUnit::from_f32_round(insets.right),
    bottom: LayoutUnit::from_f32_round(insets.bottom),
    left: LayoutUnit::from_f32_round(insets.left),
  }
}
