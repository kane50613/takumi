use std::{collections::HashMap, sync::Arc};

use derive_builder::Builder;
use image::RgbaImage;
use taffy::{AvailableSpace, NodeId, TaffyError, TaffyTree, geometry::Size};

use crate::{
  GlobalContext,
  layout::{
    Viewport,
    node::Node,
    style::{
      Affine, Display, ImageScalingAlgorithm, InheritedStyle, SpacePair, apply_backdrop_filter,
      apply_filters,
    },
    tree::NodeTree,
  },
  rendering::{
    BorderProperties, Canvas, CanvasConstrain, CanvasConstrainResult, Sizing, draw_debug_border,
    overlay_image,
  },
  resources::image::ImageSource,
};

use crate::rendering::RenderContext;

#[derive(Clone, Builder)]
/// Options for rendering a node. Construct using [`RenderOptionsBuilder`] to avoid breaking changes.
pub struct RenderOptions<'g, N: Node<N>> {
  /// The viewport to render the node in.
  pub(crate) viewport: Viewport,
  /// The global context.
  pub(crate) global: &'g GlobalContext,
  /// The node to render.
  pub(crate) node: N,
  /// Whether to draw debug borders.
  #[builder(default)]
  pub(crate) draw_debug_border: bool,
  /// The resources fetched externally.
  #[builder(default)]
  pub(crate) fetched_resources: HashMap<Arc<str>, Arc<ImageSource>>,
}

/// Renders a node to an image.
pub fn render<'g, N: Node<N>>(options: RenderOptions<'g, N>) -> Result<RgbaImage, crate::Error> {
  let mut taffy = TaffyTree::new();

  let render_context = RenderContext {
    draw_debug_border: options.draw_debug_border,
    ..RenderContext::new(options.global, options.viewport, options.fetched_resources)
  };

  let tree = NodeTree::from_node(&render_context, options.node);

  let root_node_id = tree.insert_into_taffy(&mut taffy)?;

  taffy.compute_layout_with_measure(
    root_node_id,
    render_context.sizing.viewport.into(),
    |known_dimensions, available_space, _node_id, node_context, style| {
      if let Size {
        width: Some(width),
        height: Some(height),
      } = known_dimensions.maybe_apply_aspect_ratio(style.aspect_ratio)
      {
        Size { width, height }
      } else if let Some(context) = node_context {
        context.measure(available_space, known_dimensions, style)
      } else {
        Size::ZERO
      }
    },
  )?;

  let root_size = taffy
    .layout(root_node_id)?
    .size
    .map(|size| size.round() as u32);

  if root_size.width == 0 || root_size.height == 0 {
    return Err(crate::Error::InvalidViewport);
  }

  let root_size = root_size.zip_map(options.viewport.into(), |size, viewport| {
    if let AvailableSpace::Definite(defined) = viewport {
      defined as u32
    } else {
      size
    }
  });

  let mut canvas = Canvas::new(root_size);

  render_node(&mut taffy, root_node_id, &mut canvas, Affine::IDENTITY)?;

  Ok(canvas.into_inner())
}

fn apply_transform(
  transform: &mut Affine,
  style: &InheritedStyle,
  border_box: Size<f32>,
  sizing: &Sizing,
) {
  let transform_origin = style.transform_origin.unwrap_or_default();
  let origin = transform_origin.to_point(sizing, border_box);

  // CSS Transforms Level 2 order: T(origin) * translate * rotate * scale * transform * T(-origin)
  // Ref: https://www.w3.org/TR/css-transforms-2/#ctm

  let mut local = Affine::translation(origin.x, origin.y);

  let translate = style.resolve_translate();
  if translate != SpacePair::default() {
    local *= Affine::translation(
      translate.x.to_px(sizing, border_box.width),
      translate.y.to_px(sizing, border_box.height),
    );
  }

  if let Some(rotate) = style.rotate {
    local *= Affine::rotation(rotate);
  }

  let scale = style.resolve_scale();
  if scale != SpacePair::default() {
    local *= Affine::scale(scale.x.0, scale.y.0);
  }

  if let Some(node_transform) = &style.transform {
    local *= Affine::from_transforms(node_transform.iter(), sizing, border_box);
  }

  local *= Affine::translation(-origin.x, -origin.y);

  *transform *= local;
}

