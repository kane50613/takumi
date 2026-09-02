//! The seam between deciding what to paint and painting it.

use crate::{
  context::RenderContext,
  geometry::{ComputedLayout, PathCommand, Point, Rect, Size},
  layout::{
    border::{BorderPaint, BorderProperties},
    decoration::{ClipBox, OutlineGeometry, outline_paint},
    inline::DecorationRect,
  },
  shadow::SizedShadow,
  style::{Affine, BackgroundClip, BoxShadow, Color, FillRule, Sides, TextDecorationLines},
};

/// A closed shape to fill, in the coordinate space of the box that owns it.
pub enum FillShape {
  /// An axis-aligned rectangle at the box origin.
  Rect(Size<f32>),
  /// A rectangle whose corners come from `border`.
  RoundedRect {
    /// The corner geometry.
    border: BorderProperties,
    /// The rectangle's size.
    size: Size<f32>,
    /// Where the rectangle sits inside the box.
    offset: Point<f32>,
  },
  /// Anything else.
  Path {
    /// The path.
    commands: Vec<PathCommand>,
    /// How to decide what lies inside the path.
    rule: FillRule,
  },
}

impl FillShape {
  /// The shape as path commands, for a backend that only draws paths.
  pub fn to_commands(&self) -> Vec<PathCommand> {
    let mut commands = Vec::with_capacity(BorderProperties::PATH_COMMANDS_AMOUNT * 2);

    match self {
      Self::Rect(size) => {
        BorderProperties::default().append_mask_commands(&mut commands, *size, Point::ZERO);
      }
      Self::RoundedRect {
        border,
        size,
        offset,
      } => border.append_mask_commands(&mut commands, *size, *offset),
      Self::Path { commands: path, .. } => commands.extend_from_slice(path),
    }
    commands
  }

  /// How to decide what lies inside the shape.
  pub fn rule(&self) -> FillRule {
    match self {
      Self::Path { rule, .. } => *rule,
      _ => FillRule::NonZero,
    }
  }
}

/// What a backend has to be able to do for the shared painting code to drive it.
pub trait PaintDevice {
  /// Fills `shape` under `transform`, with a single colour.
  fn fill_shape(&mut self, shape: &FillShape, color: Color, transform: Affine);

  /// Strokes `shape` under `transform`.
  fn stroke_shape(&mut self, _shape: &FillShape, _stroke: &StrokeStyle, _transform: Affine) {}
}

/// How to stroke a shape.
pub struct StrokeStyle {
  /// The stroke colour.
  pub color: Color,
  /// The stroke width.
  pub width: f32,
  /// Dash and gap lengths, when the stroke is dashed or dotted.
  pub dash: Option<[f32; 2]>,
  /// Whether the dashes have round caps, which is how `dotted` draws.
  pub round_cap: bool,
}

/// A box's `box-shadow` layers, split by where they fall.
#[derive(Default, Clone)]
pub struct BoxShadows {
  /// Shadows inside the box.
  pub inset: Vec<SizedShadow>,
  /// Shadows outside it.
  pub outer: Vec<SizedShadow>,
}

/// Everything a backend needs to paint one box, decided once.
pub struct BoxPainter<'c> {
  context: &'c RenderContext,
  layout: ComputedLayout,
  border: BorderProperties,
}

impl<'c> BoxPainter<'c> {
  /// Prepares the box at `layout` for painting.
  pub fn new(context: &'c RenderContext, layout: ComputedLayout) -> Self {
    Self {
      context,
      layout,
      border: BorderProperties::from_context(context, layout.size, layout.border),
    }
  }

  /// Prepares a fragment of the box that paints its own decorations, which is what
  /// `box-decoration-break: clone` asks for.
  pub fn fragment(context: &'c RenderContext, layout: ComputedLayout, size: Size<f32>) -> Self {
    Self::new(context, ComputedLayout { size, ..layout })
  }

  /// The box's border geometry, corners included.
  pub fn border(&self) -> &BorderProperties {
    &self.border
  }

  /// The box a background paints into, per `background-clip`.
  pub fn background_clip_shape(&self) -> Option<FillShape> {
    background_clip_shape(
      self.context.style.background_clip,
      &self.border,
      self.layout,
    )
  }

  /// Paints `background-color`.
  pub fn background_color<D: PaintDevice>(&self, origin: Point<f32>, device: &mut D) {
    let color = self
      .context
      .style
      .background_color
      .resolve(self.context.current_color);

    if color.0[3] == 0 {
      return;
    }
    let Some(shape) = self.background_clip_shape() else {
      return;
    };

    device.fill_shape(&shape, color, Affine::translation(origin.x, origin.y));
  }

  /// The box's `box-shadow` layers, resolved and split into the ones that fall inside the box and
  /// the ones outside it.
  pub fn shadows(&self) -> BoxShadows {
    let Some(shadows) = self.context.style.box_shadow.as_deref() else {
      return BoxShadows::default();
    };
    let resolve = |shadow: &BoxShadow| {
      SizedShadow::from_box_shadow(
        *shadow,
        &self.context.sizing,
        self.context.current_color,
        self.layout.size,
      )
    };
    let visible = |shadow: &SizedShadow| shadow.color.0[3] != 0;

    BoxShadows {
      inset: shadows
        .iter()
        .filter(|shadow| shadow.inset)
        .map(resolve)
        .filter(visible)
        .collect(),
      outer: shadows
        .iter()
        .filter(|shadow| !shadow.inset)
        .map(resolve)
        .filter(visible)
        .collect(),
    }
  }

