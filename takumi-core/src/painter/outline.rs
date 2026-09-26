//! A box's `outline`, painted after Blink's
//! [`OutlinePainter`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/paint/outline_painter.cc).

use super::{BoxBorderPainter, BoxPainter, FillShape, PaintDevice};
use crate::{
  geometry::Point,
  layout::{
    decoration::OutlineGeometry,
    inline::{OutlineIsland, ProcessedInlineSpan},
  },
  style::{Affine, FillRule},
};

/// A box's outline, held until the box's content has painted, as CSS 2 Appendix E orders them.
#[derive(Debug, Clone, Copy)]
pub struct PendingOutline {
  outline: OutlineGeometry,
  origin: Point<f32>,
}

impl PendingOutline {
  /// Paints the outline as a ring around the border box, grown by
  /// `outline-offset + outline-width`.
  pub fn paint<D: PaintDevice>(&self, device: &mut D) {
    let OutlineGeometry { border, size, grow } = self.outline;

    BoxBorderPainter::new(&border, size).paint(
      Point {
        x: self.origin.x - grow,
        y: self.origin.y - grow,
      },
      device,
    );
  }
}

impl BoxPainter<'_> {
  /// The outline of the box with its border box at `origin`, or `None` when it paints none.
  pub fn pending_outline(&self, origin: Point<f32>) -> Option<PendingOutline> {
    Some(PendingOutline {
      outline: self.outline()?,
      origin,
    })
  }
}

impl OutlineIsland {
  /// Strokes the outline of the span that owns the island, with the block's border box at
  /// `origin`.
  pub fn paint<D: PaintDevice>(
    &self,
    spans: &[ProcessedInlineSpan<'_>],
    origin: Point<f32>,
    device: &mut D,
  ) {
    let Some(ProcessedInlineSpan::Text { style, .. }) = self
      .span_id()
      .and_then(|span_id| spans.get(span_id as usize))
    else {
      return;
    };
    let Some(stroke) = style
      .outline_stroke()
      .filter(|stroke| stroke.color.0[3] != 0)
    else {
      return;
    };
    let opacity = style.parent.opacity.0;
    let contour = self.contour(style.outline_offset + stroke.width / 2.0);

    if contour.is_empty() || opacity <= 0.0 {
      return;
    }

    let layered = opacity < 1.0;

    if layered {
      device.begin_layer(opacity);
    }

    device.stroke_shape(
      &FillShape::Path {
        commands: contour,
        rule: FillRule::NonZero,
      },
      &stroke,
      Affine::translation(origin.x, origin.y),
    );

    if layered {
      device.end_layer();
    }
  }
}
