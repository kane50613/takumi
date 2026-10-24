//! Lengths in user units, after Blink's `SVGLengthContext`.

use super::State;
use crate::{
  svg::tree::Units,
  svg::{Attr, Length, LengthContext, LengthMode, Node as SvgNode},
};

/// The axis a percentage on `attr` resolves against.
fn length_mode(attr: Attr) -> LengthMode {
  match attr {
    Attr::Cx
    | Attr::Dx
    | Attr::Fx
    | Attr::MarkerWidth
    | Attr::RefX
    | Attr::Rx
    | Attr::Width
    | Attr::X
    | Attr::X1
    | Attr::X2 => LengthMode::Width,
    Attr::Cy
    | Attr::Dy
    | Attr::Fy
    | Attr::Height
    | Attr::MarkerHeight
    | Attr::RefY
    | Attr::Ry
    | Attr::Y
    | Attr::Y1
    | Attr::Y2 => LengthMode::Height,
    _ => LengthMode::Other,
  }
}

/// What a length on `node` resolves against.
pub(crate) fn context(node: SvgNode, state: &State) -> LengthContext {
  LengthContext {
    font_size: node
      .style()
      .map_or(state.opt.font_size, |style| style.font_size),
    width: state.view_box.width(),
    height: state.view_box.height(),
    dpi: state.opt.dpi,
  }
}

/// A length in user units, or in fractions of the bounding box when `object_units` says so.
pub(crate) fn convert_length(
  length: Length,
  node: SvgNode,
  attr: Attr,
  object_units: Units,
  state: &State,
) -> f32 {
  let context = context(node, state);

  if object_units == Units::ObjectBoundingBox {
    length.to_fraction_or_user_units(&context)
  } else {
    length.to_user_units(length_mode(attr), &context)
  }
}

/// A length whose percentage is of the normalized viewport diagonal, as for `stroke-width`.
pub(crate) fn convert_other_length(length: Length, node: SvgNode, state: &State) -> f32 {
  length.to_user_units(LengthMode::Other, &context(node, state))
}

pub(crate) fn convert_user_length(length: Length, node: SvgNode, attr: Attr, state: &State) -> f32 {
  convert_length(length, node, attr, Units::UserSpaceOnUse, state)
}
