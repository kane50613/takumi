mod container;
mod image;
mod text;

pub use container::*;
pub use image::*;
pub use text::*;

use serde::Deserialize;
use taffy::{AvailableSpace, Layout, Point, Size};
use zeno::Fill;

use crate::{
  Result,
  layout::{
    Viewport,
    inline::InlineContentKind,
    style::{Affine, BackgroundClip, BackgroundImage, CssValue, InheritedStyle, Sides, Style},
  },
  rendering::{
    BorderProperties, Canvas, RenderContext, SizedShadow, collect_background_image_tiles,
    create_background_image, draw_background_layers,
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

      fn create_inherited_style(&mut self, parent: &$crate::layout::style::InheritedStyle, viewport: $crate::layout::Viewport) -> $crate::layout::style::InheritedStyle {
        match self {
          $( $name::$variant(inner) => <_ as $crate::layout::node::Node<$name>>::create_inherited_style(inner, parent, viewport), )*
        }
      }

      fn inline_content(&self, context: &$crate::rendering::RenderContext) -> Option<$crate::layout::inline::InlineContentKind> {
        match self {
          $( $name::$variant(inner) => <_ as $crate::layout::node::Node<$name>>::inline_content(inner, context), )*
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

      fn draw_background_color(&self, context: &$crate::rendering::RenderContext, canvas: &mut $crate::rendering::Canvas, layout: $crate::taffy::Layout) -> $crate::Result<()> {
        match self {
          $( $name::$variant(inner) => <_ as $crate::layout::node::Node<$name>>::draw_background_color(inner, context, canvas, layout), )*
        }
      }

      fn draw_content(&self, context: &$crate::rendering::RenderContext, canvas: &mut $crate::rendering::Canvas, layout: $crate::taffy::Layout) -> $crate::Result<()> {
        match self {
          $( $name::$variant(inner) => <_ as $crate::layout::node::Node<$name>>::draw_content(inner, context, canvas, layout), )*
        }
      }

      fn draw_border(&self, context: &$crate::rendering::RenderContext, canvas: &mut $crate::rendering::Canvas, layout: $crate::taffy::Layout) -> $crate::Result<()> {
        match self {
          $( $name::$variant(inner) => <_ as $crate::layout::node::Node<$name>>::draw_border(inner, context, canvas, layout), )*
        }
      }

      fn draw_outset_box_shadow(&self, context: &$crate::rendering::RenderContext, canvas: &mut $crate::rendering::Canvas, layout: $crate::taffy::Layout) -> $crate::Result<()> {
        match self {
          $( $name::$variant(inner) => <_ as $crate::layout::node::Node<$name>>::draw_outset_box_shadow(inner, context, canvas, layout), )*
        }
      }

      fn draw_inset_box_shadow(&self, context: &$crate::rendering::RenderContext, canvas: &mut $crate::rendering::Canvas, layout: $crate::taffy::Layout) -> $crate::Result<()> {
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

      fn draw_background_image(&self, context: &$crate::rendering::RenderContext, canvas: &mut $crate::rendering::Canvas, layout: $crate::taffy::Layout) -> $crate::Result<()> {
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
      if let CssValue::Value(Some(images)) = &style.background_image {
        collection.insert_many(images.iter().filter_map(|image| {
          if let BackgroundImage::Url(url) = image {
            Some(url.clone())
          } else {
            None
          }
        }))
      };

      if let CssValue::Value(background) = &style.background {
        collection.insert_many(background.iter().filter_map(|background| {
          if let BackgroundImage::Url(url) = &background.image {
            Some(url.clone())
          } else {
            None
          }
        }));
      };

      if let CssValue::Value(Some(images)) = &style.mask_image {
        collection.insert_many(images.iter().filter_map(|image| {
          if let BackgroundImage::Url(url) = image {
            Some(url.clone())
          } else {
            None
          }
        }));
      };

      if let CssValue::Value(mask) = &style.mask {
        collection.insert_many(mask.iter().filter_map(|background| {
          if let BackgroundImage::Url(url) = &background.image {
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
  fn create_inherited_style(
    &mut self,
    _parent: &InheritedStyle,
    viewport: Viewport,
  ) -> InheritedStyle;

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

  /// Draws the outset box shadow of the node.
  fn draw_outset_box_shadow(
    &self,
    context: &RenderContext,
    canvas: &mut Canvas,
    layout: Layout,
  ) -> Result<()> {
    let Some(box_shadow) = context.style.box_shadow.as_ref() else {
      return Ok(());
    };

    let border_radius = BorderProperties::from_context(context, layout.size, layout.border);

    for shadow in box_shadow.iter() {
      if shadow.inset {
        continue;
      }

      let mut paths = Vec::new();

      let mut border_radius = border_radius;
      let resolved_spread_radius = shadow
        .spread_radius
        .resolve_to_px(context, layout.size.width)
        .max(0.0);

      border_radius.expand_by(Sides([resolved_spread_radius; 4]).into());

      let shadow = SizedShadow::from_box_shadow(*shadow, context, layout.size);

      border_radius.append_mask_commands(
        &mut paths,
        layout.size,
        Point {
          x: -shadow.spread_radius,
          y: -shadow.spread_radius,
        },
      );

      shadow.draw_outset(
        &mut canvas.image,
        &mut canvas.mask_memory,
        canvas.constrains.last(),
        &paths,
        context.transform,
        Fill::EvenOdd.into(),
      );
    }

    Ok(())
  }

  /// Draws the inset box shadow of the node.
  fn draw_inset_box_shadow(
    &self,
    context: &RenderContext,
    canvas: &mut Canvas,
    layout: Layout,
  ) -> Result<()> {
    if let Some(box_shadow) = context.style.box_shadow.as_ref() {
      let border_radius = BorderProperties::from_context(context, layout.size, layout.border);

      for shadow in box_shadow.iter() {
        if !shadow.inset {
          continue;
        }

        let shadow = SizedShadow::from_box_shadow(*shadow, context, layout.size);
        shadow.draw_inset(context.transform, border_radius, canvas, layout);
      }
    }
    Ok(())
  }

  /// Draws the background color of the node.
  fn draw_background_color(
    &self,
    context: &RenderContext,
    canvas: &mut Canvas,
    layout: Layout,
  ) -> Result<()> {
    let mut radius = BorderProperties::from_context(context, layout.size, layout.border);

    match context.style.background_clip {
      BackgroundClip::BorderBox => {
        canvas.fill_color(
          layout.size,
          context
            .style
            .background_color
            .resolve(context.current_color, context.opacity),
          radius,
          context.transform,
        );
      }
      BackgroundClip::PaddingBox => {
        radius.inset_by_border_width();

        canvas.fill_color(
          Size {
            width: layout.size.width - layout.border.left - layout.border.right,
            height: layout.size.height - layout.border.top - layout.border.bottom,
          },
          context
            .style
            .background_color
            .resolve(context.current_color, context.opacity),
          radius,
          Affine::translation(layout.border.left, layout.border.top) * context.transform,
        );
      }
      BackgroundClip::ContentBox => {
        radius.inset_by_border_width();
        radius.expand_by(layout.padding.map(|size| -size));

        canvas.fill_color(
          layout.content_box_size(),
          context
            .style
            .background_color
            .resolve(context.current_color, context.opacity),
          radius,
          Affine::translation(
            layout.padding.left + layout.border.left,
            layout.padding.top + layout.border.top,
          ) * context.transform,
        );
      }
      _ => {}
    }

    Ok(())
  }

  /// Draws the background image(s) of the node.
  fn draw_background_image(
    &self,
    context: &RenderContext,
    canvas: &mut Canvas,
    layout: Layout,
  ) -> Result<()> {
    let mut border_radius = BorderProperties::from_context(context, layout.size, layout.border);

    match context.style.background_clip {
      BackgroundClip::BorderBox => {
        let tiles = collect_background_image_tiles(context, layout.size)?;

        draw_background_layers(tiles, border_radius, context, canvas);
      }
      BackgroundClip::PaddingBox => {
        border_radius.inset_by_border_width();

        if let Some(image) = create_background_image(
          context,
          layout.size,
          Size {
            width: layout.size.width - layout.border.left - layout.border.right,
            height: layout.size.height - layout.border.top - layout.border.bottom,
          },
          Point {
            x: layout.border.left,
            y: layout.border.top,
          },
          &mut canvas.mask_memory,
        )? {
          canvas.overlay_image(
            image.into(),
            border_radius,
            Affine::translation(layout.border.left, layout.border.top) * context.transform,
            context.style.image_rendering,
            context.style.filter.as_ref(),
            context.opacity,
          );
        }
      }
      BackgroundClip::ContentBox => {
        border_radius.inset_by_border_width();
        border_radius.expand_by(layout.padding.map(|size| -size));

        if let Some(image) = create_background_image(
          context,
          layout.size,
          layout.content_box_size(),
          Point {
            x: layout.padding.left + layout.border.left,
            y: layout.padding.top + layout.border.top,
          },
          &mut canvas.mask_memory,
        )? {
          canvas.overlay_image(
            image.into(),
            border_radius,
            Affine::translation(
              layout.padding.left + layout.border.left,
              layout.padding.top + layout.border.top,
            ) * context.transform,
            context.style.image_rendering,
            context.style.filter.as_ref(),
            context.opacity,
          );
        }
      }
      _ => {}
    }

    Ok(())
  }

  /// Draws the main content of the node.
  fn draw_content(
    &self,
    _context: &RenderContext,
    _canvas: &mut Canvas,
    _layout: Layout,
  ) -> Result<()> {
    // Default implementation does nothing
    Ok(())
  }

  /// Draws the border of the node.
  fn draw_border(
    &self,
    context: &RenderContext,
    canvas: &mut Canvas,
    layout: Layout,
  ) -> Result<()> {
    let fill_image = if context.style.background_clip == BackgroundClip::BorderArea {
      create_background_image(
        context,
        layout.size,
        layout.size,
        Point::zero(),
        &mut canvas.mask_memory,
      )?
      .map(Into::into)
    } else {
      None
    };

    BorderProperties::from_context(context, layout.size, layout.border).draw(
      canvas,
      layout.size,
      context.transform,
      fill_image,
    );
    Ok(())
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
