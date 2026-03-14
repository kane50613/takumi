use std::{collections::BTreeMap, iter::once};

use serde::Deserialize;
use taffy::{AvailableSpace, Layout, Size};

use crate::{
  Result,
  layout::{
    inline::{
      InlineContentKind, InlineItem, InlineLayoutStage, create_inline_constraint,
      create_inline_layout, measure_inline_layout,
    },
    node::{Node, NodeMetadata, NodeStyleLayers},
    style::{Style, tw::TailwindValues},
  },
  rendering::{Canvas, MaxHeight, RenderContext, inline_drawing::draw_inline_layout},
};

/// A node that renders text content.
///
/// Text nodes display text with configurable font properties,
/// alignment, and styling options.
#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct TextNode {
  /// Shared node metadata.
  #[serde(flatten)]
  pub(crate) metadata: NodeMetadata,
  /// The text content to be rendered
  pub(crate) text: String,
}

impl TextNode {
  /// Set the tag name and return the updated text node.
  pub fn with_tag_name(mut self, tag_name: impl Into<Box<str>>) -> Self {
    self.metadata.tag_name = Some(tag_name.into());
    self
  }

  /// Set the class name and return the updated text node.
  pub fn with_class_name(mut self, class_name: impl Into<Box<str>>) -> Self {
    self.metadata.class_name = Some(class_name.into());
    self
  }

  /// Set the id and return the updated text node.
  pub fn with_id(mut self, id: impl Into<Box<str>>) -> Self {
    self.metadata.id = Some(id.into());
    self
  }

  /// Set the attributes and return the updated text node.
  pub fn with_attributes(mut self, attributes: BTreeMap<Box<str>, Box<str>>) -> Self {
    self.metadata.attributes = Some(attributes);
    self
  }

  /// Set the preset style and return the updated text node.
  pub fn with_preset(mut self, preset: Style) -> Self {
    self.metadata.preset = Some(preset);
    self
  }

  /// Set the inline style and return the updated text node.
  pub fn with_style(mut self, style: Style) -> Self {
    self.metadata.style = Some(style);
    self
  }

  /// Set the Tailwind values and return the updated text node.
  pub fn with_tw(mut self, tw: TailwindValues) -> Self {
    self.metadata.tw = Some(tw);
    self
  }

  /// Set the text content of the node.
  pub fn with_text(mut self, text: impl Into<String>) -> Self {
    self.text = text.into();
    self
  }
}

impl<Nodes: Node<Nodes>> Node<Nodes> for TextNode {
  fn metadata(&self) -> &NodeMetadata {
    &self.metadata
  }

  fn metadata_mut(&mut self) -> &mut NodeMetadata {
    &mut self.metadata
  }

  fn take_style_layers(&mut self) -> NodeStyleLayers {
    NodeStyleLayers {
      preset: self.metadata.preset.take(),
      author_tw: self.metadata.tw.take(),
      inline: self.metadata.style.take(),
    }
  }

  fn inline_content(&self) -> Option<InlineContentKind<'_>> {
    Some(InlineContentKind::Text(self.text.as_str().into()))
  }

  fn draw_content(
    &self,
    context: &RenderContext,
    canvas: &mut Canvas,
    layout: Layout,
  ) -> Result<()> {
    let font_style = context.style.to_sized_font_style(context);
    let size = layout.content_box_size();

    if font_style.sizing.font_size == 0.0 {
      return Ok(());
    }

    let max_height = match font_style.parent.line_clamp.as_ref() {
      Some(clamp) => Some(MaxHeight::HeightAndLines(size.height, clamp.count)),
      None => Some(MaxHeight::Absolute(size.height)),
    };

    let inline_text: InlineItem<'_, '_, Nodes> = InlineItem::Text {
      text: self.text.as_str().into(),
      context,
    };

    let (inline_layout, _, spans) = create_inline_layout(
      once(inline_text),
      Size {
        width: AvailableSpace::Definite(size.width),
        height: AvailableSpace::Definite(size.height),
      },
      size.width,
      max_height,
      &font_style,
      context.global,
      InlineLayoutStage::Draw,
    );

    draw_inline_layout(context, canvas, layout, inline_layout, &font_style, &spans)?;

    Ok(())
  }

  fn measure(
    &self,
    context: &RenderContext,
    available_space: Size<AvailableSpace>,
    known_dimensions: Size<Option<f32>>,
    _style: &taffy::Style,
  ) -> Size<f32> {
    let inline_content: InlineItem<'_, '_, Nodes> = InlineItem::Text {
      text: self.text.as_str().into(),
      context,
    };

    let (max_width, max_height) =
      create_inline_constraint(context, available_space, known_dimensions);

    let font_style = context.style.to_sized_font_style(context);

    let (mut layout, _, _) = create_inline_layout(
      once(inline_content),
      available_space,
      max_width,
      max_height,
      &font_style,
      context.global,
      InlineLayoutStage::Measure,
    );

    measure_inline_layout(&mut layout, max_width)
  }

  fn get_style(&self) -> Option<&Style> {
    self.metadata.style.as_ref()
  }
}
