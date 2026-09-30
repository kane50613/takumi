//! Text decoration lines, painted after Blink's `DecorationLinePainter` in
//! [`decoration_line_painter.cc`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/paint/decoration_line_painter.cc),
//! the offsets `TextDecorationInfo::ComputeLineData` gives a double or wavy line, and the cuts
//! `TextPainter::ClipDecorationLine` makes for `text-decoration-skip-ink`. Follows Blink under the
//! notice in LICENSE-CHROMIUM.
//!
//! A line snaps to whole pixels of the block's border box, which is Blink's transform-node space
//! since layout places every box on whole pixels.

use super::{FillShape, PaintDevice, StrokeStyle, border::StyledLine};
use crate::{
  geometry::{PathCommand, Point, Rect, Size},
  layout::{
    inline::DecorationLine,
    intercept::{Spans, remaining_spans},
  },
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

/// A dashed or dotted line where Blink's `DrawLineAsStroke` draws it: from and to whole pixels, on
/// a pixel row, a whole number of pixels thick.
struct DashedLine {
  start: f32,
  end: f32,
  /// The centre of the pixel row, before an odd thickness moves the stroke onto the half pixel.
  row: f32,
  thickness: f32,
}

impl DecorationLine {
  /// Paints the line, cut where `text-decoration-skip-ink` gives way to the glyphs.
  pub fn paint<D: PaintDevice>(&self, device: &mut D) {
    if self.color.0[3] == 0 || self.width <= 0.0 {
      return;
    }

    let bounds = self.bounds();
    let pieces = self.pieces(bounds);

    match self.style {
      TextDecorationStyle::Solid => self.fill(&pieces, 0.0, device),
      TextDecorationStyle::Double => {
        self.fill(&pieces, 0.0, device);
        self.fill(&pieces, self.double_offset(), device);
      }
      TextDecorationStyle::Dotted | TextDecorationStyle::Dashed => {
        let style = if self.style == TextDecorationStyle::Dotted {
          BorderStyle::Dotted
        } else {
          BorderStyle::Dashed
        };
        let dashed = self.dashed();
        let y = if dashed.thickness % 2.0 == 1.0 {
          dashed.row + 0.5
        } else {
          dashed.row
        };
        let line = StyledLine::new(
          Point { x: dashed.start, y },
          Point { x: dashed.end, y },
          dashed.thickness,
          style,
          self.color,
        );

        if self.skips.is_empty() {
          line.paint(self.transform, device);
        } else {
          // Blink's clip-out rects reach a pixel past the band, so they cover the half-pixel shift.
          self.clip(
            &pieces,
            bounds.top - 1.0,
            bounds.bottom + 1.0,
            device,
            |device| line.paint(self.transform, device),
          );
        }
      }
      TextDecorationStyle::Wavy => {
        let wave = Wave::of(self.thickness);

        self.clip(&pieces, bounds.top, bounds.bottom, device, |device| {
          device.stroke_shape(
            &FillShape::Path {
              commands: wave.centerline(
                Point {
                  x: self.origin.x,
                  y: self.origin.y + self.wavy_offset() + 0.5,
                },
                self.width,
              ),
              rule: FillRule::NonZero,
            },
            &StrokeStyle {
              color: self.color,
              width: self.thickness,
              dash: None,
              round_cap: false,
            },
            self.transform,
          );
        });
      }
    }
  }

  /// The area the line paints, which `skip-ink` looks for glyphs in, after Blink's
  /// `DecorationLinePainter::Bounds`.
  pub(crate) fn bounds(&self) -> Rect<f32> {
    let Point { x, y } = self.origin;
    let right = x + self.width;

    match self.style {
      TextDecorationStyle::Solid => Rect {
        left: x,
        top: y,
        right,
        bottom: y + self.thickness,
      },
      TextDecorationStyle::Double => {
        let offset = self.double_offset();

        Rect {
          left: x,
          top: y + offset.min(0.0),
          right,
          bottom: y + self.thickness + offset.max(0.0),
        }
      }
      TextDecorationStyle::Dotted | TextDecorationStyle::Dashed => {
        let dashed = self.dashed();

        Rect {
          left: dashed.start,
          top: dashed.row - dashed.thickness / 2.0,
          right: dashed.end,
          bottom: dashed.row + dashed.thickness / 2.0,
        }
      }
      TextDecorationStyle::Wavy => {
        let wave = Wave::of(self.thickness);
        let reach = wave.amplitude() + self.thickness / 2.0;
        let midline = y + self.wavy_offset();

        Rect {
          left: x,
          top: midline + (0.5 - reach).floor(),
          right,
          bottom: midline + (0.5 + reach).ceil(),
        }
      }
    }
  }

  /// The stretches of `bounds` left once `skip-ink` cuts the glyphs out, each cut on a whole
  /// device pixel as Blink's unantialiased `ClipOut` makes it.
  fn pieces(&self, bounds: Rect<f32>) -> Spans {
    let cuts: Spans = self
      .skips
      .iter()
      .map(|&(start, end)| (self.device_pixel_x(start), self.device_pixel_x(end)))
      .collect();

    remaining_spans(bounds.left, bounds.right, &cuts)
  }

  /// `x` moved to the device pixel edge Skia rounds an unantialiased clip to. Under a rotation or
  /// skew the clip stays a path whose edges Skia does not move.
  fn device_pixel_x(&self, x: f32) -> f32 {
    let Affine { a, b, c, x: dx, .. } = self.output;

    if b != 0.0 || c != 0.0 || a == 0.0 {
      return x;
    }
    ((a * x + dx + 0.5).floor() - dx) / a
  }

  /// Fills the pieces of the line `offset` below its top, the top on a whole pixel and the
  /// thickness rounded down to one, as Blink's `DrawLineAsRect` snaps it.
  fn fill<D: PaintDevice>(&self, pieces: &[(f32, f32)], offset: f32, device: &mut D) {
    let top = (self.origin.y + offset + 0.5).floor();
    let height = self.thickness.floor().max(1.0);

    for &(start, end) in pieces {
      device.fill_shape(
        &FillShape::Rect(Size {
          width: end - start,
          height,
        }),
        self.color,
        self.transform * Affine::translation(start, top),
      );
    }
  }

  /// Runs `paint` clipped to the pieces of the line, from `top` to `bottom`.
  fn clip<D: PaintDevice>(
    &self,
    pieces: &[(f32, f32)],
    top: f32,
    bottom: f32,
    device: &mut D,
    paint: impl FnOnce(&mut D),
  ) {
    if pieces.is_empty() {
      return;
    }

    let commands = pieces
      .iter()
      .flat_map(|&(start, end)| {
        [
          PathCommand::MoveTo(Point { x: start, y: top }),
          PathCommand::LineTo(Point { x: end, y: top }),
          PathCommand::LineTo(Point { x: end, y: bottom }),
          PathCommand::LineTo(Point {
            x: start,
            y: bottom,
          }),
          PathCommand::Close,
        ]
      })
      .collect();

    device.push_clip(
      &FillShape::Path {
        commands,
        rule: FillRule::NonZero,
      },
      self.transform,
    );
    paint(device);
    device.pop_clip();
  }

  /// Where a dashed or dotted line runs: `GetSnappedPointsForTextLine` truncates its ends and
  /// floors its middle to whole pixels, and `DrawLineAsStroke` rounds its thickness.
  fn dashed(&self) -> DashedLine {
    DashedLine {
      start: self.origin.x.floor(),
      end: (self.origin.x + self.width).floor(),
      row: (self.origin.y + (self.thickness / 2.0).max(0.5)).floor(),
      thickness: self.thickness.round().max(1.0),
    }
  }

  /// How far a double line's second line sits from the first: below an underline, above an
  /// overline, and below a line-through, a whole number of pixels there.
  fn double_offset(&self) -> f32 {
    let offset = self.thickness + 1.0;

    match self.line {
      TextDecorationLines::OVERLINE => -offset,
      TextDecorationLines::LINE_THROUGH => offset.floor(),
      _ => offset,
    }
  }

  /// How far a wavy line sits from the straight one: below an underline, above an overline,
  /// and on a line-through.
  fn wavy_offset(&self) -> f32 {
    let offset = self.thickness + 1.0;

    match self.line {
      TextDecorationLines::OVERLINE => -offset,
      TextDecorationLines::LINE_THROUGH => 0.0,
      _ => offset,
    }
  }
}
