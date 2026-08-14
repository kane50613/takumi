mod container;
mod image;
mod text;

use std::{
  collections::BTreeMap,
  mem::take,
  sync::{Arc, Weak},
};

use serde::Deserialize;

pub use self::image::resolve_image;
use self::{
  container::{
    container_children_ref, deserialize_children, drop_container_children, take_container_children,
  },
  image::{measure_image_node, take_image_style_layers},
  text::measure_text_node,
};
use crate::{
  Xxh3HashSet,
  context::RenderContext,
  geometry::{AvailableSpace, Size},
  layout::{inline::InlineContentKind, node::image::image_url},
  resources::{
    image::{ImageError, ImageResult, ImageSource},
    image_buffer::ImageBuffer,
  },
  style::{Direction, Lang, Style, StyleDeclaration, TailwindValues, ToCss},
  viewport::Viewport,
};

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
  /// The BCP-47 language tag for this node, equivalent to the `lang` attribute.
  pub lang: Option<Lang>,
}

#[derive(Debug, Default, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
/// Variant-specific text node data.
pub struct TextData {
  /// The text content.
  pub text: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
/// An image source as received from input: URL, raw bytes, raw pixels, or
/// already loaded.
#[non_exhaustive]
pub enum ImageSourceInput {
  /// Source URL or path.
  Url(Arc<str>),
  // `serde_bytes` so an FFI `Uint8Array`/`ArrayBuffer`, surfaced as a bytes value
  // rather than a number array, deserializes here.
  /// Raw image bytes.
  Buffer(#[serde(with = "serde_bytes")] Vec<u8>),
  /// Raw RGBA pixels with explicit dimensions, used without decoding.
  Rgba(RgbaImage),
  /// Pre-resolved image source.
  #[serde(skip_deserializing)]
  Loaded(ImageSource),
}

/// Raw row-major RGBA pixels, converted to a premultiplied bitmap once at
/// construction; resolving is a reference-count bump.
#[derive(Debug, Clone, Deserialize)]
#[serde(try_from = "RgbaImageData")]
pub struct RgbaImage {
  source: ImageSource,
}

impl RgbaImage {
  /// Wraps raw RGBA pixels, premultiplying in place unless `premultiplied`
  /// says the bytes already are. Errors if `data.len() != width * height * 4`.
  pub fn new(
    data: Vec<u8>,
    width: u32,
    height: u32,
    premultiplied: bool,
  ) -> Result<Self, ImageError> {
    let buffer = if premultiplied {
      ImageBuffer::from_premultiplied_rgba(data, width, height)
    } else {
      ImageBuffer::from_rgba_bytes(data, width, height)
    };

    buffer
      .map(|buffer| Self {
        source: ImageSource::from(buffer),
      })
      .ok_or(ImageError::MismatchedBufferSize)
  }
}

/// The wire shape of [`RgbaImage`]: straight alpha unless `premultiplied`.
/// `Option` so an explicit JS `undefined`, surfaced as a unit value, still
/// deserializes.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RgbaImageData {
  width: u32,
  height: u32,
  #[serde(with = "serde_bytes")]
  data: Vec<u8>,
  #[serde(default)]
  premultiplied: Option<bool>,
}

impl TryFrom<RgbaImageData> for RgbaImage {
  type Error = ImageError;

  fn try_from(raw: RgbaImageData) -> Result<Self, Self::Error> {
    Self::new(
      raw.data,
      raw.width,
      raw.height,
      raw.premultiplied.unwrap_or_default(),
    )
  }
}

impl ImageSourceInput {
  /// Resolve this input to image bytes using the render context.
  pub fn resolve(&self, context: &RenderContext) -> ImageResult {
    match self {
      Self::Url(src) => resolve_image(src, context),
      Self::Buffer(data) => ImageSource::from_bytes_lazy(data, 0, Weak::new()),
      Self::Rgba(raw) => Ok(raw.source.clone()),
      Self::Loaded(source) => Ok(source.clone()),
    }
  }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
/// Variant-specific image node data.
pub struct ImageData {
  /// The image source.
  pub src: ImageSourceInput,
  /// Override width in pixels.
  pub width: Option<f32>,
  /// Override height in pixels.
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

impl From<ImageBuffer> for ImageData {
  fn from(buffer: ImageBuffer) -> Self {
    Self::from(ImageSource::from(buffer))
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
  /// The variant-specific content of this node.
  #[serde(flatten)]
  pub kind: NodeKind,
}

/// Represents the nodes enum.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
#[non_exhaustive]
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