  /// The outline the box paints, or `None` when it paints none.
  pub fn outline(&self) -> Option<OutlineGeometry> {
    outline_paint(self.context, self.layout.size)
  }
}

fn background_clip_shape(
  clip: BackgroundClip,
  border: &BorderProperties,
  layout: ComputedLayout,
) -> Option<FillShape> {
  if layout.size.width <= 0.0 || layout.size.height <= 0.0 {
    return None;
  }
  let mut commands = Vec::with_capacity(BorderProperties::PATH_COMMANDS_AMOUNT * 2);

  match clip {
    BackgroundClip::BorderBox if border.is_zero() => Some(FillShape::Rect(layout.size)),
    BackgroundClip::BorderBox => Some(FillShape::RoundedRect {
      border: *border,
      size: layout.size,
      offset: Point::ZERO,
    }),
    BackgroundClip::PaddingBox => {
      let clip = ClipBox::padding_box(*border, layout);

      Some(FillShape::RoundedRect {
        border: clip.border,
        size: clip.size,
        offset: clip.offset,
      })
    }
    BackgroundClip::ContentBox => {
      let clip = ClipBox::content_box(*border, layout);

      Some(FillShape::RoundedRect {
        border: clip.border,
        size: clip.size,
        offset: clip.offset,
      })
    }
    BackgroundClip::BorderArea => {
      border.append_border_ring_commands(&mut commands, layout.size);
      Some(FillShape::Path {
        commands,
        rule: FillRule::EvenOdd,
      })
    }
    BackgroundClip::Text => None,
  }
}

/// Paints the decoration lines of one glyph run: `text-decoration` under, over, and through the
/// text.
pub fn paint_run_decorations<D: PaintDevice>(
  decorations: &[DecorationRect],
  over: bool,
  skip: TextDecorationLines,
  origin: Point<f32>,
  device: &mut D,
) {
  for decoration in decorations
    .iter()
    .filter(|line| line.over == over && !skip.contains(line.line))
  {
    if decoration.color.0[3] == 0 || decoration.width <= 0.0 || decoration.height <= 0.0 {
      continue;
    }
    let [a, b, c, d, e, f] = decoration.transform;

    device.fill_shape(
      &FillShape::Rect(Size {
        width: decoration.width,
        height: decoration.height,
      }),
      decoration.color,
      Affine {
        a,
        b,
        c,
        d,
        x: e + origin.x,
        y: f + origin.y,
      },
    );
  }
}

/// Paints a border ring, unless it needs per-side work: a uniform dashed or dotted border strokes
/// the centerline so the pattern runs round the whole ring, and a double border fills two rings.
pub fn paint_border<D: PaintDevice>(
  border: &BorderProperties,
  size: Size<f32>,
  origin: Point<f32>,
  device: &mut D,
) -> bool {
  let at = Affine::translation(origin.x, origin.y);

  match border.paint() {
    BorderPaint::Sides => return false,
    // A transparent ring is a fill nobody sees, and painting it would only
    // lengthen the output.
    BorderPaint::Ring { color }
    | BorderPaint::Double { color, .. }
    | BorderPaint::Stroked { color, .. }
      if color.0[3] == 0 => {}
    BorderPaint::Ring { color } => {
      let mut commands = Vec::with_capacity(BorderProperties::PATH_COMMANDS_AMOUNT * 2);

      border.append_border_ring_commands(&mut commands, size);
      device.fill_shape(
        &FillShape::Path {
          commands,
          rule: FillRule::EvenOdd,
        },
        color,
        at,
      );
    }
    BorderPaint::Double { color, width } => {
      let third = width / 3.0;

      for inset in [0.0, third * 2.0] {
        let mut ring = *border;

        ring.expand_by(Rect {
          top: -inset,
          right: -inset,
          bottom: -inset,
          left: -inset,
        });
        ring.width = Sides([third; 4]).into();

        let ring_size = Size {
          width: (size.width - inset * 2.0).max(0.0),
          height: (size.height - inset * 2.0).max(0.0),
        };
        let mut commands = Vec::with_capacity(BorderProperties::PATH_COMMANDS_AMOUNT * 2);

        ring.append_border_ring_commands(&mut commands, ring_size);
        device.fill_shape(
          &FillShape::Path {
            commands,
            rule: FillRule::EvenOdd,
          },
          color,
          Affine::translation(origin.x + inset, origin.y + inset),
        );
      }
    }
    BorderPaint::Stroked {
      color,
      width,
      style,
    } => {
      let half = width / 2.0;
      let mut center = *border;

      center.expand_by(Rect {
        top: -half,
        right: -half,
        bottom: -half,
        left: -half,
      });

      let center_size = Size {
        width: (size.width - width).max(0.0),
        height: (size.height - width).max(0.0),
      };
      let mut commands = Vec::with_capacity(BorderProperties::PATH_COMMANDS_AMOUNT);

      center.append_mask_commands(&mut commands, center_size, Point { x: half, y: half });

      let perimeter = center.approximate_rounded_rect_perimeter(center_size);
      let dash = style.dash_pattern(width, perimeter, true);

      device.stroke_shape(
        &FillShape::Path {
          commands,
          rule: FillRule::NonZero,
        },
        &StrokeStyle {
          color,
          width,
          dash: dash.map(|dash| dash.intervals),
          round_cap: dash.is_some_and(|dash| dash.round_cap),
        },
        at,
      );
    }
  }

  true
}
