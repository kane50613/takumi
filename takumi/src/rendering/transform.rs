use super::{FastBlendImage, RenderContext};
use crate::layout::{
  node::Node,
  style::{LengthUnit, TransformFunction, TransformOrigin},
};
use image::{Rgba, RgbaImage};
use imageproc::geometric_transformations::{Interpolation, Projection};
use nalgebra::{Point2, Projective2, Rotation2, Translation2};
use taffy::{NodeId, Point, TaffyTree};

// Helper to draw a node and its children recursively onto a buffer without checking for transforms.
pub fn draw_subtree_to_buffer<Nodes: Node<Nodes>>(
  taffy: &TaffyTree<crate::rendering::render::NodeRender<'_, Nodes>>, // access NodeRender
  node_id: NodeId,
  canvas: &mut FastBlendImage,
  offset_from_root: Point<f32>,
) {
  let node_context = taffy.get_node_context(node_id).unwrap();
  let mut layout = *taffy.layout(node_id).unwrap();

  layout.location.x = offset_from_root.x;
  layout.location.y = offset_from_root.y;

  node_context
    .node
    .draw_on_canvas(&node_context.context, canvas, layout);

  let children_ids = taffy.children(node_id).unwrap();
  for child_id in children_ids {
    let child_layout = *taffy.layout(child_id).unwrap();
    let child_offset = Point {
      x: offset_from_root.x + child_layout.location.x,
      y: offset_from_root.y + child_layout.location.y,
    };
    draw_subtree_to_buffer(taffy, child_id, canvas, child_offset);
  }
}

fn resolve_length_unit(unit: LengthUnit, base: f32, context: &RenderContext) -> f32 {
  match unit {
    LengthUnit::Percentage(p) => (p / 100.0) * base,
    _ => unit.resolve_to_px(context),
  }
}

// Main function to handle the transformation of a node and its entire subtree.
pub fn draw_transformed_node_and_children<Nodes: Node<Nodes>>(
  taffy: &TaffyTree<crate::rendering::render::NodeRender<'_, Nodes>>,
  node_id: NodeId,
  canvas: &mut FastBlendImage,
  parent_offset: Point<f32>,
) {
  let node_context = taffy.get_node_context(node_id).unwrap();
  let style = node_context.node.get_style();
  let layout = *taffy.layout(node_id).unwrap();
  let (w, h) = (layout.size.width, layout.size.height);
  let (w_u32, h_u32) = (w as u32, h as u32);

  if w_u32 == 0 || h_u32 == 0 {
    return;
  }

  // 1. Create a temporary buffer and draw the entire subtree onto it.
  let mut node_canvas = FastBlendImage(RgbaImage::new(w_u32, h_u32));
  draw_subtree_to_buffer(taffy, node_id, &mut node_canvas, Point { x: 0.0, y: 0.0 });

  // 2. Determine the rotation and origin.
  let degrees = style
    .transform
    .as_ref()
    .and_then(|t| {
      t.0.iter().find_map(|f| match f {
        TransformFunction::Rotate(deg) => Some(*deg),
        _ => None,
      })
    })
    .unwrap_or(0.0);

  if degrees == 0.0 {
    let final_x = parent_offset.x + layout.location.x;
    let final_y = parent_offset.y + layout.location.y;
    canvas.overlay_image(&node_canvas.0, final_x as i32, final_y as i32);
    return;
  }

  let origin = style.transform_origin.unwrap_or(TransformOrigin(
    LengthUnit::Percentage(50.0),
    LengthUnit::Percentage(50.0),
  ));
  let ox = resolve_length_unit(origin.0, w, &node_context.context);
  let oy = resolve_length_unit(origin.1, h, &node_context.context);

  // 3. Build the transformation matrix.
  let to_origin = Translation2::new(-ox, -oy);
  let rotation = Rotation2::new(degrees.to_radians());
  let from_origin = Translation2::new(ox, oy);
  let transform: Projective2<f32> =
    Projective2::from_matrix_unchecked((from_origin * rotation * to_origin).to_homogeneous());

  // 4. Calculate the bounding box of the transformed image.
  let corners = [
    Point2::new(0.0, 0.0),
    Point2::new(w, 0.0),
    Point2::new(w, h),
    Point2::new(0.0, h),
  ];
  let transformed_corners: Vec<_> = corners.iter().map(|p| transform * p).collect();
  let min_x = transformed_corners
    .iter()
    .map(|p| p.x)
    .fold(f32::INFINITY, f32::min);
  let max_x = transformed_corners
    .iter()
    .map(|p| p.x)
    .fold(f32::NEG_INFINITY, f32::max);
  let min_y = transformed_corners
    .iter()
    .map(|p| p.y)
    .fold(f32::INFINITY, f32::min);
  let max_y = transformed_corners
    .iter()
    .map(|p| p.y)
    .fold(f32::NEG_INFINITY, f32::max);

  let new_width = (max_x - min_x).ceil() as u32;
  let new_height = (max_y - min_y).ceil() as u32;

  // 5. Warp the rendered subtree into a new buffer of the correct size.
  let offset_to_new_buffer = Translation2::new(-min_x, -min_y);
  let final_transform = offset_to_new_buffer * transform;
  let inverse_transform = final_transform
    .try_inverse()
    .unwrap_or_else(Projective2::identity);

  let h_matrix = inverse_transform.to_homogeneous().transpose();
  let proj = Projection::from_matrix(h_matrix.as_slice().try_into().unwrap()).unwrap();

  let mut transformed_buffer = RgbaImage::new(new_width, new_height);
  imageproc::geometric_transformations::warp_into(
    &node_canvas.0,
    &proj,
    Interpolation::Bilinear,
    Rgba([0, 0, 0, 0]),
    &mut transformed_buffer,
  );

  // 6. Composite the final result onto the main canvas.
  let final_x = parent_offset.x + layout.location.x + min_x;
  let final_y = parent_offset.y + layout.location.y + min_y;
  canvas.overlay_image(&transformed_buffer, final_x as i32, final_y as i32);
}
