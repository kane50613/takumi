use takumi_core::{
  geometry::{ComputedLayout as Layout, Point},
  painter::BoxBorderPainter,
};

use crate::{
  BorderProperties, Canvas,
  node_paint::CanvasDevice,
  style::{Affine, BorderStyle, Color, ImageScalingAlgorithm, Sides, SpacePair},
};

/// Draws debug borders around the node's layout areas.
pub(crate) fn draw_debug_border(canvas: &mut Canvas, layout: Layout, transform: Affine) {
  let outline = |color: Color| BorderProperties {
    width: Sides([1.0; 4]).into(),
    color: Sides([color; 4]).into(),
    radius: Sides([SpacePair::from_single(0.0); 4]),
    image_rendering: ImageScalingAlgorithm::Auto,
    collapsed: false,
    style: Sides([BorderStyle::Solid; 4]).into(),
    shape: Sides::default(),
  };

  let mut device = CanvasDevice::new(canvas, transform, ImageScalingAlgorithm::Auto);

  BoxBorderPainter::new(&outline(Color([255, 0, 0, 255])), layout.size)
    .paint(Point::ZERO, &mut device);
  BoxBorderPainter::new(&outline(Color([0, 255, 0, 255])), layout.content_box_size()).paint(
    Point {
      x: layout.padding.left + layout.border.left,
      y: layout.padding.top + layout.border.top,
    },
    &mut device,
  );
}