  /// Takes the node's own text, leaving an empty container behind.
  pub(crate) fn take_text(&mut self) -> Option<String> {
    let NodeKind::Text(data) = &mut self.kind else {
      return None;
    };

    if data.text.is_empty() {
      return None;
    }

    let text = take(&mut data.text);
    self.kind = NodeKind::Container {
      children: Vec::new(),
    };

    Some(text)
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

  /// Sets the BCP-47 language tag and returns the updated node.
  pub fn with_lang(mut self, lang: Lang) -> Self {
    self.metadata.lang = Some(lang);
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
        Direction::Rtl => "rtl",
        Direction::Ltr => "ltr",
      };
      attrs.push(format!("dir=\"{}\"", dir_str));
    }
    if let Some(lang) = &self.metadata.lang {
      attrs.push(format!("lang=\"{}\"", escape_attr(lang.as_str())));
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
      lang: self.metadata.lang.take(),
    }
  }

  pub(crate) fn inline_content(&self) -> Option<InlineContentKind<'_>> {
    match &self.kind {
      NodeKind::Container { .. } => None,
      NodeKind::Image(_) => Some(InlineContentKind::Box),
      NodeKind::Text(text) => Some(InlineContentKind::Text(text.text.as_str().into())),
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
  pub(crate) fn metadata_image_urls<'a>(&'a self, urls: &mut Xxh3HashSet<&'a str>) {
    match &self.kind {
      NodeKind::Container { .. } => {
        let Some(children) = self.children_ref() else {
          return;
        };

        for child in children {
          child.metadata_image_urls(urls);
        }
      }
      NodeKind::Image(image) => {
        if let Some(url) = image_url(image) {
          urls.insert(url);
        }
      }
      NodeKind::Text(_) => {}
    }
  }

  /// Collects resource URLs referenced by this node tree's styles.
  pub(crate) fn style_image_urls<'a>(&'a self, urls: &mut Xxh3HashSet<&'a str>) {
    if let Some(preset) = self.metadata.preset.as_ref() {
      urls.extend(preset.image_urls());
    }

    if let Some(author_tw) = self.metadata.tw.as_ref() {
      urls.extend(author_tw.image_urls(Viewport::default()));
    }

    if let Some(inline) = self.metadata.style.as_ref() {
      urls.extend(inline.image_urls());
    }

    let Some(children) = self.children_ref() else {
      return;
    };

    for child in children {
      child.style_image_urls(urls);
    }
  }

  /// Collects unique resource URLs referenced by this node tree and styles.
  pub fn image_urls(&self) -> impl Iterator<Item = &str> {
    let mut urls = Xxh3HashSet::default();
    self.metadata_image_urls(&mut urls);
    self.style_image_urls(&mut urls);

    urls.into_iter()
  }

  pub(crate) fn is_replaced_element(&self) -> bool {
    matches!(self.kind, NodeKind::Image(_))
  }

  /// The element's `href` attribute, when present and non-empty.
  pub fn href(&self) -> Option<&str> {
    self.attribute("href").filter(|href| !href.is_empty())
  }

