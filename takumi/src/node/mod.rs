/// Module for border drawing operations
pub mod border;
/// Module for drawing operations on canvas
pub mod draw;
/// Module for measuring text and image dimensions
pub mod measure;
/// Module for styling and layout properties
pub mod style;

/// Macros for node implementations
pub mod macros;

use std::fmt::Debug;
use std::sync::{Arc, OnceLock};

use async_trait::async_trait;
use futures_util::future::join_all;
use merge::Merge;
use serde::{Deserialize, Serialize};
use taffy::{AvailableSpace, Layout, Size};

use crate::border_radius::BorderRadius;
use crate::box_shadow::draw_box_shadow;
use crate::context::GlobalContext;
use crate::node::draw::{FastBlendImage, draw_background_color};
use crate::node::{
  border::draw_border,
  draw::{ImageState, draw_image, draw_text},
  measure::{measure_image, measure_text},
  style::Style,
};
use crate::render::RenderContext;

/// A trait representing a node in the layout tree.
///
/// This trait defines the common interface for all elements that can be
/// rendered in the layout system, including containers, text, and images.
#[async_trait]
pub trait Node<N: Node<N>>: Send + Sync + Debug + Clone {
  /// Return reference to children nodes.
  fn get_children(&self) -> Option<Vec<&N>> {
    None
  }

  /// Returns a reference to the node's style properties.
  fn get_style(&self) -> &Style;

  /// Returns a mutable reference to the node's style properties.
  fn get_style_mut(&mut self) -> &mut Style;

  /// Inherits style properties from a parent node.
  ///
  /// This method merges inheritable style properties from the parent
  /// into this node's style, then propagates the inheritance to children.
  fn inherit_style(&mut self, parent: &Style) {
    let style = self.get_style_mut();

    style
      .inheritable_style
      .merge(parent.inheritable_style.clone());

    self.inherit_style_for_children();
  }

  /// Called after the style is inherited and before the layout is computed.
  ///
  /// You can use this method to modify the node's style before the layout is computed.
  fn before_layout(&mut self) {}

  /// Propagates style inheritance to child nodes.
  ///
  /// Override this method in container nodes to pass styles to children.
  fn inherit_style_for_children(&mut self) {}

  /// Returns true if this node requires async hydration before rendering.
  ///
  /// Used for nodes that need to load external resources like images.
  fn should_hydrate_async(&self) -> bool {
    false
  }

  /// Performs async hydration of the node.
  ///
  /// This method is called for nodes that return true from `should_hydrate_async()`
  /// to load external resources before rendering.
  async fn hydrate_async(&self, _context: &GlobalContext) {}

  /// Measures the intrinsic size of the node.
  ///
  /// This method calculates the size the node would prefer given
  /// the available space and any known dimensions.
  fn measure(
    &self,
    _context: &RenderContext,
    _available_space: Size<AvailableSpace>,
    _known_dimensions: Size<Option<f32>>,
  ) -> Size<f32> {
    Size::ZERO
  }

  /// Draws the node onto the canvas using the computed layout.
  fn draw_on_canvas(&self, context: &RenderContext, canvas: &mut FastBlendImage, layout: Layout) {
    self.draw_box_shadow(context, canvas, layout);
    self.draw_background(context, canvas, layout);
    self.draw_content(context, canvas, layout);
    self.draw_border(context, canvas, layout);
  }

  /// Draws the box shadow of the node.
  fn draw_box_shadow(&self, context: &RenderContext, canvas: &mut FastBlendImage, layout: Layout) {
    if let Some(box_shadow) = &self.get_style().box_shadow {
      draw_box_shadow(box_shadow, self.get_style(), context, canvas, layout);
    }
  }

  /// Draws the background of the node.
  fn draw_background(&self, context: &RenderContext, canvas: &mut FastBlendImage, layout: Layout) {
    if let Some(background_color) = &self.get_style().background_color {
      let radius = self
        .get_style()
        .inheritable_style
        .border_radius
        .map(|radius| BorderRadius::from_layout(context, &layout, radius.into()));

      draw_background_color(background_color, radius, canvas, layout);
    }
  }

  /// Draws the main content of the node.
  fn draw_content(&self, _context: &RenderContext, _canvas: &mut FastBlendImage, _layout: Layout) {
    // Default implementation does nothing
  }

  /// Draws the border of the node.
  fn draw_border(&self, context: &RenderContext, canvas: &mut FastBlendImage, layout: Layout) {
    draw_border(context, self.get_style(), canvas, &layout);
  }
}

