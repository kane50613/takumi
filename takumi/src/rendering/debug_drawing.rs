use taffy::Layout;

use crate::{
  layout::style::{Affine, Color, Sides, SpacePair},
  rendering::{BorderProperties, Canvas},
};

/// Draws debug borders around the node's layout areas.
pub fn draw_debug_border(canvas: &mut Canvas, layout: Layout, transform: Affine) {
  // border-box
  BorderProperties {
    width: Sides([1.0; 4]).into(),
    color: Color([255, 0, 0, 255]), // red
    radius: Sides([SpacePair::from_single(0.0); 4]),
  }
  .draw(canvas, layout.size, transform, None);

  // content-box
  BorderProperties {
    width: Sides([1.0; 4]).into(),
    color: Color([0, 255, 0, 255]), // green
    radius: Sides([SpacePair::from_single(0.0); 4]),
  }
  .draw(
    canvas,
    layout.content_box_size(),
    Affine::translation(
      layout.padding.left + layout.border.left,
      layout.padding.top + layout.border.top,
    ) * transform,
    None,
  );
}