  /// The element's `alt` attribute, when present. An explicitly empty value
  /// marks a decorative image, distinct from a missing attribute.
  pub fn alt(&self) -> Option<&str> {
    self.attribute("alt")
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
    if name.eq_ignore_ascii_case("lang") {
      return self.metadata.lang.as_ref().map(Lang::as_str);
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
  pub(crate) lang: Option<Lang>,
}

/// Selector-matching queries read by [`crate::matching`].
impl Node {
  /// The element's tag name.
  pub fn tag_name(&self) -> Option<&str> {
    self.metadata.tag_name.as_deref()
  }

  /// The element's `id` attribute, when present.
  pub fn id(&self) -> Option<&str> {
    self.metadata.id.as_deref()
  }

  /// The element's `class` attribute, space-separated.
  pub fn class_name(&self) -> Option<&str> {
    self.metadata.class_name.as_deref()
  }

  pub(crate) fn attr(&self, name: &str) -> Option<&str> {
    self.attribute(name)
  }

  pub(crate) fn is_replaced(&self) -> bool {
    self.is_replaced_element()
  }

  pub(crate) fn children(&self) -> Option<&[Self]> {
    self.children_ref()
  }
}

#[cfg(test)]
mod tests {
  use std::str::FromStr;

  use super::*;
  use crate::style::{BackgroundImage, Style, StyleDeclaration, TailwindValues};

  #[test]
  fn alt_distinguishes_missing_from_empty() {
    let node = Node::container(Vec::new());

    assert_eq!(node.alt(), None);

    let node = node.with_attributes(BTreeMap::from([("alt".into(), "".into())]));

    assert_eq!(node.alt(), Some(""));
  }

  #[test]
  fn image_source_input_deserializes_raw_rgba() {
    let input: ImageSourceInput = serde_json::from_value(serde_json::json!({
      "width": 2,
      "height": 1,
      "data": [255, 0, 0, 128, 0, 255, 0, 255],
    }))
    .unwrap();

    let ImageSourceInput::Rgba(raw) = input else {
      panic!("expected Rgba, got {input:?}");
    };
    let ImageSource::Bitmap(buffer) = &raw.source else {
      panic!("expected bitmap source");
    };

    assert_eq!((buffer.width(), buffer.height()), (2, 1));
    assert_eq!(buffer.data(), [128, 0, 0, 128, 0, 255, 0, 255]);
  }

  #[test]
  fn image_source_input_raw_rgba_premultiplied_skips_premultiply() {
    let input: ImageSourceInput = serde_json::from_value(serde_json::json!({
      "width": 1,
      "height": 1,
      "data": [100, 50, 0, 128],
      "premultiplied": true,
    }))
    .unwrap();

    let ImageSourceInput::Rgba(raw) = input else {
      panic!("expected Rgba, got {input:?}");
    };
    let ImageSource::Bitmap(buffer) = &raw.source else {
      panic!("expected bitmap source");
    };

    assert_eq!(buffer.data(), [100, 50, 0, 128]);
  }

  #[test]
  fn image_source_input_rejects_mismatched_raw_rgba() {
    let result: Result<ImageSourceInput, _> = serde_json::from_value(serde_json::json!({
      "width": 4,
      "height": 4,
      "data": [0, 0, 0, 0],
    }));

    assert!(result.is_err());
  }

  #[test]
  fn collect_style_fetch_tasks_collects_nested_background_image_urls() {
    let background_url = "https://placehold.co/80x80/22c55e/white";
    let node = Node::container([Node::container([]).with_style(Style::default().with(
      StyleDeclaration::background_image(Some(
        [BackgroundImage::Url(background_url.into())].into(),
      )),
    ))]);

    let mut urls = Xxh3HashSet::default();
    node.style_image_urls(&mut urls);

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
    node.style_image_urls(&mut urls);

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
    node.style_image_urls(&mut urls);

    assert_eq!(urls.into_iter().collect::<Vec<_>>(), vec![mask_url]);
  }
}

#[cfg(test)]
mod matching_tests {
  use std::collections::BTreeMap;

  use crate::{
    layout::node::Node,
    matching::{MatchedDeclarationsView, match_stylesheets_view},
    style::{ComputedStyle, Length, Style, StyleSheet},
    viewport::Viewport,
  };

  fn container_with_class(class_name: &str) -> Node {
    Node::container([]).with_class_name(class_name)
  }

  fn computed_width_from_matches(matches: &MatchedDeclarationsView<'_>) -> Length {
    let mut style = Style::default();
    for &declarations in matches.normal() {
      for declaration in declarations.iter() {
        declaration.merge_into_ref(&mut style);
      }
    }
    for &declarations in matches.important() {
      for declaration in declarations.iter() {
        declaration.merge_into_ref(&mut style);
      }
    }
    style.inherit(&ComputedStyle::default()).width
  }

  fn computed_height_from_matches(matches: &MatchedDeclarationsView<'_>) -> Length {
    let mut style = Style::default();
    for &declarations in matches.normal() {
      for declaration in declarations.iter() {
        declaration.merge_into_ref(&mut style);
      }
    }
    for &declarations in matches.important() {
      for declaration in declarations.iter() {
        declaration.merge_into_ref(&mut style);
      }
    }
    style.inherit(&ComputedStyle::default()).height
  }

  fn parse_stylesheet(css: &str) -> StyleSheet {
    let result = StyleSheet::parse(css);
    assert!(result.is_ok(), "expected stylesheet to parse: {result:?}");
    result.unwrap_or_default()
  }

  fn parse_stylesheet_list<I, S>(stylesheets: I) -> StyleSheet
  where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
  {
    let result = StyleSheet::parse_list(stylesheets);
    assert!(
      result.is_ok(),
      "expected stylesheet list to parse: {result:?}"
    );
    result.unwrap_or_default()
  }

  #[test]
  fn layered_rules_outrank_source_order() {
    let root = container_with_class("card");
    let stylesheet = parse_stylesheet(
      r#"
        @layer theme, base;
        @layer base {
          .card { width: 10px; }
        }
        @layer theme {
          .card { width: 20px; }
        }
      "#,
    );

    let matched = match_stylesheets_view(&root, &stylesheet, Viewport::default());
    assert_eq!(matched.len(), 1);
    assert_eq!(
      computed_width_from_matches(matched[0].element()),
      Length::Px(10.0)
    );
  }

  #[test]
  fn nested_selector_uses_parent_list_specificity() {
    let root = Node::container([container_with_class("title")]).with_class_name("card notice");

    let stylesheet = parse_stylesheet(
      r#"
        .card, #panel {
          .title { width: 10px; }
        }

        .notice .title { width: 20px; }
      "#,
    );

    let matched = match_stylesheets_view(&root, &stylesheet, Viewport::default());
    assert_eq!(matched.len(), 2);
    assert_eq!(
      computed_width_from_matches(matched[1].element()),
      Length::Px(10.0)
    );
  }

  #[test]
  fn important_layered_rules_outrank_unlayered_important() {
    let root = container_with_class("card");
    let stylesheet = parse_stylesheet(
      r#"
        @layer theme, base;
        .card { width: 5px !important; }
        @layer base {
          .card { width: 10px !important; }
        }
        @layer theme {
          .card { width: 20px !important; }
        }
      "#,
    );

    let matched = match_stylesheets_view(&root, &stylesheet, Viewport::default());
    assert_eq!(matched.len(), 1);
    assert_eq!(
      computed_width_from_matches(matched[0].element()),
      Length::Px(20.0)
    );
  }

  #[test]
  fn later_stylesheet_rules_outrank_earlier_stylesheets_on_ties() {
    let root = container_with_class("card");
    let stylesheet = parse_stylesheet(".card { width: 10px; } .card { width: 20px; }");

    let matched = match_stylesheets_view(&root, &stylesheet, Viewport::default());
    assert_eq!(matched.len(), 1);
    assert_eq!(
      computed_width_from_matches(matched[0].element()),
      Length::Px(20.0)
    );
  }

  #[test]
  fn parse_list_preserves_cross_stylesheet_layer_order() {
    let root = container_with_class("card");
    let stylesheet = parse_stylesheet_list([
      r#"
        @layer theme, base;
        @layer base {
          .card { width: 10px; }
        }
      "#,
      r#"
        @layer theme {
          .card { width: 20px; }
        }
      "#,
    ]);

    let matched = match_stylesheets_view(&root, &stylesheet, Viewport::default());
    assert_eq!(matched.len(), 1);
    assert_eq!(
      computed_width_from_matches(matched[0].element()),
      Length::Px(10.0)
    );
  }

  #[test]
  fn root_selector_list_with_host_keeps_matching_root() {
    let root = Node::default();
    let stylesheet = parse_stylesheet(
      r#"
        :root, :host {
          width: 10px;
        }
      "#,
    );

    let matched = match_stylesheets_view(&root, &stylesheet, Viewport::default());
    assert_eq!(matched.len(), 1);
    assert_eq!(
      computed_width_from_matches(matched[0].element()),
      Length::Px(10.0)
    );
  }

  #[test]
  fn sibling_combinators_only_match_the_correct_siblings() {
    let root = Node::container([
      container_with_class("lead"),
      container_with_class("title"),
      container_with_class("spacer"),
      container_with_class("title"),
    ])
    .with_class_name("container");
    let stylesheet = parse_stylesheet(
      r#"
        .container .title { width: 20px; }
        .lead + .title { width: 10px; }
        .lead ~ .title { height: 30px; }
      "#,
    );

    let matched = match_stylesheets_view(&root, &stylesheet, Viewport::default());
    assert_eq!(matched.len(), 5);
    assert_eq!(
      computed_width_from_matches(matched[2].element()),
      Length::Px(10.0)
    );
    assert_eq!(
      computed_height_from_matches(matched[2].element()),
      Length::Px(30.0)
    );
    assert_eq!(
      computed_width_from_matches(matched[4].element()),
      Length::Px(20.0)
    );
    assert_eq!(
      computed_height_from_matches(matched[4].element()),
      Length::Px(30.0)
    );
  }

  #[test]
  fn attribute_selectors_match_node_metadata_and_attributes() {
    let root = Node::container([Node::container([])
      .with_id("hero")
      .with_class_name("card featured")
      .with_attributes(BTreeMap::from([
        (Box::<str>::from("data-kind"), Box::<str>::from("promo")),
        (
          Box::<str>::from("data-state"),
          Box::<str>::from("ready now"),
        ),
      ]))]);
    let stylesheet = parse_stylesheet(
      r#"
        [id="hero"] { width: 10px; }
        [class~="featured"] { height: 20px; }
        [data-kind="promo"] { width: 30px; }
        [data-state~="ready"] { height: 40px; }
      "#,
    );

    let matched = match_stylesheets_view(&root, &stylesheet, Viewport::default());
    assert_eq!(matched.len(), 2);
    assert_eq!(
      computed_width_from_matches(matched[1].element()),
      Length::Px(30.0)
    );
    assert_eq!(
      computed_height_from_matches(matched[1].element()),
      Length::Px(40.0)
    );
  }

  #[test]
  fn test_repro_mixed_importance_bug() {
    let stylesheet = parse_stylesheet(".test { width: 10px; height: 20px !important; }");
    let root = Node::container([]).with_class_name("test");
    let matched = match_stylesheets_view(&root, &stylesheet, Viewport::default());

    // Matched normal: should have width: 10px.
    // Matched important: should have height: 20px.

    assert_eq!(matched[0].element().normal()[0].len(), 1);
    assert!(matched[0].element().normal()[0].importance.is_empty());

    assert_eq!(matched[0].element().important()[0].len(), 1);
    assert!(!matched[0].element().important()[0].importance.is_empty());
  }

  #[test]
  fn pseudo_element_rules_land_in_before_after_buckets() {
    let root = Node::container([]).with_class_name("card");
    let stylesheet = parse_stylesheet(
      r#"
        .card::before { content: "x"; }
        .card::after  { content: "y"; }
        .card         { width: 10px; }
      "#,
    );

    let matched = match_stylesheets_view(&root, &stylesheet, Viewport::default());
    assert_eq!(matched.len(), 1);

    assert!(!matched[0].element().normal().is_empty());
    assert!(matched[0].before().is_some());
    assert!(matched[0].after().is_some());
  }

  #[test]
  fn pseudo_element_rules_are_skipped_on_replaced_elements() {
    let root = Node::image("data:image/png;base64,iVBOR").with_class_name("logo");
    let stylesheet = parse_stylesheet(r#".logo::before { content: "x"; }"#);

    let matched = match_stylesheets_view(&root, &stylesheet, Viewport::default());
    assert_eq!(matched.len(), 1);
    assert!(matched[0].before().is_none());
    assert!(matched[0].after().is_none());
  }

  #[test]
  fn unsupported_pseudo_element_does_not_create_bucket() {
    let root = Node::container([]).with_class_name("card");
    let stylesheet = parse_stylesheet(r#".card::placeholder { color: red; }"#);

    let matched = match_stylesheets_view(&root, &stylesheet, Viewport::default());
    assert_eq!(matched.len(), 1);
    assert!(matched[0].before().is_none());
    assert!(matched[0].after().is_none());
  }
}
