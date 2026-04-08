mod container;
mod image;
mod text;

use serde::Deserialize;
use std::collections::BTreeMap;
use std::sync::Arc;
use taffy::{AvailableSpace, Layout, Point, Size};
use tiny_skia::Pixmap;

use crate::{
  Result, Xxh3HashSet,
  layout::{
    Viewport,
    inline::InlineContentKind,
    node::image::image_resource_url,
    style::{Affine, BackgroundClip, BlendMode, Direction, Sides, Style, tw::TailwindValues},
  },
  rendering::{
    BorderProperties, Canvas, Fill, PaintSource, RenderContext, SizedShadow,
    collect_background_layers, rasterize_layers, release_rasterized_background_tile,
  },
  resources::image::{ImageResult, ImageSource},
};
use ::image::RgbaImage;

use self::{
  container::{
    container_children_ref, deserialize_children, drop_container_children, take_container_children,
  },
  image::{
    draw_image_node_content, image_inline_content, measure_image_node, take_image_style_layers,
  },
  text::{draw_text_node_content, measure_text_node, text_inline_content},
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
  /// The text direction for this node.
  pub(crate) dir: Option<Direction>,
}

#[derive(Debug, Default, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
/// Variant-specific text node data.
pub(crate) struct TextData {
  pub(crate) text: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub(crate) enum ImageSourceInput {
  Url(Arc<str>),
  Buffer(Vec<u8>),
  #[serde(skip_deserializing)]
  Loaded(ImageSource),
}

impl ImageSourceInput {
  pub(crate) fn resolve(&self, context: &RenderContext) -> ImageResult {
    match self {
      Self::Url(src) => resolve_image(src, context),
      Self::Buffer(data) => ImageSource::from_bytes(data),
      Self::Loaded(source) => Ok(source.clone()),
    }
  }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
/// Variant-specific image node data.
pub struct ImageData {
  pub(crate) src: ImageSourceInput,
  pub(crate) width: Option<f32>,
  pub(crate) height: Option<f32>,
}

impl From<&str> for ImageData {
  fn from(src: &str) -> Self {
    Self {
      src: ImageSourceInput::Url(src.into()),
      width: None,
      height: None,
    }
  }
}

impl From<String> for ImageData {
  fn from(src: String) -> Self {
    Self {
      src: ImageSourceInput::Url(src.into()),
      width: None,
      height: None,
    }
  }
}

impl From<Arc<str>> for ImageData {
  fn from(src: Arc<str>) -> Self {
    Self {
      src: ImageSourceInput::Url(src),
      width: None,
      height: None,
    }
  }
}

impl From<Vec<u8>> for ImageData {
  fn from(data: Vec<u8>) -> Self {
    Self {
      src: ImageSourceInput::Buffer(data),
      width: None,
      height: None,
    }
  }
}

impl From<&[u8]> for ImageData {
  fn from(data: &[u8]) -> Self {
    Self {
      src: ImageSourceInput::Buffer(data.to_vec()),
      width: None,
      height: None,
    }
  }
}

impl From<ImageSource> for ImageData {
  fn from(source: ImageSource) -> Self {
    Self {
      src: ImageSourceInput::Loaded(source),
      width: None,
      height: None,
    }
  }
}

impl From<Pixmap> for ImageData {
  fn from(pixmap: Pixmap) -> Self {
    Self::from(ImageSource::from(pixmap))
  }
}

impl From<RgbaImage> for ImageData {
  fn from(bitmap: RgbaImage) -> Self {
    Self::from(ImageSource::from(bitmap))
  }
}

impl From<(&str, u32, u32)> for ImageData {
  fn from((src, width, height): (&str, u32, u32)) -> Self {
    Self {
      src: ImageSourceInput::Url(src.into()),
      width: Some(width as f32),
      height: Some(height as f32),
    }
  }
}

impl From<(String, u32, u32)> for ImageData {
  fn from((src, width, height): (String, u32, u32)) -> Self {
    Self {
      src: ImageSourceInput::Url(src.into()),
      width: Some(width as f32),
      height: Some(height as f32),
    }
  }
}

impl From<(Arc<str>, u32, u32)> for ImageData {
  fn from((src, width, height): (Arc<str>, u32, u32)) -> Self {
    Self {
      src: ImageSourceInput::Url(src),
      width: Some(width as f32),
      height: Some(height as f32),
    }
  }
}

impl From<(&str, f32, f32)> for ImageData {
  fn from((src, width, height): (&str, f32, f32)) -> Self {
    Self {
      src: ImageSourceInput::Url(src.into()),
      width: Some(width),
      height: Some(height),
    }
  }
}

impl From<(String, f32, f32)> for ImageData {
  fn from((src, width, height): (String, f32, f32)) -> Self {
    Self {
      src: ImageSourceInput::Url(src.into()),
      width: Some(width),
      height: Some(height),
    }
  }
}

impl From<(Arc<str>, f32, f32)> for ImageData {
  fn from((src, width, height): (Arc<str>, f32, f32)) -> Self {
    Self {
      src: ImageSourceInput::Url(src),
      width: Some(width),
      height: Some(height),
    }
  }
}

impl From<(&str, Option<f32>, Option<f32>)> for ImageData {
  fn from((src, width, height): (&str, Option<f32>, Option<f32>)) -> Self {
    Self {
      src: ImageSourceInput::Url(src.into()),
      width,
      height,
    }
  }
}

impl From<(String, Option<f32>, Option<f32>)> for ImageData {
  fn from((src, width, height): (String, Option<f32>, Option<f32>)) -> Self {
    Self {
      src: ImageSourceInput::Url(src.into()),
      width,
      height,
    }
  }
}

impl From<(Arc<str>, Option<f32>, Option<f32>)> for ImageData {
  fn from((src, width, height): (Arc<str>, Option<f32>, Option<f32>)) -> Self {
    Self {
      src: ImageSourceInput::Url(src),
      width,
      height,
    }
  }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
/// A renderable node with shared metadata and variant-specific content.
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

