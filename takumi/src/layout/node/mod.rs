mod container;
mod image;
mod text;

use ::image::RgbaImage;
use serde::Deserialize;
use std::collections::BTreeMap;
use std::sync::Arc;
use taffy::{AvailableSpace, Layout, Point, Size};
use zeno::Fill;

use crate::{
  Result,
  layout::{
    Viewport,
    inline::InlineContentKind,
    style::{Affine, BackgroundClip, BlendMode, Sides, Style, tw::TailwindValues},
  },
  rendering::{
    BackgroundTile, BorderProperties, Canvas, RenderContext, SizedShadow,
    collect_background_layers, rasterize_layers,
  },
  resources::task::FetchTaskCollection,
};

use self::{
  container::{
    container_children_ref, deserialize_children, drop_container_children, take_container_children,
    take_container_style_layers,
  },
  image::{
    draw_image_node_content, image_collect_fetch_tasks, image_inline_content, measure_image_node,
    take_image_style_layers,
  },
  text::{draw_text_node_content, measure_text_node, take_text_style_layers, text_inline_content},
};

pub(crate) use self::image::resolve_image;

/// Shared metadata stored by every renderable node.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct NodeMetadata {
  /// The element's tag name.
  pub(crate) tag_name: Option<Box<str>>,
  /// The element's class name.
  pub(crate) class_name: Option<Box<str>>,
  /// The element's id.
  pub(crate) id: Option<Box<str>>,
  /// Additional element attributes for selector matching and serialization.
  pub(crate) attributes: Option<BTreeMap<Box<str>, Box<str>>>,
  /// Default style presets from HTML element type (lowest priority).
  pub(crate) preset: Option<Style>,
  /// The styling properties for this node.
  pub(crate) style: Option<Style>,
  /// The tailwind properties for this node.
  pub(crate) tw: Option<TailwindValues>,
}

#[derive(Debug, Default, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
/// Variant-specific text node data.
pub(crate) struct TextData {
  pub(crate) text: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
/// Variant-specific image node data.
#[non_exhaustive]
pub struct ImageData {
  pub(crate) src: Arc<str>,
  pub(crate) width: Option<f32>,
  pub(crate) height: Option<f32>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
/// A renderable node with shared metadata and variant-specific content.
#[non_exhaustive]
pub struct Node {
  #[serde(flatten)]
  pub(crate) metadata: NodeMetadata,
  #[serde(flatten)]
  pub(crate) kind: NodeKind,
}

/// Represents the nodes enum.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub(crate) enum NodeKind {
  /// A node that contains other nodes.
  Container {
    /// The container child nodes.
    #[serde(default, deserialize_with = "deserialize_children")]
    children: Vec<Node>,
  },
  /// A node that displays an image.
  Image(ImageData),
  /// A node that displays text.
  Text(TextData),
}

impl Default for Node {
  fn default() -> Self {
    Self::container([])
  }
}

impl Drop for Node {
  fn drop(&mut self) {
    drop_container_children(&mut self.kind);
  }
}

impl Node {
  /// Creates a container node with the provided child nodes.
  pub fn container(children: impl Into<Vec<Node>>) -> Self {
    Self {
      metadata: NodeMetadata::default(),
      kind: NodeKind::Container {
        children: children.into(),
      },
    }
  }

  /// Creates an image node from any supported image input shape.
  pub fn image(data: impl Into<ImageData>) -> Self {
    Self {
      metadata: NodeMetadata::default(),
      kind: NodeKind::Image(data.into()),
    }
  }

  /// Creates a text node from the provided text.
  pub fn text(text: impl Into<String>) -> Self {
    Self {
      metadata: NodeMetadata::default(),
      kind: NodeKind::Text(TextData { text: text.into() }),
    }
  }

  pub(crate) fn children_ref(&self) -> Option<&[Node]> {
    container_children_ref(&self.kind)
  }

  pub(crate) fn take_children(&mut self) -> Option<Box<[Node]>> {
    take_container_children(&mut self.kind)
  }

  /// Sets the tag name and returns the updated node.
  pub fn with_tag_name(mut self, tag_name: impl Into<Box<str>>) -> Self {
    self.metadata.tag_name = Some(tag_name.into());
    self
  }

  /// Sets the class name and returns the updated node.
  pub fn with_class_name(mut self, class_name: impl Into<Box<str>>) -> Self {
    self.metadata.class_name = Some(class_name.into());
    self
  }

  /// Sets the element id and returns the updated node.
  pub fn with_id(mut self, id: impl Into<Box<str>>) -> Self {
    self.metadata.id = Some(id.into());
    self
  }

  /// Sets the element attributes and returns the updated node.
  pub fn with_attributes(mut self, attributes: BTreeMap<Box<str>, Box<str>>) -> Self {
    self.metadata.attributes = Some(attributes);
    self
  }

  /// Sets the preset style and returns the updated node.
  pub fn with_preset(mut self, preset: Style) -> Self {
    self.metadata.preset = Some(preset);
    self
  }

  /// Sets the inline style and returns the updated node.
  pub fn with_style(mut self, style: Style) -> Self {
    self.metadata.style = Some(style);
    self
  }

  /// Sets the Tailwind-derived style input and returns the updated node.
  pub fn with_tw(mut self, tw: TailwindValues) -> Self {
    self.metadata.tw = Some(tw);
    self
  }

  pub(crate) fn take_style_layers(&mut self) -> NodeStyleLayers {
    if matches!(self.kind, NodeKind::Container { .. }) {
      return take_container_style_layers(self);
    }

    if let NodeKind::Image(image) = &self.kind {
      return take_image_style_layers(self, image.width, image.height);
    }

    take_text_style_layers(self)
  }

