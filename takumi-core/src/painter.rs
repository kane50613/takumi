//! The seam between deciding what to paint and painting it.
//!
//! Everything above this seam is the same for every backend: which box a
//! declaration paints into, what shape that box is, and what colour goes in it.
//! Below it, a rasterizer writes pixels, an SVG writer emits elements, and a
//! PDF writer emits operators. Only the second half belongs to a backend.

use crate::context::RenderContext;
use crate::geometry::{ComputedLayout, PathCommand, Point, Size};
use crate::layout::border::BorderProperties;
use crate::layout::decoration::ClipBox;
use crate::layout::decoration::{OutlineGeometry, outline_paint};
use crate::layout::inline::DecorationRect;
use crate::shadow::SizedShadow;
use crate::style::BoxShadow;
use crate::style::{Affine, BackgroundClip, Color, FillRule, TextDecorationLines};

/// A closed shape to fill, in the coordinate space of the box that owns it.
///
/// A backend that can express a plain rectangle more cheaply than a path gets
/// told which it is, the way `GraphicsContext` splits `DrawRect` from
/// `DrawPath`.
pub enum FillShape {
  /// An axis-aligned rectangle at the box origin.
  Rect(Size<f32>),
  /// A rectangle with the corners `border` gives it. A rasterizer composites
  /// one directly; a vector backend turns it into a path.
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

/// What a backend has to be able to do for the shared painting code to drive
/// it. A method takes coordinates in the page's space, so the caller never has
/// to know how a backend tracks its own transform.
pub trait PaintDevice {
  /// Fills `shape` under `transform`, with a single colour.
  fn fill_shape(&mut self, shape: &FillShape, color: Color, transform: Affine);
}

/// A box's `box-shadow` layers, split by where they fall.
#[derive(Default, Clone)]
pub struct BoxShadows {
  /// Shadows inside the box.
  pub inset: Vec<SizedShadow>,
  /// Shadows outside it.
  pub outer: Vec<SizedShadow>,
}

/// One step of painting a box, in the order the steps run.
///
/// Follows CSS 2.1 Appendix E and Blink's `PaintPhase`, where the outline is
/// the last phase and so paints above the box's own content.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoxPaintStep {
  /// `box-shadow` layers that fall outside the box.
  OuterShadow,
  /// `background-color` and the `background-image` layers.
  Background,
  /// `box-shadow` layers that fall inside the box.
  InsetShadow,
  /// `border-*`.
  Border,
  /// The box's own content: its text, its image, its inline layout.
  Content,
  /// `outline`, above everything else the box paints.
  Outline,
}

/// Everything a backend needs to paint one box, decided once.
///
/// The decisions live here so a backend never repeats them: which shape a
/// declaration paints into, whether it paints at all, and with what. A backend
/// supplies a [`PaintDevice`] and gets pixels, elements, or operators out.
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

  /// Prepares a fragment of the box that paints its own decorations, which is
  /// what `box-decoration-break: clone` asks for.
  pub fn fragment(context: &'c RenderContext, layout: ComputedLayout, size: Size<f32>) -> Self {
    Self::new(context, ComputedLayout { size, ..layout })
  }

  /// The box's border geometry, corners included.
  pub fn border(&self) -> &BorderProperties {
    &self.border
  }

  /// The box a background paints into, per `background-clip`. `None` when the
  /// declaration paints no box at all, which is what `text` does: the fill
  /// moves onto the glyphs.
  pub fn background_clip_shape(&self) -> Option<FillShape> {
    background_clip_shape(
      self.context.style.background_clip,
      &self.border,
      self.layout,
    )
  }

  /// Paints `background-color`. Does nothing when the colour is transparent or
  /// `background-clip` moves the fill onto the glyphs.
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

  /// The box's `box-shadow` layers, resolved and split into the ones that fall
  /// inside the box and the ones outside it. A fully transparent shadow is
  /// dropped: it paints nothing anywhere.
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

  /// The order the box's steps run in.
  ///
  /// `background-clip: border-area` fills the border ring, so its background
  /// paints over the border rather than under it.
  pub fn paint_order(&self) -> [BoxPaintStep; 6] {
    use BoxPaintStep::*;

    match self.context.style.background_clip {
      BackgroundClip::BorderArea => [
        OuterShadow,
        InsetShadow,
        Border,
        Background,
        Content,
        Outline,
      ],
      _ => [
        OuterShadow,
        Background,
        InsetShadow,
        Border,
        Content,
        Outline,
      ],
    }
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

/// Paints the decoration lines of one glyph run: `text-decoration` under, over,
/// and through the text. `over` selects the lines that paint above the glyphs.
///
/// Skip-ink is not applied here. It needs the glyphs rasterized, so the raster
/// backend refines the underline itself, and it paints the rest with subpixel
/// coverage this rect fill does not reproduce.
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