  /// Sets the direction and returns the updated node.
  pub fn with_dir(mut self, dir: Direction) -> Self {
    self.metadata.dir = Some(dir);
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
    if let NodeKind::Image(image) = &self.kind {
      return take_image_style_layers(self, image.width, image.height);
    }

    NodeStyleLayers {
      preset: self.metadata.preset.take(),
      author_tw: self.metadata.tw.take(),
      inline: self.metadata.style.take(),
      dir: self.metadata.dir.take(),
    }
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
  pub(crate) fn metadata_resource_urls<'a>(&'a self, urls: &mut Xxh3HashSet<&'a str>) {
    match &self.kind {
      NodeKind::Container { .. } => {
        let Some(children) = self.children_ref() else {
          return;
        };

        for child in children {
          child.metadata_resource_urls(urls);
        }
      }
      NodeKind::Image(image) => {
        if let Some(url) = image_resource_url(image) {
          urls.insert(url);
        }
      }
      NodeKind::Text(_) => {}
    }
  }

  /// Collects resource URLs referenced by this node tree's styles.
  pub(crate) fn style_resource_urls<'a>(&'a self, urls: &mut Xxh3HashSet<&'a str>) {
    if let Some(preset) = self.metadata.preset.as_ref() {
      urls.extend(preset.resource_urls());
    }

    if let Some(author_tw) = self.metadata.tw.as_ref() {
      urls.extend(author_tw.resource_urls(Viewport::default()));
    }

    if let Some(inline) = self.metadata.style.as_ref() {
      urls.extend(inline.resource_urls());
    }

    let Some(children) = self.children_ref() else {
      return;
    };

    for child in children {
      child.style_resource_urls(urls);
    }
  }

  /// Collects unique resource URLs referenced by this node tree and styles.
  pub fn resource_urls(&self) -> impl Iterator<Item = &str> {
    let mut urls = Xxh3HashSet::default();
    self.metadata_resource_urls(&mut urls);
    self.style_resource_urls(&mut urls);

    urls.into_iter()
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
        let layers = collect_background_layers(context, layout.size, &mut canvas.buffer_pool)?;

        if border_radius.is_zero() {
          for tile in layers {
            for y in &tile.ys {
              for x in &tile.xs {
                let transform = context.transform * Affine::translation(*x as f32, *y as f32);
                if transform.only_translation()
                  && canvas.overlay_background_tile_direct(
                    &tile.tile,
                    transform.decompose_translation(),
                    tile.blend_mode,
                  )
                {
                  continue;
                }

                canvas.overlay_image(
                  &tile.tile,
                  border_radius,
                  transform,
                  context.style.image_rendering,
                  tile.blend_mode,
                );
              }
            }
          }
        } else if let Some(tile) = rasterize_layers(
          layers,
          layout.size.map(|x| x as u32),
          context,
          BorderProperties::default(),
          Affine::IDENTITY,
          &mut canvas.buffer_pool,
        )? {
          canvas.overlay_image(
            &tile,
            border_radius,
            context.transform,
            context.style.image_rendering,
            BlendMode::Normal,
          );

          release_rasterized_background_tile(tile, &mut canvas.buffer_pool);
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
          &mut canvas.buffer_pool,
        )? {
          canvas.overlay_image(
            &tile,
            BorderProperties::default(),
            context.transform * Affine::translation(layout.border.left, layout.border.top),
            context.style.image_rendering,
            BlendMode::Normal,
          );

          release_rasterized_background_tile(tile, &mut canvas.buffer_pool);
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

          release_rasterized_background_tile(tile, &mut canvas.buffer_pool);
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
        &mut canvas.buffer_pool,
      )?
    } else {
      None
    };

    BorderProperties::from_context(context, layout.size, layout.border).draw(
      canvas,
      layout.size,
      context.transform,
      clip_image.as_ref().map(PaintSource::from),
    );

    if let Some(tile) = clip_image {
      release_rasterized_background_tile(tile, &mut canvas.buffer_pool);
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

    border.draw(canvas, size, transform, None);

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
  pub dir: Option<Direction>,
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

    let mut urls = Xxh3HashSet::default();
    node.style_resource_urls(&mut urls);

    assert_eq!(urls.into_iter().collect::<Vec<_>>(), vec![background_url]);
  }

  #[test]
  fn collect_style_fetch_tasks_collects_preset_and_tailwind_image_urls() {
    let preset_url = "https://placehold.co/64x64/f97316/white";
    let tailwind_url = "/bg.png";
    let Ok(tw) = TailwindValues::from_str("bg-[url(/bg.png)]") else {
      return;
    };
    let node = Node::container([])
      .with_preset(
        Style::default().with(StyleDeclaration::background_image(Some(
          [BackgroundImage::Url(preset_url.into())].into(),
        ))),
      )
      .with_tw(tw);

    let mut urls = Xxh3HashSet::default();
    node.style_resource_urls(&mut urls);

    let tasks = urls.into_iter().collect::<Vec<_>>();

    assert_eq!(tasks, vec![tailwind_url, preset_url]);
  }
}