fn render_node<'g, Nodes: Node<Nodes>>(
  taffy: &mut TaffyTree<NodeTree<'g, Nodes>>,
  node_id: NodeId,
  canvas: &mut Canvas,
  mut transform: Affine,
) -> Result<(), crate::Error> {
  let layout = *taffy.layout(node_id)?;

  let Some(node) = taffy.get_node_context_mut(node_id) else {
    return Err(TaffyError::InvalidInputNode(node_id).into());
  };

  if node.context.opacity == 0 || node.context.style.display == Display::None {
    return Ok(());
  }

  transform *= Affine::translation(layout.location.x, layout.location.y);

  apply_transform(
    &mut transform,
    &node.context.style,
    layout.size,
    &node.context.sizing,
  );

  // If a transform function causes the current transformation matrix of an object to be non-invertible, the object and its content do not get displayed.
  // https://drafts.csswg.org/css-transforms/#transform-function-lists
  if !transform.is_invertible() {
    return Ok(());
  }

  node.context.transform = transform;

  // Normal rendering path (no filters requiring node-level rendering)
  let constrain = CanvasConstrain::from_node(
    &node.context,
    &node.context.style,
    layout,
    transform,
    &mut canvas.mask_memory,
  )?;

  // Skip rendering if the node is not visible
  if matches!(constrain, CanvasConstrainResult::SkipRendering) {
    return Ok(());
  }

  let has_constrain = constrain.is_some();

  // Apply backdrop-filter effects to the area behind this element
  if !node.context.style.backdrop_filter.is_empty() {
    let border = BorderProperties::from_context(&node.context, layout.size, layout.border);

    apply_backdrop_filter(canvas, border, layout.size, transform, &node.context);
  }

  let should_create_isolated_canvas = !node.context.style.filter.is_empty();

  // If isolated canvas is required, replace the current canvas with a new one.
  // Make sure to merge the image back!
  let original_canvas_image = if should_create_isolated_canvas {
    Some(canvas.replace_new_image())
  } else {
    None
  };

  match constrain {
    CanvasConstrainResult::None => {
      node.draw_shell(canvas, layout)?;
    }
    CanvasConstrainResult::Some(constrain) => match constrain {
      CanvasConstrain::ClipPath { .. } | CanvasConstrain::MaskImage { .. } => {
        canvas.push_constrain(constrain);
        node.draw_shell(canvas, layout)?;
      }
      CanvasConstrain::Overflow { .. } => {
        node.draw_shell(canvas, layout)?;
        canvas.push_constrain(constrain);
      }
    },
    CanvasConstrainResult::SkipRendering => unreachable!(),
  }

  node.draw_content(canvas, layout)?;

  if node.context.draw_debug_border {
    draw_debug_border(canvas, layout, transform);
  }

  let filters = node.context.style.filter.clone();
  let sizing = node.context.sizing;
  let current_color = node.context.current_color;
  let opacity = node.context.opacity;

  if node.should_create_inline_layout() {
    node.draw_inline(canvas, layout)?;
  } else {
    for child_id in taffy.children(node_id)? {
      render_node(taffy, child_id, canvas, transform)?;
    }
  }

  apply_filters(
    &mut canvas.image,
    &sizing,
    current_color,
    opacity,
    filters.iter(),
  );

  // If there was an isolated canvas, composite the filtered image back into the original canvas
  if let Some(mut original_canvas_image) = original_canvas_image {
    overlay_image(
      &mut original_canvas_image,
      &canvas.image,
      BorderProperties::zero(),
      Affine::IDENTITY,
      ImageScalingAlgorithm::Auto,
      255,
      None,
      &mut canvas.mask_memory,
    );

    canvas.image = original_canvas_image;
  }

  if has_constrain {
    canvas.pop_constrain();
  }

  Ok(())
}
