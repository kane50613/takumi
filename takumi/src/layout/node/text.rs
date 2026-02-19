use std::iter::once;

use serde::Deserialize;
use taffy::{AvailableSpace, Layout, Size};

use crate::{
  Result,
  layout::{
    Viewport,
    inline::{
      InlineContentKind, InlineItem, InlineLayoutStage, create_inline_constraint,
      create_inline_layout, measure_inline_layout,
    },
    node::Node,
    style::{InheritedStyle, Style, tw::TailwindValues},
  },
  rendering::{Canvas, MaxHeight, RenderContext, inline_drawing::draw_inline_layout},
};

/// A node that renders text content.
///
/// Text nodes display text with configurable font properties,
/// alignment, and styling options.
#[derive(Debug, Clone, Deserialize)]
pub struct TextNode {
  /// Default style presets from HTML element type (lowest priority)
  pub preset: Option<Style>,
  /// The styling properties for this text node
  pub style: Option<Style>,
  /// The text content to be rendered
  pub text: String,
  /// The tailwind properties for this text node
  pub tw: Option<TailwindValues>,
}

impl<Nodes: Node<Nodes>> Node<Nodes> for TextNode {
  fn create_inherited_style(
    &mut self,
    parent_style: &InheritedStyle,
    viewport: Viewport,
  ) -> InheritedStyle {
    // Start with empty style
    let mut style = Style::default();

    // 1. Apply preset first (lowest priority)
    if let Some(preset) = self.preset.take() {
      style.merge_from(preset);
    }

    // 2. Apply Tailwind (medium priority)
    if let Some(tw) = self.tw.as_ref() {
      tw.apply(&mut style, viewport);
    }

    // 3. Merge inline style last (highest priority)
    if let Some(inline_style) = self.style.take() {
      style.merge_from(inline_style);
    }

    style.inherit(parent_style)
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
    self.style.as_ref()
  }
}
