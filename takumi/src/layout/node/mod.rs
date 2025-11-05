mod container;
mod image;
mod text;

pub use container::*;
pub use image::*;
pub use text::*;

use serde::Deserialize;
use taffy::{AvailableSpace, Layout, Point, Size};
use zeno::Mask;

use crate::{
  layout::{
    inline::InlineContentKind,
    style::{BackgroundImage, CssOption, CssValue, InheritedStyle, Style, tw::TailwindProperties},
  },
  rendering::{
    BorderProperties, Canvas, RenderContext, SizedShadow, draw_background_layers, draw_border,
    resolve_layers_tiles,
  },
  resources::task::FetchTaskCollection,
};

/// Implements the Node trait for an enum type that contains different node variants.
macro_rules! impl_node_enum {
  ($name:ident, $($variant:ident => $variant_type:ty),*) => {
    impl $crate::layout::node::Node<$name> for $name {
      fn take_children(&mut self) -> Option<Vec<$name>> {
        match self {
          $( $name::$variant(inner) => inner.take_children(), )*
        }
      }

      fn children_ref(&self) -> Option<&[$name]> {
        match self {
          $( $name::$variant(inner) => inner.children_ref(), )*
        }
      }

      fn create_inherited_style(&mut self, parent: &$crate::layout::style::InheritedStyle) -> $crate::layout::style::InheritedStyle {
        match self {
          $( $name::$variant(inner) => <_ as $crate::layout::node::Node<$name>>::create_inherited_style(inner, parent), )*
        }
      }

      fn inline_content(&self, context: &$crate::rendering::RenderContext) -> Option<$crate::layout::inline::InlineContentKind> {
        match self {
          $( $name::$variant(inner) => <_ as $crate::layout::node::Node<$name>>::inline_content(inner, context), )*
        }
      }

      fn get_tailwind_properties(&self) -> Option<&TailwindProperties> {
        match self {
          $( $name::$variant(inner) => <_ as $crate::layout::node::Node<$name>>::get_tailwind_properties(inner), )*
        }
      }

      fn measure(
        &self,
        context: &$crate::rendering::RenderContext,
        available_space: $crate::taffy::Size<$crate::taffy::AvailableSpace>,
        known_dimensions: $crate::taffy::Size<Option<f32>>,
        style: &taffy::Style,
      ) -> $crate::taffy::Size<f32> {
        match self {
          $( $name::$variant(inner) => <_ as $crate::layout::node::Node<$name>>::measure(inner, context, available_space, known_dimensions, style), )*
        }
      }

      fn draw_on_canvas(&self, context: &$crate::rendering::RenderContext, canvas: &mut $crate::rendering::Canvas, layout: $crate::taffy::Layout) {
        match self {
          $( $name::$variant(inner) => <_ as $crate::layout::node::Node<$name>>::draw_on_canvas(inner, context, canvas, layout), )*
        }
      }

      fn draw_background_color(&self, context: &$crate::rendering::RenderContext, canvas: &mut $crate::rendering::Canvas, layout: $crate::taffy::Layout) {
        match self {
          $( $name::$variant(inner) => <_ as $crate::layout::node::Node<$name>>::draw_background_color(inner, context, canvas, layout), )*
        }
      }

      fn draw_content(&self, context: &$crate::rendering::RenderContext, canvas: &mut $crate::rendering::Canvas, layout: $crate::taffy::Layout) {
        match self {
          $( $name::$variant(inner) => <_ as $crate::layout::node::Node<$name>>::draw_content(inner, context, canvas, layout), )*
        }
      }

      fn draw_border(&self, context: &$crate::rendering::RenderContext, canvas: &mut $crate::rendering::Canvas, layout: $crate::taffy::Layout) {
        match self {
          $( $name::$variant(inner) => <_ as $crate::layout::node::Node<$name>>::draw_border(inner, context, canvas, layout), )*
        }
      }

      fn draw_outset_box_shadow(&self, context: &$crate::rendering::RenderContext, canvas: &mut $crate::rendering::Canvas, layout: $crate::taffy::Layout) {
        match self {
          $( $name::$variant(inner) => <_ as $crate::layout::node::Node<$name>>::draw_outset_box_shadow(inner, context, canvas, layout), )*
        }
      }

      fn draw_inset_box_shadow(&self, context: &$crate::rendering::RenderContext, canvas: &mut $crate::rendering::Canvas, layout: $crate::taffy::Layout) {
        match self {
          $( $name::$variant(inner) => <_ as $crate::layout::node::Node<$name>>::draw_inset_box_shadow(inner, context, canvas, layout), )*
        }
      }

      fn get_style(&self) -> Option<&Style> {
        match self {
          $( $name::$variant(inner) => <_ as $crate::layout::node::Node<$name>>::get_style(inner), )*
        }
      }

      fn collect_fetch_tasks(&self, collection: &mut FetchTaskCollection) {
        match self {
          $( $name::$variant(inner) => <_ as $crate::layout::node::Node<$name>>::collect_fetch_tasks(inner, collection), )*
        }
      }

      fn collect_style_fetch_tasks(&self, collection: &mut FetchTaskCollection) {
        match self {
          $( $name::$variant(inner) => <_ as $crate::layout::node::Node<$name>>::collect_style_fetch_tasks(inner, collection), )*
        }
      }

      fn draw_background_image(&self, context: &$crate::rendering::RenderContext, canvas: &mut $crate::rendering::Canvas, layout: $crate::taffy::Layout) {
        match self {
          $( $name::$variant(inner) => <_ as $crate::layout::node::Node<$name>>::draw_background_image(inner, context, canvas, layout), )*
        }
      }
    }

    $(
      impl From<$variant_type> for $name {
        fn from(inner: $variant_type) -> Self {
          $name::$variant(inner)
        }
      }
    )*
  };
}

