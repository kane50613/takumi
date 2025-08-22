use std::io::{Seek, Write};

use image::{ExtendedColorType, ImageFormat, RgbaImage, codecs::jpeg::JpegEncoder};
use serde::{Deserialize, Serialize};
use taffy::{AvailableSpace, NodeId, Point, TaffyTree, geometry::Size};

use crate::{
  GlobalContext,
  layout::{Viewport, node::Node},
  rendering::FastBlendImage,
};

use super::transform::draw_transformed_node_and_children;
use crate::rendering::RenderContext;

/// Stores the context and node for rendering.
pub(crate) struct NodeRender<'ctx, Nodes: Node<Nodes>> {
  pub(crate) context: RenderContext<'ctx>,
  pub(crate) node: Nodes,
}

/// Output format for the rendered image.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
#[cfg_attr(feature = "wasm", wasm_bindgen::prelude::wasm_bindgen)]
pub enum ImageOutputFormat {
  /// WebP format, suitable for web images with good compression.
  WebP,
  /// PNG format, lossless and supports transparency.
  Png,
  /// JPEG format, lossy compression suitable for photographs.
  Jpeg,
}

impl ImageOutputFormat {
  /// Returns the MIME type for the image output format.
  pub fn content_type(&self) -> &'static str {
    match self {
      ImageOutputFormat::WebP => "image/webp",
      ImageOutputFormat::Png => "image/png",
      ImageOutputFormat::Jpeg => "image/jpeg",
    }
  }
}

impl From<ImageOutputFormat> for ImageFormat {
  fn from(format: ImageOutputFormat) -> Self {
    match format {
      ImageOutputFormat::WebP => Self::WebP,
      ImageOutputFormat::Png => Self::Png,
      ImageOutputFormat::Jpeg => Self::Jpeg,
    }
  }
}

/// Writes the rendered image to the specified destination.
pub fn write_image<T: Write + Seek>(
  image: &RgbaImage,
  destination: &mut T,
  format: ImageOutputFormat,
  jpeg_quality: Option<u8>,
) -> Result<(), image::ImageError> {
  match format {
    ImageOutputFormat::Png | ImageOutputFormat::WebP => {
      image.write_to(destination, format.into())?;
    }
    ImageOutputFormat::Jpeg => {
      // Strip alpha channel into a tightly packed RGB buffer
      let raw = image.as_raw();
      let mut rgb = Vec::with_capacity(raw.len() / 4 * 3);
      for px in raw.chunks_exact(4) {
        rgb.extend_from_slice(&px[..3]);
      }

      let mut encoder = JpegEncoder::new_with_quality(destination, jpeg_quality.unwrap_or(75));
      encoder.encode(&rgb, image.width(), image.height(), ExtendedColorType::Rgb8)?;
    }
  }

  Ok(())
}

/// Renders a node to an image.
pub fn render<Nodes: Node<Nodes>>(
  viewport: Viewport,
  global: &GlobalContext,
  mut root_node: Nodes,
) -> Result<RgbaImage, crate::Error> {
  root_node.inherit_style_for_children();

  let mut taffy = TaffyTree::new();

  let render_context = RenderContext {
    global,
    viewport,
    parent_font_size: viewport.font_size,
  };

  let root_node_id = insert_taffy_node(&mut taffy, root_node, &render_context);

  let mut canvas = FastBlendImage(RgbaImage::new(viewport.width, viewport.height));

  let available_space = Size {
    width: AvailableSpace::Definite(viewport.width as f32),
    height: AvailableSpace::Definite(viewport.height as f32),
  };

  taffy
    .compute_layout_with_measure(
      root_node_id,
      available_space,
      |known_dimensions, available_space, _node_id, node_context, _style| {
        let node = node_context.unwrap();

        if let Size {
          width: Some(width),
          height: Some(height),
        } = known_dimensions
        {
          return Size { width, height };
        }

        node
          .node
          .measure(&node.context, available_space, known_dimensions)
      },
    )
    .unwrap();

  draw_node_recursively(&taffy, root_node_id, &mut canvas, Point::ZERO);

  Ok(canvas.0)
}

fn draw_node_recursively<Nodes: Node<Nodes>>(
  taffy: &TaffyTree<NodeRender<Nodes>>,
  node_id: NodeId,
  canvas: &mut FastBlendImage,
  offset: Point<f32>,
) {
  let node_context = taffy.get_node_context(node_id).unwrap();
  let style = node_context.node.get_style();

  // If there is a transform, draw entire subtree via transform module and return early.
  if style.transform.is_some() {
    draw_transformed_node_and_children(taffy, node_id, canvas, offset);
    return;
  }

  let mut layout = *taffy.layout(node_id).unwrap();
  layout.location.x += offset.x;
  layout.location.y += offset.y;

  node_context
    .node
    .draw_on_canvas(&node_context.context, canvas, layout);

  for child_id in taffy.children(node_id).unwrap() {
    draw_node_recursively(taffy, child_id, canvas, layout.location);
  }
}

fn insert_taffy_node<'ctx, Nodes: Node<Nodes>>(
  taffy: &mut TaffyTree<NodeRender<'ctx, Nodes>>,
  mut node: Nodes,
  render_context: &RenderContext<'ctx>,
) -> NodeId {
  let children = node.take_children();

  let parent_font_size = node
    .get_style()
    .inheritable_style
    .font_size
    .map(|f| f.resolve_to_px(render_context))
    .unwrap_or(render_context.parent_font_size);

  let node_id = taffy
    .new_leaf_with_context(
      node.get_style().resolve_to_taffy_style(render_context),
      NodeRender {
        context: *render_context,
        node,
      },
    )
    .unwrap();

  if let Some(children) = children {
    let render_context = RenderContext {
      global: render_context.global,
      viewport: render_context.viewport,
      parent_font_size,
    };

    let children_ids = children
      .into_iter()
      .map(|child| insert_taffy_node(taffy, child, &render_context))
      .collect::<Vec<_>>();

    taffy.set_children(node_id, &children_ids).unwrap();
  }

  node_id
}
