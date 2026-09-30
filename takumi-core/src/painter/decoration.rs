//! Text decoration lines, painted after Blink's `DecorationLinePainter` in
//! [`decoration_line_painter.cc`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/paint/decoration_line_painter.cc)
//! and the offsets `TextDecorationInfo::ComputeLineData` gives a double or wavy line. Follows
//! Blink under the notice in LICENSE-CHROMIUM.
//!
//! Approximate: the lines start from their snapped rectangle, where Blink offsets a double or
//! wavy line by the unsnapped thickness and places a wave on the unsnapped top, and `skip-ink`
//! cuts every style where glyphs cross the straight line, where Blink cuts the band each style
//! fills.

use super::{FillShape, PaintDevice, StrokeStyle, border::StyledLine};
use crate::{
  geometry::{PathCommand, Point, Size},
  layout::inline::DecorationRect,
  style::{Affine, BorderStyle, FillRule, TextDecorationLines, TextDecorationStyle},
};

/// A wave's shape: one cubic Bezier per wavelength, its control points
/// `control_point_distance` off the midline.
struct Wave {
  wavelength: f32,
  control_point_distance: f32,
}

impl Wave {
  /// The wave Blink's `MakeWave` draws for a `thickness` line, its steps on half pixels for
  /// cleaner antialiasing.
  fn of(thickness: f32) -> Self {
    let thickness = thickness.max(1.0);

    Self {
      wavelength: 1.0 + 2.0 * (2.0 * thickness + 0.5).round(),
      control_point_distance: 0.5 + (3.0 * thickness + 0.5).round(),
    }
  }

  /// How far the centerline swings from its midline, at the cubic's extreme.
  fn amplitude(&self) -> f32 {
    self.control_point_distance / (2.0 * 3.0_f32.sqrt())
  }

  /// The centerline from one wavelength before `start` to one past `start + width`, its
  /// midpoints at `start.y`.
  fn centerline(&self, start: Point<f32>, width: f32) -> Vec<PathCommand> {
    let end = start.x + width + self.wavelength;
    let half = self.wavelength / 2.0;
    let mut x = start.x - self.wavelength;
    let mut commands = vec![PathCommand::MoveTo(Point { x, y: start.y })];

    while x < end {
      commands.push(PathCommand::CubicTo(
        Point {
          x: x + half,
          y: start.y + self.control_point_distance,
        },
        Point {
          x: x + half,
          y: start.y - self.control_point_distance,
        },
        Point {
          x: x + self.wavelength,
          y: start.y,
        },
      ));
      x += self.wavelength;
    }
    commands
  }
}

impl DecorationRect {
  /// Paints the line with its border box at `origin`.
  pub fn paint<D: PaintDevice>(&self, origin: Point<f32>, device: &mut D) {
    if self.color.0[3] == 0 || self.width <= 0.0 || self.height <= 0.0 {
      return;
    }

    let [a, b, c, d, e, f] = self.transform;
    let at = Affine {
      a,
      b,
      c,
      d,
      x: e + origin.x,
      y: f + origin.y,
    };

    match self.style {
      TextDecorationStyle::Solid => self.fill(at, device),
      TextDecorationStyle::Double => {
        self.fill(at, device);
        self.fill(Affine::translation(0.0, self.double_offset()) * at, device);
      }
      TextDecorationStyle::Dotted | TextDecorationStyle::Dashed => {
        let style = if self.style == TextDecorationStyle::Dotted {
          BorderStyle::Dotted
        } else {
          BorderStyle::Dashed
        };
        let y = self.height / 2.0;
        let (start, end) = self.line_span;
        let line = StyledLine::new(
          Point { x: start, y },
          Point { x: end, y },
          self.height,
          style,
          self.color,
        );

        if self.line_span == (0.0, self.width) {
          line.paint(at, device);
        } else {
          self.clip(0.0, self.height, at, device, |device| {
            line.paint(at, device)
          });
        }
      }
      TextDecorationStyle::Wavy => {
        let wave = Wave::of(self.height);
        let midline = self.wavy_offset() + 0.5;
        let reach = wave.amplitude() + self.height / 2.0;
        let (start, end) = self.line_span;
        let top = (midline - reach).floor();
        let bottom = (midline + reach).ceil();

        self.clip(top, bottom, at, device, |device| {
          device.stroke_shape(
            &FillShape::Path {
              commands: wave.centerline(
                Point {
                  x: start,
                  y: midline,
                },
                end - start,
              ),
              rule: FillRule::NonZero,
            },
            &StrokeStyle {
              color: self.color,
              width: self.height,
              dash: None,
              round_cap: false,
            },
            at,
          );
        });
      }
    }
  }

  /// Fills the rectangle under `at`.
  fn fill<D: PaintDevice>(&self, at: Affine, device: &mut D) {
    device.fill_shape(
      &FillShape::Rect(Size {
        width: self.width,
        height: self.height,
      }),
      self.color,
      at,
    );
  }

  /// Runs `paint` clipped to this rect's stretch of the line, from `top` to `bottom`.
  fn clip<D: PaintDevice>(
    &self,
    top: f32,
    bottom: f32,
    at: Affine,
    device: &mut D,
    paint: impl FnOnce(&mut D),
  ) {
    device.push_clip(
      &FillShape::Rect(Size {
        width: self.width,
        height: bottom - top,
      }),
      Affine::translation(0.0, top) * at,
    );
    paint(device);
    device.pop_clip();
  }

  /// How far a double line's second line sits from the first: below an underline, above an
  /// overline, and below a line-through, a whole number of pixels there.
  fn double_offset(&self) -> f32 {
    let offset = self.height + 1.0;

    match self.line {
      TextDecorationLines::OVERLINE => -offset,
      TextDecorationLines::LINE_THROUGH => offset.floor(),
      _ => offset,
    }
  }

  /// How far a wavy line sits from the straight one: below an underline, above an overline,
  /// and on a line-through.
  fn wavy_offset(&self) -> f32 {
    let offset = self.height + 1.0;

    match self.line {
      TextDecorationLines::OVERLINE => -offset,
      TextDecorationLines::LINE_THROUGH => 0.0,
      _ => offset,
    }
  }
}