/// A trait representing a node in the layout tree.
///
/// This trait defines the common interface for all elements that can be
/// rendered in the layout system, including containers, text, and images.
pub trait Node<N: Node<N>>: Send + Sync + Clone {
  /// Get the tailwind property of the node.
  fn get_tailwind_properties(&self) -> Option<&TailwindProperties> {
    None
  }

  /// Gets reference of children.
  fn children_ref(&self) -> Option<&[N]> {
    None
  }

  /// Creates resolving tasks for node's http resources.
  fn collect_fetch_tasks(&self, collection: &mut FetchTaskCollection) {
    let Some(children) = self.children_ref() else {
      return;
    };

    for child in children {
      child.collect_fetch_tasks(collection);
    }
  }

  /// Returns a reference to this node's raw [`Style`], if any.
  fn get_style(&self) -> Option<&Style>;

  /// Creates resolving tasks for style's http resources.
  fn collect_style_fetch_tasks(&self, collection: &mut FetchTaskCollection) {
    if let Some(style) = self.get_style() {
      if let CssValue::Value(CssOption(Some(images))) = &style.background_image {
        collection.insert_many(images.0.iter().filter_map(|image| {
          if let BackgroundImage::Url(url) = image {
            Some(url.clone())
          } else {
            None
          }
        }))
      };

      if let CssValue::Value(CssOption(Some(images))) = &style.mask_image {
        collection.insert_many(images.0.iter().filter_map(|image| {
          if let BackgroundImage::Url(url) = image {
            Some(url.clone())
          } else {
            None
          }
        }));
      };
    };

    let Some(children) = self.children_ref() else {
      return;
    };

    for child in children {
      child.collect_fetch_tasks(collection);
    }
  }

  /// Return reference to children nodes.
  fn take_children(&mut self) -> Option<Vec<N>> {
    None
  }

  /// Create a [`InheritedStyle`] instance or clone the parent's.
  fn create_inherited_style(&mut self, _parent: &InheritedStyle) -> InheritedStyle;

  /// Retrieve content for inline layout.
  fn inline_content(&self, _context: &RenderContext) -> Option<InlineContentKind> {
    None
  }

  /// Measures content size of this node.
  fn measure(
    &self,
    _context: &RenderContext,
    _available_space: Size<AvailableSpace>,
    _known_dimensions: Size<Option<f32>>,
    _style: &taffy::Style,
  ) -> Size<f32> {
    Size::ZERO
  }

