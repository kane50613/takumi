mod container;
mod image;
mod text;

use ::image::RgbaImage;
pub use container::*;
pub use image::*;
pub use text::*;

use serde::Deserialize;
use taffy::{AvailableSpace, Layout, Point, Size};
use zeno::Fill;

use crate::{
  Result,
  layout::{
    inline::InlineContentKind,
    style::{
      Affine, BackgroundClip, BackgroundImage, BlendMode, CssValue, Length, Sides, Style,
      tw::TailwindValues,
    },
  },
  rendering::{
    BackgroundTile, BorderProperties, Canvas, RenderContext, SizedShadow,
    collect_background_layers, rasterize_layers,
  },
  resources::task::FetchTaskCollection,
};

/// Implements the Node trait for an enum type that contains different node variants.
macro_rules! impl_node_enum {
  ($name:ident, $($variant:ident => $variant_type:ty),*) => {
    impl $crate::layout::node::Node<$name> for $name {
      fn take_children(&mut self) -> Option<Box<[$name]>> {
        match self {
          $( $name::$variant(inner) => inner.take_children(), )*
        }
      }

      fn children_ref(&self) -> Option<&[$name]> {
        match self {
          $( $name::$variant(inner) => inner.children_ref(), )*
        }
      }

      fn take_style_layers(&mut self) -> $crate::layout::node::NodeStyleLayers {
        match self {
          $( $name::$variant(inner) => <_ as $crate::layout::node::Node<$name>>::take_style_layers(inner), )*
        }
      }

      fn inline_content(&self) -> Option<$crate::layout::inline::InlineContentKind<'_>> {
        match self {
          $( $name::$variant(inner) => <_ as $crate::layout::node::Node<$name>>::inline_content(inner), )*
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

      fn draw_outline(&self, context: &$crate::rendering::RenderContext, canvas: &mut $crate::rendering::Canvas, layout: $crate::taffy::Layout) -> $crate::Result<()> {
        match self {
          $( $name::$variant(inner) => <_ as $crate::layout::node::Node<$name>>::draw_outline(inner, context, canvas, layout), )*
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

      fn tag_name(&self) -> Option<&str> {
        match self {
          $( $name::$variant(inner) => <_ as $crate::layout::node::Node<$name>>::tag_name(inner), )*
        }
      }

      fn class_name(&self) -> Option<&str> {
        match self {
          $( $name::$variant(inner) => <_ as $crate::layout::node::Node<$name>>::class_name(inner), )*
        }
      }

      fn id(&self) -> Option<&str> {
        match self {
          $( $name::$variant(inner) => <_ as $crate::layout::node::Node<$name>>::id(inner), )*
        }
      }

      fn get_style(&self) -> Option<&Style> {
        match self {
          $( $name::$variant(inner) => <_ as $crate::layout::node::Node<$name>>::get_style(inner), )*
        }
      }

      fn is_replaced_element(&self) -> bool {
        match self {
          $( $name::$variant(inner) => <_ as $crate::layout::node::Node<$name>>::is_replaced_element(inner), )*
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

      fn draw_background(&self, context: &$crate::rendering::RenderContext, canvas: &mut $crate::rendering::Canvas, layout: $crate::taffy::Layout) -> $crate::Result<()> {
        match self {
          $( $name::$variant(inner) => <_ as $crate::layout::node::Node<$name>>::draw_background(inner, context, canvas, layout), )*
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
  /// Returns the tag name of the node, if any.
  fn tag_name(&self) -> Option<&str> {
    None
  }

  /// Returns the class name of the node, if any.
  fn class_name(&self) -> Option<&str> {
    None
  }

  /// Returns the id of the node, if any.
  fn id(&self) -> Option<&str> {
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

  /// Whether this node behaves like a [CSS replaced element](https://drafts.csswg.org/css-display/#replaced-element)
  /// for sizing purposes.
  fn is_replaced_element(&self) -> bool {
    false
  }

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
  fn take_children(&mut self) -> Option<Box<[N]>> {
    None
  }

  /// Takes the node's local style layers for cascade assembly.
  fn take_style_layers(&mut self) -> NodeStyleLayers;

  /// Retrieve content for inline layout.
  fn inline_content(&self) -> Option<InlineContentKind<'_>> {
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

    let element_border_radius = BorderProperties::from_context(context, layout.size, layout.border);

    for shadow in box_shadow.iter() {
      if shadow.inset {
        continue;
      }

      let mut paths = Vec::new();
      let mut element_paths = Vec::new();

      let mut border_radius = element_border_radius;
      let resolved_spread_radius = shadow
        .spread_radius
        .to_px(&context.sizing, layout.size.width);

      border_radius.expand_by(Sides([resolved_spread_radius; 4]).into());

      let shadow =
        SizedShadow::from_box_shadow(*shadow, &context.sizing, context.current_color, layout.size);

      let spread_size = Size {
        width: (layout.size.width + 2.0 * resolved_spread_radius).max(0.0),
        height: (layout.size.height + 2.0 * resolved_spread_radius).max(0.0),
      };

      border_radius.append_mask_commands(
        &mut paths,
        spread_size,
        Point {
          x: -resolved_spread_radius,
          y: -resolved_spread_radius,
        },
      );

      element_border_radius.append_mask_commands(&mut element_paths, layout.size, Point::ZERO);

      shadow.draw_outset(
        canvas,
        &paths,
        context.transform,
        Fill::NonZero.into(),
        Some(&element_paths),
      )?;
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

        let shadow = SizedShadow::from_box_shadow(
          *shadow,
          &context.sizing,
          context.current_color,
          layout.size,
        );
        shadow.draw_inset(context.transform, border_radius, canvas, layout)?;
      }
    }
    Ok(())
  }

  /// Draws the background image(s) of the node.
  fn draw_background(
    &self,
    context: &RenderContext,
    canvas: &mut Canvas,
    layout: Layout,
  ) -> Result<()> {
    let mut border_radius = BorderProperties::from_context(context, layout.size, layout.border);

    match context.style.background_clip {
      BackgroundClip::BorderBox => {
        let tiles = collect_background_layers(context, layout.size, &mut canvas.buffer_pool)?;

        for tile in tiles {
          for y in &tile.ys {
            for x in &tile.xs {
              canvas.overlay_image(
                &tile.tile,
                border_radius,
                context.transform * Affine::translation(*x as f32, *y as f32),
                context.style.image_rendering,
                tile.blend_mode,
              );
            }
          }
        }
      }
      BackgroundClip::PaddingBox => {
        border_radius.inset_by_border_width();

        let layers = collect_background_layers(context, layout.size, &mut canvas.buffer_pool)?;

        if let Some(tile) = rasterize_layers(
          layers,
          Size {
            width: (layout.size.width - layout.border.left - layout.border.right) as u32,
            height: (layout.size.height - layout.border.top - layout.border.bottom) as u32,
          },
          context,
          border_radius,
          Affine::translation(-layout.border.left, -layout.border.top),
          &mut canvas.mask_memory,
          &mut canvas.buffer_pool,
        )? {
          canvas.overlay_image(
            &tile,
            BorderProperties::default(),
            context.transform * Affine::translation(layout.border.left, layout.border.top),
            context.style.image_rendering,
            BlendMode::Normal,
          );

          if let BackgroundTile::Image(image) = tile {
            canvas.buffer_pool.release_image(image);
          }
        }
      }
      BackgroundClip::ContentBox => {
        border_radius.inset_by_border_width();
        border_radius.expand_by(layout.padding.map(|size| -size));

        let layers = collect_background_layers(context, layout.size, &mut canvas.buffer_pool)?;

        if let Some(tile) = rasterize_layers(
          layers,
          layout.content_box_size().map(|x| x as u32),
          context,
          border_radius,
          Affine::translation(
            -layout.padding.left - layout.border.left,
            -layout.padding.top - layout.border.top,
          ),
          &mut canvas.mask_memory,
          &mut canvas.buffer_pool,
        )? {
          canvas.overlay_image(
            &tile,
            BorderProperties::default(),
            context.transform
              * Affine::translation(
                layout.padding.left + layout.border.left,
                layout.padding.top + layout.border.top,
              ),
            context.style.image_rendering,
            BlendMode::Normal,
          );

          if let BackgroundTile::Image(image) = tile {
            canvas.buffer_pool.release_image(image);
          }
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
    let clip_image = if context.style.background_clip == BackgroundClip::BorderArea {
      rasterize_layers(
        collect_background_layers(context, layout.size, &mut canvas.buffer_pool)?,
        layout.size.map(|x| x as u32),
        context,
        BorderProperties::default(),
        Affine::IDENTITY,
        &mut canvas.mask_memory,
        &mut canvas.buffer_pool,
      )?
    } else {
      None
    };

    BorderProperties::from_context(context, layout.size, layout.border).draw(
      canvas,
      layout.size,
      context.transform,
      clip_image.as_ref(),
    );

    if let Some(BackgroundTile::Image(image)) = clip_image {
      canvas.buffer_pool.release_image(image);
    }
    Ok(())
  }

  /// Draws the outline of the node.
  fn draw_outline(
    &self,
    context: &RenderContext,
    canvas: &mut Canvas,
    layout: Layout,
  ) -> Result<()> {
    let width = context
      .style
      .outline_width
      .unwrap_or(context.style.outline.width)
      .to_px(&context.sizing, layout.size.width)
      .max(0.0);

    let offset = context
      .style
      .outline_offset
      .unwrap_or(Length::zero())
      .to_px(&context.sizing, layout.size.width);

    let mut border = BorderProperties {
      width: Sides([width; 4]).into(),
      color: context
        .style
        .outline_color
        .unwrap_or(context.style.outline.color)
        .resolve(context.current_color),
      style: context
        .style
        .outline_style
        .unwrap_or(context.style.outline.style),
      image_rendering: context.style.image_rendering,
      radius: BorderProperties::resolve_radius_part(context, layout.size),
    };

    border.expand_by(Sides([offset + width; 4]).into());

    let transform = Affine::translation(-offset - width, -offset - width) * context.transform;
    let size = layout.size.map(|x| x + (offset + width) * 2.0);

    border.draw::<RgbaImage>(canvas, size, transform, None);

    Ok(())
  }
}

/// Style layers contributed by a node before cascade/inheritance assembly.
#[derive(Debug, Default, Clone)]
pub struct NodeStyleLayers {
  /// UA/default style preset for the element.
  pub preset: Option<Style>,
  /// Tailwind-derived author style for the element.
  pub author_tw: Option<TailwindValues>,
  /// Inline style attached directly to the element.
  pub inline: Option<Style>,
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
