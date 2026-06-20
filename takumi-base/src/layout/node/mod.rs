mod container;
mod image;
mod text;

use crate::resources::image_buffer::ImageBuffer;
use serde::Deserialize;
use serde_bytes::ByteBuf;
use std::collections::BTreeMap;
use std::sync::Arc;
use taffy::{AvailableSpace, Size};

use crate::{
  Xxh3HashSet,
  context::RenderContext,
  layout::{
    Viewport,
    inline::InlineContentKind,
    node::image::image_resource_url,
    style::{Direction, Style, StyleDeclaration, ToCss, tw::TailwindValues},
  },
  resources::image::{ImageResult, ImageSource},
};
use ::image::RgbaImage;

use self::{
  container::{
    container_children_ref, deserialize_children, drop_container_children, take_container_children,
  },
  image::{image_inline_content, measure_image_node, take_image_style_layers},
  text::{measure_text_node, text_inline_content},
};

pub use self::image::resolve_image;

/// Shared metadata stored by every renderable node.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct NodeMetadata {
  /// The element's tag name.
  pub tag_name: Option<Box<str>>,
  /// The element's class name.
  pub class_name: Option<Box<str>>,
  /// The element's id.
  pub id: Option<Box<str>>,
  /// Additional element attributes for selector matching and serialization.
  pub attributes: Option<BTreeMap<Box<str>, Box<str>>>,
  /// Default style presets from HTML element type (lowest priority).
  pub preset: Option<Style>,
  /// The styling properties for this node.
  pub style: Option<Style>,
  /// The tailwind properties for this node.
  pub tw: Option<TailwindValues>,
  /// The text direction for this node.
  pub dir: Option<Direction>,
}

#[derive(Debug, Default, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
/// Variant-specific text node data.
pub struct TextData {
  pub text: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum ImageSourceInput {
  Url(Arc<str>),
  // `ByteBuf` (not `Vec<u8>`) so an FFI `Uint8Array`/`ArrayBuffer`, surfaced as a
  // bytes value rather than a number array, deserializes here.
  Buffer(ByteBuf),
  #[serde(skip_deserializing)]
  Loaded(ImageSource),
}

impl ImageSourceInput {
  pub fn resolve(&self, context: &RenderContext) -> ImageResult {
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
  pub src: ImageSourceInput,
  pub width: Option<f32>,
  pub height: Option<f32>,
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
      src: ImageSourceInput::Buffer(ByteBuf::from(data)),
      width: None,
      height: None,
    }
  }
}

