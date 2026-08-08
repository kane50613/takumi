//! The seam between deciding what to paint and painting it.
//!
//! Everything above this seam is the same for every backend: which box a
//! declaration paints into, what shape that box is, and what colour goes in it.
//! Below it, a rasterizer writes pixels, an SVG writer emits elements, and a
//! PDF writer emits operators. Only the second half belongs to a backend.

use crate::geometry::{ComputedLayout, PathCommand, Point, Size};
use crate::layout::border::BorderProperties;
use crate::layout::decoration::ClipBox;
use crate::style::{BackgroundClip, Color, ComputedStyle, FillRule};

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
  /// Fills `shape`, offset by `origin`, with a single colour.
  fn fill_shape(&mut self, shape: &FillShape, color: Color, origin: Point<f32>);
}

/// The box a background paints into, per `background-clip`. `None` when the
/// declaration paints no box at all, which is what `text` does: the fill moves
/// onto the glyphs.
pub fn background_clip_shape(
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

/// Paints a box's `background-color`. Does nothing when the colour is
/// transparent or `background-clip` moves the fill onto the glyphs.
pub fn paint_background_color<D: PaintDevice>(
  style: &ComputedStyle,
  current_color: Color,
  border: &BorderProperties,
  layout: ComputedLayout,
  origin: Point<f32>,
  device: &mut D,
) {
  let color = style.background_color.resolve(current_color);

  if color.0[3] == 0 {
    return;
  }
  let Some(shape) = background_clip_shape(style.background_clip, border, layout) else {
    return;
  };

  device.fill_shape(&shape, color, origin);
}
