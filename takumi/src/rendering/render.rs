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
      Affine, Display, DropShadowFilter, Filters, ImageScalingAlgorithm, InheritedStyle, SpacePair,
    },
    tree::NodeTree,
  },
  rendering::{
    BorderProperties, Canvas, CanvasConstrain, CanvasConstrainResult, MaskMemory, apply_fast_blur,
    draw_debug_border, overlay_image,
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
    render_context.viewport.into(),
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
  context: &RenderContext,
) {
  let transform_origin = style.transform_origin.unwrap_or_default();
  let origin = transform_origin.to_point(context, border_box);

  // CSS Transforms Level 2 order: T(origin) * translate * rotate * scale * transform * T(-origin)
  // Ref: https://www.w3.org/TR/css-transforms-2/#ctm

  let mut local = Affine::translation(origin.x, origin.y);

  let translate = style.resolve_translate();
  if translate != SpacePair::default() {
    local *= Affine::translation(
      translate.x.resolve_to_px(context, border_box.width),
      translate.y.resolve_to_px(context, border_box.height),
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
    local *= Affine::from_transforms(node_transform.iter(), context, border_box);
  }

  local *= Affine::translation(-origin.x, -origin.y);

  *transform *= local;
}

/// Applies a drop-shadow filter effect to an image.
/// This renders the shadow based on the source image's alpha channel.
fn apply_drop_shadow_filter(
  canvas: &mut RgbaImage,
  source: &RgbaImage,
  shadow_filter: &DropShadowFilter,
  context: &RenderContext,
  layout_size: Size<f32>,
  transform: Affine,
  mask_memory: &mut MaskMemory,
) {
  let offset_x = shadow_filter
    .offset_x
    .resolve_to_px(context, layout_size.width);
  let offset_y = shadow_filter
    .offset_y
    .resolve_to_px(context, layout_size.height);
  let blur_radius = shadow_filter
    .blur_radius
    .resolve_to_px(context, layout_size.width);
  let color = shadow_filter
    .color
    .resolve(context.current_color, context.opacity);

  // Calculate expansion needed for blur (blur spreads ~1.5x the radius)
  let blur_expansion = (blur_radius * 1.5).ceil() as u32;

  // Create an expanded shadow image to accommodate blur spread
  let expanded_width = source.width() + blur_expansion * 2;
  let expanded_height = source.height() + blur_expansion * 2;
  let mut shadow_image = RgbaImage::new(expanded_width, expanded_height);

  // Copy the alpha channel into the center of the expanded image
  for (y, source_row) in source.rows().enumerate() {
    for (x, source_pixel) in source_row.enumerate() {
      let alpha = source_pixel.0[3];
      if alpha > 0 {
        let shadow_pixel =
          shadow_image.get_pixel_mut(x as u32 + blur_expansion, y as u32 + blur_expansion);
        shadow_pixel.0 = [color.0[0], color.0[1], color.0[2], alpha];
      }
    }
  }

  // Apply blur to the shadow
  apply_fast_blur(&mut shadow_image, blur_radius);

  // Composite the shadow at the offset position (adjusted for blur expansion)
  let shadow_transform = transform
    * Affine::translation(
      offset_x - blur_expansion as f32,
      offset_y - blur_expansion as f32,
    );

  overlay_image(
    canvas,
    shadow_image.into(),
    BorderProperties::zero(),
    shadow_transform,
    ImageScalingAlgorithm::Auto,
    None,
    255,
    None,
    mask_memory,
  );
}

/// Macro to handle the constrain application pattern that appears in both
/// filter rendering and normal rendering paths.
/// Returns `true` if a constrain was pushed that needs popping later.
macro_rules! apply_constrain {
  ($canvas:expr, $node:expr, $layout:expr, $constrain:expr) => {{
    match $constrain {
      CanvasConstrainResult::SkipRendering => return Ok(()),
      CanvasConstrainResult::None => {
        $node.draw_shell($canvas, $layout)?;
        false
      }
      CanvasConstrainResult::Some(constrain) => match constrain {
        CanvasConstrain::ClipPath { .. } | CanvasConstrain::MaskImage { .. } => {
          $canvas.push_constrain(constrain);
          $node.draw_shell($canvas, $layout)?;
          true
        }
        CanvasConstrain::Overflow { .. } => {
          $node.draw_shell($canvas, $layout)?;
          $canvas.push_constrain(constrain);
          true
        }
      },
    }
  }};
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
    &node.context,
  );

  // If a transform function causes the current transformation matrix of an object to be non-invertible, the object and its content do not get displayed.
  // https://drafts.csswg.org/css-transforms/#transform-function-lists
  if !transform.is_invertible() {
    return Ok(());
  }

  node.context.transform = transform;

  // Check if the node has filters that require node-level rendering (blur or drop-shadow)
  let requires_filter_rendering = node
    .context
    .style
    .filter
    .as_ref()
    .is_some_and(|f: &Filters| f.requires_node_level_rendering());

  // Get filter info before borrowing canvas mutably
  let filter_info = if requires_filter_rendering {
    let filters = node.context.style.filter.as_ref();
    let blur_radius = filters.and_then(|f: &Filters| f.get_blur());
    let drop_shadows: Vec<_> = filters
      .map(|f| f.get_drop_shadows().cloned().collect())
      .unwrap_or_default();
    Some((blur_radius, drop_shadows, node.context.clone()))
  } else {
    None
  };

  // If we have filters requiring node-level rendering, use a completely separate code path
  if let Some((blur_radius, drop_shadows, original_context)) = filter_info {
    // Calculate the blur expansion needed - we need extra space for blur to spread into
    let max_blur = blur_radius
      .map(|b| b.resolve_to_px(&original_context, layout.size.width))
      .unwrap_or(0.0);

    // Also account for drop shadow blur
    let max_shadow_blur = drop_shadows
      .iter()
      .map(|s| {
        s.blur_radius
          .resolve_to_px(&original_context, layout.size.width)
      })
      .fold(0.0_f32, f32::max);

    // Take the max of blur and shadow blur, add some padding (blur typically spreads ~3x the sigma)
    let blur_expansion = (max_blur.max(max_shadow_blur) * 1.5).ceil() as u32;

    // Calculate the size needed for the temporary canvas (with blur expansion on all sides)
    let temp_size = layout.size.map(|s| s.ceil() as u32 + blur_expansion * 2);

    if temp_size.width == 0 || temp_size.height == 0 {
      return Ok(());
    }

    // Update the node's transform to render at an offset for the blur expansion
    // We need to do this AFTER we've cloned the original context
    node.context.transform = Affine::translation(blur_expansion as f32, blur_expansion as f32);

    // Render to temp canvas (with offset for blur expansion)
    let mut temp_canvas = Canvas::new(temp_size);

    let constrain = CanvasConstrain::from_node(
      &node.context,
      &node.context.style,
      layout,
      Affine::IDENTITY,
      &mut temp_canvas.mask_memory,
    )?;

    let has_constrain = apply_constrain!(&mut temp_canvas, node, layout, constrain);

    node.draw_content(&mut temp_canvas, layout)?;

    if node.context.draw_debug_border {
      draw_debug_border(&mut temp_canvas, layout, Affine::IDENTITY);
    }

    if node.should_create_inline_layout() {
      node.draw_inline(&mut temp_canvas, layout)?;
    } else {
      let child_transform = Affine::translation(blur_expansion as f32, blur_expansion as f32);
      for child_id in taffy.children(node_id)? {
        render_node(taffy, child_id, &mut temp_canvas, child_transform)?;
      }
    }

    if has_constrain {
      temp_canvas.pop_constrain();
    }

    // Now apply filter effects and composite to main canvas
    let mut temp_image = temp_canvas.into_inner();
    // Use the original transform, but offset by the negative blur expansion to align correctly
    let composite_transform = original_context.transform
      * Affine::translation(-(blur_expansion as f32), -(blur_expansion as f32));

    // Apply drop-shadow filters (draw shadows first, behind the content)
    for shadow_filter in &drop_shadows {
      apply_drop_shadow_filter(
        &mut canvas.image,
        &temp_image,
        shadow_filter,
        &original_context,
        layout.size,
        composite_transform,
        &mut canvas.mask_memory,
      );
    }

    // Apply blur filter if present
    if let Some(blur) = blur_radius {
      let blur_px = blur.resolve_to_px(&original_context, layout.size.width);
      apply_fast_blur(&mut temp_image, blur_px);
    }

    // Composite the (possibly blurred) content to the main canvas
    overlay_image(
      &mut canvas.image,
      temp_image.into(),
      BorderProperties::zero(),
      composite_transform,
      ImageScalingAlgorithm::Auto,
      None,
      original_context.opacity,
      None,
      &mut canvas.mask_memory,
    );

    return Ok(());
  }

  // Normal rendering path (no filters requiring node-level rendering)
  let constrain = CanvasConstrain::from_node(
    &node.context,
    &node.context.style,
    layout,
    transform,
    &mut canvas.mask_memory,
  )?;

  let has_constrain = apply_constrain!(canvas, node, layout, constrain);

  node.draw_content(canvas, layout)?;

  if node.context.draw_debug_border {
    draw_debug_border(canvas, layout, transform);
  }

  if node.should_create_inline_layout() {
    node.draw_inline(canvas, layout)?;
  } else {
    for child_id in taffy.children(node_id)? {
      render_node(taffy, child_id, canvas, transform)?;
    }
  }

  if has_constrain {
    canvas.pop_constrain();
  }

  Ok(())
}
