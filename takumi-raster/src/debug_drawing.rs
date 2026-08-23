use takumi_core::geometry::ComputedLayout as Layout;

use crate::{
  BorderProperties, Canvas, paint_border,
  style::{Affine, BorderStyle, Color, ImageScalingAlgorithm, Sides, SpacePair},
};

/// Draws debug borders around the node's layout areas.
pub(crate) fn draw_debug_border(canvas: &mut Canvas, layout: Layout, transform: Affine) {
  // border-box
  let border_box = BorderProperties {
    width: Sides([1.0; 4]).into(),
    color: Sides([Color([255, 0, 0, 255]); 4]).into(), // red
    radius: Sides([SpacePair::from_single(0.0); 4]),
    image_rendering: ImageScalingAlgorithm::Auto,
    collapsed: false,
    style: Sides([BorderStyle::Solid; 4]).into(),
    shape: Sides::default(),
  };
  paint_border(border_box, canvas, layout.size, transform, None);

  // content-box
  let content_box = BorderProperties {
    width: Sides([1.0; 4]).into(),
    color: Sides([Color([0, 255, 0, 255]); 4]).into(), // green
    radius: Sides([SpacePair::from_single(0.0); 4]),
    image_rendering: ImageScalingAlgorithm::Auto,
    collapsed: false,
    style: Sides([BorderStyle::Solid; 4]).into(),
    shape: Sides::default(),
  };
  paint_border(
    content_box,
    canvas,
    layout.content_box_size(),
    transform
      * Affine::translation(
        layout.padding.left + layout.border.left,
        layout.padding.top + layout.border.top,
      ),
    None,
  );
}