/// A container node that can hold child nodes.
///
/// Container nodes are used to group other nodes and apply layout
/// properties like flexbox layout to arrange their children.
#[derive(Debug, Deserialize, Clone, Serialize)]
pub struct ContainerNode<Nodes: Node<Nodes>> {
  /// The styling properties for this container
  #[serde(default, flatten)]
  pub style: Style,
  /// The child nodes contained within this container
  pub children: Option<Vec<Nodes>>,
}

#[async_trait]
impl<Nodes: Node<Nodes>> Node<Nodes> for ContainerNode<Nodes> {
  fn get_children(&self) -> Option<Vec<&Nodes>> {
    self
      .children
      .as_ref()
      .map(|children| children.iter().collect())
  }

  fn get_style(&self) -> &Style {
    &self.style
  }

  fn get_style_mut(&mut self) -> &mut Style {
    &mut self.style
  }

  fn should_hydrate_async(&self) -> bool {
    self
      .children
      .as_ref()
      .map(|children| children.iter().any(|child| child.should_hydrate_async()))
      .unwrap_or(false)
  }

  async fn hydrate_async(&self, context: &GlobalContext) {
    let Some(children) = &self.children else {
      return;
    };

    let futures = children.iter().filter_map(|child| {
      if child.should_hydrate_async() {
        Some(child.hydrate_async(context))
      } else {
        None
      }
    });

    join_all(futures).await;
  }

  fn inherit_style_for_children(&mut self) {
    let style = self.get_style().clone();

    let Some(children) = &mut self.children else {
      return;
    };

    for child in children.iter_mut() {
      child.inherit_style(&style);
    }
  }
}

/// A node that renders text content.
///
/// Text nodes display text with configurable font properties,
/// alignment, and styling options.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TextNode {
  /// The styling properties for this text node
  #[serde(default, flatten)]
  pub style: Style,
  /// The text content to be rendered
  pub text: String,
}

impl<Nodes: Node<Nodes>> Node<Nodes> for TextNode {
  fn get_style(&self) -> &Style {
    &self.style
  }

  fn get_style_mut(&mut self) -> &mut Style {
    &mut self.style
  }

  fn draw_content(&self, context: &RenderContext, canvas: &mut FastBlendImage, layout: Layout) {
    draw_text(
      &self.text,
      &self.style.resolve_to_font_style(context),
      context,
      canvas,
      layout,
    );
  }

  fn measure(
    &self,
    context: &RenderContext,
    available_space: Size<AvailableSpace>,
    known_dimensions: Size<Option<f32>>,
  ) -> Size<f32> {
    measure_text(
      context,
      &self.text,
      &self.style.resolve_to_font_style(context),
      known_dimensions,
      available_space,
    )
  }
}

/// A node that renders image content.
///
/// Image nodes display images loaded from URLs or file paths,
/// with support for async loading and caching.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ImageNode {
  /// The styling properties for this image node
  #[serde(default, flatten)]
  pub style: Style,
  /// The source URL or path to the image
  pub src: String,
  /// The cached image state (not serialized)
  #[serde(skip)]
  pub image: Arc<OnceLock<Arc<ImageState>>>,
}

#[async_trait]
impl<Nodes: Node<Nodes>> Node<Nodes> for ImageNode {
  fn get_style(&self) -> &Style {
    &self.style
  }

  fn get_style_mut(&mut self) -> &mut Style {
    &mut self.style
  }

  fn should_hydrate_async(&self) -> bool {
    self.image.get().is_none()
  }

  async fn hydrate_async(&self, context: &GlobalContext) {
    let image_store = &context.image_store;

    if let Some(img) = image_store.get(&self.src) {
      self.image.set(img).unwrap();
      return;
    }

    let img = image_store.fetch_async(&self.src).await;

    image_store.insert(self.src.clone(), img.clone());
    self.image.set(img).unwrap();
  }

  fn measure(
    &self,
    _context: &RenderContext,
    available_space: Size<AvailableSpace>,
    known_dimensions: Size<Option<f32>>,
  ) -> Size<f32> {
    let ImageState::Fetched(image) = self.image.get().unwrap().as_ref() else {
      return Size::ZERO;
    };

    let (width, height) = image.dimensions();

    measure_image(
      Size {
        width: width as f32,
        height: height as f32,
      },
      known_dimensions,
      available_space,
    )
  }

  fn draw_content(&self, context: &RenderContext, canvas: &mut FastBlendImage, layout: Layout) {
    let ImageState::Fetched(image) = self.image.get().unwrap().as_ref() else {
      return;
    };

    draw_image(image, &self.style, context, canvas, layout);
  }
}
