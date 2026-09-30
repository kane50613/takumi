//! A box's `outline`, painted after Blink's
//! [`OutlinePainter`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/paint/outline_painter.cc).

use std::cmp::Ordering;

use super::{BoxBorderPainter, BoxPainter, FillShape, PaintDevice, PaintRole, border::StyledLine};
use crate::{
  geometry::{PathBuilder, Point, Size},
  layout::{
    border::{BorderProperties, BorderSide},
    decoration::OutlineGeometry,
    inline::{OutlineIsland, ProcessedInlineSpan},
  },
  style::{Affine, BorderStyle, Color, FillRule, Sides},
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

    device.set_role(PaintRole::Outline);
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
  /// Paints the outline of the span that owns the island, with the block's border box at
  /// `origin`: a lone rect as a box border, as Blink's `PaintSingleRectOutline` does, and
  /// anything else after Blink's `ComplexOutlinePainter`.
  ///
  /// Approximate: the contour keeps square corners, where Blink rounds them when the element has
  /// a `border-radius`, and the inner edge follows the rects grown by `outline-offset`, where Blink
  /// shrinks the outer contour. Follows Blink under the notice in LICENSE-CHROMIUM.
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
    let width = style.outline_width;
    let opacity = style.parent.opacity.0;
    let mut color = style.outline_color;

    if width <= 0.0
      || !style.outline_style.is_rendered()
      || color.0[3] == 0
      || opacity <= 0.0
      || !style.parent.is_visible()
    {
      return;
    }

    if let Some(rect) = self.lone_rect() {
      let outline = OutlineGeometry::ring(
        Size {
          width: rect.width,
          height: rect.height,
        },
        style.outline_offset,
        BorderProperties {
          width: Sides([width; 4]).into(),
          color: Sides([color; 4]).into(),
          style: Sides([style.outline_style; 4]).into(),
          ..BorderProperties::default()
        },
      );
      let origin = Point {
        x: origin.x + rect.x,
        y: origin.y + rect.y,
      };

      return device.with_opacity(opacity, |device| {
        PendingOutline { outline, origin }.paint(device)
      });
    }

    // Blink draws thin double, groove and ridge outlines solid.
    let outline_style = match style.outline_style {
      BorderStyle::Double if width <= 2.0 => BorderStyle::Solid,
      BorderStyle::Groove | BorderStyle::Ridge if width <= 1.0 => BorderStyle::Solid,
      outline_style => outline_style,
    };
    let inner = style.outline_offset;
    let outer = inner + width;
    let at = Affine::translation(origin.x, origin.y);
    // Dashes overlap at the corners, so a translucent colour paints opaque into a layer that
    // takes its alpha.
    let alpha_layer =
      color.0[3] < u8::MAX && !matches!(outline_style, BorderStyle::Solid | BorderStyle::Double);

    device.set_role(PaintRole::Outline);
    device.with_opacity(opacity, |device| {
      if alpha_layer {
        device.begin_layer(f32::from(color.0[3]) / f32::from(u8::MAX));
        color.0[3] = u8::MAX;
      }

      match outline_style {
        BorderStyle::Double => {
          let third = width / 3.0;

          device.fill_shape(&self.ring(outer - third, outer), color, at);
          device.fill_shape(&self.ring(inner, inner + third), color, at);
        }
        BorderStyle::Dashed | BorderStyle::Dotted => {
          device.push_clip(&self.ring(inner, outer), at);

          let corners = self.corners(inner + width / 2.0);
          let half = width / 2.0;

          for (&start, &end) in corners.iter().zip(corners.iter().cycle().skip(1)) {
            let [start, end] = if start.x > end.x || start.y > end.y {
              [end, start]
            } else {
              [start, end]
            };

            if start == end {
              continue;
            }

            let horizontal = start.y == end.y;
            let reach = |point: Point<f32>, toward: f32| Point {
              x: point.x + if horizontal { toward } else { 0.0 },
              y: point.y + if horizontal { 0.0 } else { toward },
            };

            StyledLine::new(
              reach(start, -half),
              reach(end, half),
              width,
              outline_style,
              color,
            )
            .paint(at, device);
          }

          device.pop_clip();
        }
        BorderStyle::Inset | BorderStyle::Outset => {
          self.paint_shaded_band(inner, outer, outline_style, color, at, device);
        }
        BorderStyle::Groove | BorderStyle::Ridge => {
          let (outer_half, inner_half) = if outline_style == BorderStyle::Groove {
            (BorderStyle::Inset, BorderStyle::Outset)
          } else {
            (BorderStyle::Outset, BorderStyle::Inset)
          };
          let middle = inner + width / 2.0;

          self.paint_shaded_band(middle, outer, outer_half, color, at, device);
          self.paint_shaded_band(inner, middle, inner_half, color, at, device);
        }
        _ => device.fill_shape(&self.ring(inner, outer), color, at),
      }

      if alpha_layer {
        device.end_layer();
      }
    });
  }

  /// The band between the contours `inner` and `outer` past the island's rects.
  fn ring(&self, inner: f32, outer: f32) -> FillShape {
    let mut commands = self.contour(outer);

    commands.extend(self.contour(inner));

    FillShape::Path {
      commands,
      rule: FillRule::EvenOdd,
    }
  }

  /// Fills the band between the contours `inner` and `outer` edge by edge, each edge mitred at its
  /// corners and darkened on the sides `style` shades, as Blink's `PaintInsetOrOutsetOutline` does.
  fn paint_shaded_band<D: PaintDevice>(
    &self,
    inner: f32,
    outer: f32,
    style: BorderStyle,
    color: Color,
    at: Affine,
    device: &mut D,
  ) {
    let outer_corners = self.corners(outer);
    let inner_corners = self.corners(inner);

    if outer_corners.len() != inner_corners.len() {
      return device.fill_shape(&self.ring(inner, outer), color, at);
    }

    let count = outer_corners.len();

    for index in 0..count {
      let next = (index + 1) % count;
      let [start, end] = [outer_corners[index], outer_corners[next]];
      let side = match (end.x.total_cmp(&start.x), end.y.total_cmp(&start.y)) {
        (Ordering::Greater, _) => BorderSide::Top,
        (Ordering::Less, _) => BorderSide::Bottom,
        (_, Ordering::Greater) => BorderSide::Right,
        (_, Ordering::Less) => BorderSide::Left,
        _ => continue,
      };
      let shaded = if side.darkened_by(style) {
        color.dark()
      } else {
        color
      };
      let mut commands = Vec::with_capacity(5);

      commands.move_to((start.x, start.y));
      commands.line_to((end.x, end.y));
      commands.line_to((inner_corners[next].x, inner_corners[next].y));
      commands.line_to((inner_corners[index].x, inner_corners[index].y));
      commands.close();

      device.fill_shape(
        &FillShape::Path {
          commands,
          rule: FillRule::NonZero,
        },
        shaded,
        at,
      );
    }
  }
}