  /// Draws the node onto the canvas using the computed layout.
  fn draw_on_canvas(&self, context: &RenderContext, canvas: &mut Canvas, layout: Layout) {
    self.draw_outset_box_shadow(context, canvas, layout);
    self.draw_background_color(context, canvas, layout);
    self.draw_background_image(context, canvas, layout);
    self.draw_inset_box_shadow(context, canvas, layout);
    self.draw_border(context, canvas, layout);
    self.draw_content(context, canvas, layout);
  }

  /// Draws the outset box shadow of the node.
  fn draw_outset_box_shadow(&self, context: &RenderContext, canvas: &mut Canvas, layout: Layout) {
    if let Some(box_shadow) = context.style.box_shadow.as_ref() {
      let border_radius = BorderProperties::from_context(context, &layout);

      for shadow in box_shadow.0.iter() {
        if shadow.inset {
          continue;
        }

        let mut paths = Vec::new();

        let shadow = SizedShadow::from_box_shadow(*shadow, context, layout.size);

        border_radius
          .expand_by(shadow.spread_radius)
          .append_mask_commands(&mut paths);
        border_radius.transform.apply_on_paths(&mut paths);

        context.transform.apply_on_paths(&mut paths);

        let (mask, placement) = Mask::new(&paths).render();

        shadow.draw_outset(canvas, mask.into(), placement, layout.location);
      }
    }
  }

  /// Draws the inset box shadow of the node.
  fn draw_inset_box_shadow(&self, context: &RenderContext, canvas: &mut Canvas, layout: Layout) {
    if let Some(box_shadow) = context.style.box_shadow.as_ref() {
      let border_radius = BorderProperties::from_context(context, &layout);

      for shadow in box_shadow.0.iter() {
        if !shadow.inset {
          continue;
        }

        let shadow = SizedShadow::from_box_shadow(*shadow, context, layout.size);
        shadow.draw_inset(context.transform, border_radius, canvas, layout);
      }
    }
  }

  /// Draws the background color of the node.
  fn draw_background_color(&self, context: &RenderContext, canvas: &mut Canvas, layout: Layout) {
    let radius = BorderProperties::from_context(context, &layout);

    canvas.fill_color(
      Point {
        x: layout.location.x as i32,
        y: layout.location.y as i32,
      },
      Size {
        width: layout.size.width as u32,
        height: layout.size.height as u32,
      },
      context
        .style
        .background_color
        .resolve(context.current_color, context.opacity),
      radius,
      context.transform,
    );
  }

  /// Draws the background image(s) of the node.
  fn draw_background_image(&self, context: &RenderContext, canvas: &mut Canvas, layout: Layout) {
    let Some(background_image) = context.style.background_image.as_ref() else {
      return;
    };

    let tiles = resolve_layers_tiles(
      background_image,
      context.style.background_position.as_ref(),
      context.style.background_size.as_ref(),
      context.style.background_repeat.as_ref(),
      context,
      layout,
    );

    draw_background_layers(
      tiles,
      BorderProperties::from_context(context, &layout).inset_by_border_width(),
      context,
      canvas,
      layout,
    );
  }

  /// Draws the main content of the node.
  fn draw_content(&self, _context: &RenderContext, _canvas: &mut Canvas, _layout: Layout) {
    // Default implementation does nothing
  }

  /// Draws the border of the node.
  fn draw_border(&self, context: &RenderContext, canvas: &mut Canvas, layout: Layout) {
    draw_border(
      canvas,
      layout.location,
      BorderProperties::from_context(context, &layout),
    );
  }
}

/// Represents the nodes enum.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum NodeKind {
  /// A node that contains other nodes.
  Container(ContainerNode<NodeKind>),
  /// A node that displays an image.
  Image(ImageNode),
  /// A node that displays text.
  Text(TextNode),
}

impl_node_enum!(
  NodeKind,
  Container => ContainerNode<NodeKind>,
  Image => ImageNode,
  Text => TextNode
);