  pub(crate) fn inline_content(&self) -> Option<InlineContentKind<'_>> {
    match &self.kind {
      NodeKind::Container { .. } => None,
      NodeKind::Image(_) => image_inline_content(&self.kind),
      NodeKind::Text(text) => text_inline_content(text),
    }
  }

  pub(crate) fn measure(
    &self,
    context: &RenderContext,
    available_space: Size<AvailableSpace>,
    known_dimensions: Size<Option<f32>>,
    style: &taffy::Style,
  ) -> Size<f32> {
    match &self.kind {
      NodeKind::Container { .. } => Size::ZERO,
      NodeKind::Image(image) => {
        measure_image_node(image, context, available_space, known_dimensions, style)
      }
      NodeKind::Text(text) => measure_text_node(text, context, available_space, known_dimensions),
    }
  }

  /// Collects resource URLs referenced by this node tree.
  pub fn collect_fetch_tasks(&self, collection: &mut FetchTaskCollection) {
    match &self.kind {
      NodeKind::Container { .. } => {
        let Some(children) = self.children_ref() else {
          return;
        };

        for child in children {
          child.collect_fetch_tasks(collection);
        }
      }
      NodeKind::Image(image) => image_collect_fetch_tasks(image, collection),
      NodeKind::Text(_) => {}
    }
  }

  /// Collects resource URLs referenced by this node tree's styles.
  pub fn collect_style_fetch_tasks(&self, collection: &mut FetchTaskCollection) {
    if let Some(preset) = self.metadata.preset.as_ref() {
      preset.collect_fetch_tasks(collection);
    }

    if let Some(author_tw) = self.metadata.tw.as_ref() {
      author_tw.collect_fetch_tasks(Viewport::new(None, None), collection);
    }

    if let Some(inline) = self.metadata.style.as_ref() {
      inline.collect_fetch_tasks(collection);
    }

    let Some(children) = self.children_ref() else {
      return;
    };

    for child in children {
      child.collect_style_fetch_tasks(collection);
    }
  }

  pub(crate) fn is_replaced_element(&self) -> bool {
    matches!(self.kind, NodeKind::Image(_))
  }

  pub(crate) fn draw_outset_box_shadow(
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

  pub(crate) fn draw_inset_box_shadow(
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

  pub(crate) fn draw_background(
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

  pub(crate) fn draw_content(
    &self,
    context: &RenderContext,
    canvas: &mut Canvas,
    layout: Layout,
  ) -> Result<()> {
    match &self.kind {
      NodeKind::Container { .. } => Ok(()),
      NodeKind::Image(image) => draw_image_node_content(image, context, canvas, layout),
      NodeKind::Text(text) => draw_text_node_content(text, context, canvas, layout),
    }
  }

  pub(crate) fn draw_border(
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

  pub(crate) fn draw_outline(
    &self,
    context: &RenderContext,
    canvas: &mut Canvas,
    layout: Layout,
  ) -> Result<()> {
    let width = context
      .style
      .outline_width
      .to_px(&context.sizing, layout.size.width)
      .max(0.0);

    let offset = context
      .style
      .outline_offset
      .to_px(&context.sizing, layout.size.width);

    let mut border = BorderProperties {
      width: Sides([width; 4]).into(),
      color: context.style.outline_color.resolve(context.current_color),
      style: context.style.outline_style,
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
pub(crate) struct NodeStyleLayers {
  /// UA/default style preset for the element.
  pub preset: Option<Style>,
  /// Tailwind-derived author style for the element.
  pub author_tw: Option<TailwindValues>,
  /// Inline style attached directly to the element.
  pub inline: Option<Style>,
}

#[cfg(test)]
mod tests {
  use std::str::FromStr;

  use crate::layout::style::{BackgroundImage, Style, StyleDeclaration, tw::TailwindValues};

  use super::*;

  #[test]
  fn collect_style_fetch_tasks_collects_nested_background_image_urls() {
    let background_url = "https://placehold.co/80x80/22c55e/white";
    let node = Node::container([Node::container([]).with_style(Style::default().with(
      StyleDeclaration::background_image(Some(
        [BackgroundImage::Url(background_url.into())].into(),
      )),
    ))]);

    let mut collection = FetchTaskCollection::default();
    node.collect_style_fetch_tasks(&mut collection);
    let tasks = collection
      .into_inner()
      .iter()
      .map(ToString::to_string)
      .collect::<Vec<_>>();

    assert_eq!(tasks, vec![background_url.to_string()]);
  }

  #[test]
  fn collect_style_fetch_tasks_collects_preset_and_tailwind_image_urls() {
    let preset_url = "https://placehold.co/64x64/f97316/white";
    let tailwind_url = "/bg.png";
    let Ok(tw) = TailwindValues::from_str("bg-[url(/bg.png)]") else {
      unreachable!()
    };
    let node = Node::container([])
      .with_preset(
        Style::default().with(StyleDeclaration::background_image(Some(
          [BackgroundImage::Url(preset_url.into())].into(),
        ))),
      )
      .with_tw(tw);

    let mut collection = FetchTaskCollection::default();
    node.collect_style_fetch_tasks(&mut collection);

    let tasks = collection
      .into_inner()
      .iter()
      .map(ToString::to_string)
      .collect::<Vec<_>>();

    assert_eq!(
      tasks,
      vec![preset_url.to_string(), tailwind_url.to_string()]
    );
  }
}
