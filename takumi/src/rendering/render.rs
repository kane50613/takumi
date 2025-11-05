use std::{collections::HashMap, sync::Arc};

use derive_builder::Builder;
use image::RgbaImage;
use taffy::{AvailableSpace, Layout, NodeId, Point, TaffyTree, geometry::Size};

use crate::{
  GlobalContext,
  layout::{
    Viewport,
    node::Node,
    style::{Affine, Color, Display, InheritedStyle, Overflow},
    tree::NodeTree,
  },
  rendering::{BorderProperties, Canvas},
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

  let mut canvas = Canvas::new(options.viewport.into());

  let render_context = RenderContext {
    draw_debug_border: options.draw_debug_border,
    ..RenderContext::new(options.global, options.viewport, options.fetched_resources)
  };

  let tree = NodeTree::from_node(&render_context, options.node);

  let root_node_id = tree.insert_into_taffy(&mut taffy);

  let available_space = Size {
    width: AvailableSpace::Definite(render_context.viewport.width as f32),
    height: AvailableSpace::Definite(render_context.viewport.height as f32),
  };

  taffy
    .compute_layout_with_measure(
      root_node_id,
      available_space,
      |known_dimensions, available_space, _node_id, node_context, style| {
        if let Size {
          width: Some(width),
          height: Some(height),
        } = known_dimensions.maybe_apply_aspect_ratio(style.aspect_ratio)
        {
          Size { width, height }
        } else {
          node_context
            .unwrap()
            .measure(available_space, known_dimensions, style)
        }
      },
    )
    .unwrap();

  render_node(
    &mut taffy,
    root_node_id,
    &mut canvas,
    Point::ZERO,
    Affine::identity(),
  );

  Ok(canvas.into_inner())
}

fn create_transform(style: &InheritedStyle, layout: &Layout, context: &RenderContext) -> Affine {
  let mut transform = Affine::identity();

  let transform_origin = style.transform_origin.0.unwrap_or_default();

  let center = Point {
    x: transform_origin
      .0
      .x
      .to_length_unit()
      .resolve_to_px(context, layout.size.width),
    y: transform_origin
      .0
      .y
      .to_length_unit()
      .resolve_to_px(context, layout.size.height),
  };

  // According to https://www.w3.org/TR/css-transforms-2/#ctm
  // the order is `translate` -> `rotate` -> `scale` -> `transform`.
  // But we need to invert the order because of matrix multiplication.
  if let Some(node_transform) = &*style.transform {
    transform = transform * node_transform.to_affine(context, layout, center);
  }

  if let Some(scale) = *style.scale {
    transform = transform
      * Affine::scale(
        Size {
          width: scale.x.0,
          height: scale.y.0,
        },
        center,
      );
  }

  if let Some(rotate) = *style.rotate {
    transform = transform * Affine::rotation(rotate, center);
  }

  if let Some(translate) = *style.translate {
    transform = transform
      * Affine::translation(Size {
        width: translate.x.resolve_to_px(context, layout.size.width),
        height: translate.y.resolve_to_px(context, layout.size.height),
      });
  }

  transform
}

fn render_node<'g, Nodes: Node<Nodes>>(
  taffy: &mut TaffyTree<NodeTree<'g, Nodes>>,
  node_id: NodeId,
  canvas: &mut Canvas,
  offset: Point<f32>,
  mut transform: Affine,
) {
  let mut layout = *taffy.layout(node_id).unwrap();
  let node_context = taffy.get_node_context_mut(node_id).unwrap();

  if node_context.context.opacity == 0.0 || node_context.context.style.display == Display::None {
    return;
  }

  layout.location = layout.location + offset;

  transform =
    transform * create_transform(&node_context.context.style, &layout, &node_context.context);

  node_context.context.transform = transform;

  if let Some(clip) = &node_context.context.style.clip_path.0 {
    let translation = node_context.context.transform.decompose().translation;

    node_context.context.transform.zero_translation();

    let (mask, mut placement) = clip.render_mask(&node_context.context, layout.size);

    node_context.context.transform.x = -placement.left as f32;
    node_context.context.transform.y = -placement.top as f32;

    let mut inner_canvas = Canvas::new(Size {
      width: placement.width,
      height: placement.height,
    });

    let inner_layout = Layout {
      location: Point::zero(),
      ..layout
    };

    node_context.draw_on_canvas(&mut inner_canvas, inner_layout);

    if node_context.should_create_inline_layout() {
      node_context.draw_inline(&mut inner_canvas, inner_layout);
    } else {
      for child_id in taffy.children(node_id).unwrap() {
        render_node(taffy, child_id, &mut inner_canvas, Point::zero(), transform);
      }
    }

    placement.left += (layout.location.x + translation.width) as i32;
    placement.top += (layout.location.y + translation.height) as i32;

    return canvas.draw_mask(
      &mask,
      placement,
      Color::transparent(),
      Some(inner_canvas.into_inner()),
    );
  }

  node_context.draw_on_canvas(canvas, layout);

  let overflow = node_context.context.style.resolve_overflows();

  if overflow.should_clip_content() {
    // if theres no space for canvas to draw, just return.
    let Some(mut inner_canvas) = overflow.create_clip_canvas(node_context.context.viewport, layout)
    else {
      return;
    };

    let image_rendering = node_context.context.style.image_rendering;
    let filters = node_context.context.style.filter.0.clone();

    let offset = Point {
      x: if overflow.0.x == Overflow::Visible {
        layout.location.x
      } else {
        0.0
      },
      y: if overflow.0.y == Overflow::Visible {
        layout.location.y
      } else {
        0.0
      },
    };

    if node_context.should_create_inline_layout() {
      node_context.draw_inline(
        &mut inner_canvas,
        Layout {
          size: layout.content_box_size(),
          location: offset,
          ..Default::default()
        },
      );
    } else {
      for child_id in taffy.children(node_id).unwrap() {
        render_node(taffy, child_id, &mut inner_canvas, offset, transform);
      }
    }

    return canvas.overlay_image(
      &inner_canvas.into_inner(),
      Point {
        x: if overflow.0.x == Overflow::Visible {
          0
        } else {
          layout.location.x as i32
        },
        y: if overflow.0.y == Overflow::Visible {
          0
        } else {
          layout.location.y as i32
        },
      },
      BorderProperties::zero(),
      transform,
      image_rendering,
      filters.as_ref(),
    );
  }

  if node_context.should_create_inline_layout() {
    node_context.draw_inline(canvas, layout);
  } else {
    for child_id in taffy.children(node_id).unwrap() {
      render_node(taffy, child_id, canvas, layout.location, transform);
    }
  }
}