impl From<&[u8]> for ImageData {
  fn from(data: &[u8]) -> Self {
    Self {
      src: ImageSourceInput::Buffer(ByteBuf::from(data.to_vec())),
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

impl From<ImageBuffer> for ImageData {
  fn from(buffer: ImageBuffer) -> Self {
    Self::from(ImageSource::from(buffer))
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
  pub kind: NodeKind,
}

/// Represents the nodes enum.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum NodeKind {
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

  pub(crate) fn is_whitespace_only_text(&self) -> bool {
    let NodeKind::Text(data) = &self.kind else {
      return false;
    };
    data
      .text
      .bytes()
      .all(|b| matches!(b, b' ' | b'\t' | b'\n' | b'\r' | 0x0C))
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

  // Internal, do not use in production.
  #[doc(hidden)]
  pub fn to_html(&self) -> String {
    let tag = self
      .metadata
      .tag_name
      .as_deref()
      .unwrap_or(match &self.kind {
        NodeKind::Text(_) => "span",
        NodeKind::Image(_) => "img",
        NodeKind::Container { .. } => "div",
      });

    let escape_attr = |s: &str| s.replace('&', "&amp;").replace('"', "&quot;");

    let mut attrs = Vec::new();
    if let Some(id) = &self.metadata.id {
      attrs.push(format!("id=\"{}\"", escape_attr(id)));
    }
    if let Some(class_name) = &self.metadata.class_name {
      attrs.push(format!("class=\"{}\"", escape_attr(class_name)));
    }
    if let Some(dir) = &self.metadata.dir {
      let dir_str = match dir {
        Direction::Ltr => "ltr",
        Direction::Rtl => "rtl",
      };
      attrs.push(format!("dir=\"{}\"", dir_str));
    }
    if let Some(attributes) = &self.metadata.attributes {
      for (k, v) in attributes {
        attrs.push(format!("{}=\"{}\"", k, escape_attr(v)));
      }
    }

    if let NodeKind::Image(image) = &self.kind
      && let ImageSourceInput::Url(url) = &image.src
    {
      attrs.push(format!("src=\"{}\"", escape_attr(url)));
    }

    // Each entry is (prefix_len, css_string) where css_string[..prefix_len] is the property name.
    // Inline overwrites preset for the same property — no extra key allocation needed.
    let mut inline_styles: Vec<(usize, String)> = Vec::new();
    let mut push_decl = |decl: &StyleDeclaration| {
      let mut buf = String::new();
      if decl.to_css(&mut buf).is_ok() && !buf.is_empty() {
        let prop_len = buf.find(':').unwrap_or(buf.len());
        if let Some(pos) = inline_styles
          .iter()
          .position(|(len, s)| s.get(..*len) == buf.get(..prop_len))
        {
          inline_styles[pos].1 = buf;
        } else {
          inline_styles.push((prop_len, buf));
        }
      }
    };
    if let Some(preset) = &self.metadata.preset {
      for decl in preset.declarations.iter() {
        push_decl(decl);
      }
    }
    if let Some(style) = &self.metadata.style {
      for decl in style.declarations.iter() {
        push_decl(decl);
      }
    }

    if !inline_styles.is_empty() {
      let joined: String =
        inline_styles
          .iter()
          .enumerate()
          .fold(String::new(), |mut acc, (i, (_, s))| {
            if i > 0 {
              acc.push(' ');
            }
            acc.push_str(s);
            acc
          });
      attrs.push(format!("style=\"{}\"", escape_attr(&joined)));
    }

    let attrs_str = if attrs.is_empty() {
      "".to_string()
    } else {
      format!(" {}", attrs.join(" "))
    };

    match &self.kind {
      NodeKind::Text(text) => {
        let escaped = text
          .text
          .replace('&', "&amp;")
          .replace('<', "&lt;")
          .replace('>', "&gt;")
          .replace('"', "&quot;")
          .replace('\'', "&#x27;");
        format!("<{}{}>{}</{}>", tag, attrs_str, escaped, tag)
      }
      NodeKind::Image(_) => {
        format!("<{}{} />", tag, attrs_str)
      }
      NodeKind::Container { children } => {
        let mut children_html = String::new();
        for child in children {
          children_html.push_str(&child.to_html());
        }
        format!("<{}{}>{}</{}>", tag, attrs_str, children_html, tag)
      }
    }
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

  /// `id` and `class` resolve to the structured metadata fields rather than
  /// the `attributes` map.
  pub(crate) fn attribute(&self, name: &str) -> Option<&str> {
    if name.eq_ignore_ascii_case("id") {
      return self.metadata.id.as_deref();
    }
    if name.eq_ignore_ascii_case("class") {
      return self.metadata.class_name.as_deref();
    }
    self
      .metadata
      .attributes
      .as_ref()?
      .iter()
      .find(|(attr_name, _)| attr_name.eq_ignore_ascii_case(name))
      .map(|(_, value)| value.as_ref())
  }
}

/// Style layers contributed by a node before cascade/inheritance assembly.
#[derive(Debug, Default, Clone)]
pub(crate) struct NodeStyleLayers {
  /// UA/default style preset for the element.
  pub(crate) preset: Option<Style>,
  /// Tailwind-derived author style for the element.
  pub(crate) author_tw: Option<TailwindValues>,
  /// Inline style attached directly to the element.
  pub(crate) inline: Option<Style>,
  pub(crate) dir: Option<Direction>,
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

  #[test]
  fn collect_style_fetch_tasks_collects_tailwind_mask_image_url() {
    let mask_url = "/logo.svg";
    let Ok(tw) = TailwindValues::from_str("mask-[url(/logo.svg)]") else {
      return;
    };
    let node = Node::container([]).with_tw(tw);

    let mut urls = Xxh3HashSet::default();
    node.style_resource_urls(&mut urls);

    assert_eq!(urls.into_iter().collect::<Vec<_>>(), vec![mask_url]);
  }
}
