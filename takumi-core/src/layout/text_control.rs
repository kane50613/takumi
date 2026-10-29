//! Blink's text controls: an `<input>` text field lays out around an inner editor, which holds
//! its value, as Blink's `TextControlInnerEditorElement` does in the element's shadow tree.

use crate::{
  context::RenderContext,
  font_style::BoxLineHeight,
  form_control::InputType,
  layout::{
    node::Node,
    tree::{NodeOrigin, RenderNode},
  },
  layout_unit::LayoutUnit,
  matching::MatchedDeclarationsView,
  resources::font::PrimaryFontMetrics,
  style::{
    Display, FontFamilyToken, Length, LineHeight, Overflow, OverflowWrap, Size, TextWrapMode,
    WhiteSpaceCollapse,
  },
};

/// The number of characters a text field without a valid `size` attribute is wide.
const DEFAULT_SIZE: u32 = 20;

/// Blink's `kFontFamiliesWithInvalidCharWidth`: families whose average character width does not
/// size a text field.
const FONT_FAMILIES_WITH_INVALID_CHAR_WIDTH: [&str; 34] = [
  "American Typewriter",
  "Arial Hebrew",
  "Chalkboard",
  "Cochin",
  "Corsiva Hebrew",
  "Courier",
  "Euphemia UCAS",
  "Geneva",
  "Gill Sans",
  "Hei",
  "Helvetica",
  "Hoefler Text",
  "InaiMathi",
  "Kai",
  "Lucida Grande",
  "Marker Felt",
  "Monaco",
  "Mshtakan",
  "New Peninim MT",
  "Osaka",
  "Raanana",
  "STHeiti",
  "Symbol",
  "Times",
  "Apple Braille",
  "Apple LiGothic",
  "Apple LiSung",
  "Apple Symbols",
  "AppleGothic",
  "AppleMyungjo",
  "#GungSeo",
  "#HeadLineA",
  "#PCMyungjo",
  "#PilGi",
];

/// The part a box plays in a text control.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TextControlBox {
  /// The control element itself.
  Host(TextControl),
  /// The box holding its value.
  InnerEditor(TextControl),
}

/// A text control and the attributes that size it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TextControl {
  /// Blink's `LayoutTextControlSingleLine`: an `<input>` text field `size` characters wide.
  SingleLine { size: u32 },
}

impl TextControl {
  /// The text control `node` is, if any.
  pub(crate) fn of(node: &Node) -> Option<Self> {
    let input_type = InputType::of(node)?;

    if !input_type.is_text_field() {
      return None;
    }

    // Blink's `NumberInputType::GetSizeWithDecoration` sizes a number field without `size`.
    let size = if input_type == InputType::Number {
      DEFAULT_SIZE
    } else {
      node
        .attribute("size")
        .and_then(parse_non_negative_integer)
        .filter(|&size| size > 0 && size <= 0x7fff_ffff)
        .unwrap_or(DEFAULT_SIZE)
    };

    Some(Self::SingleLine { size })
  }

  /// The box `host` lays out its value in, after Blink's
  /// `TextControlInnerEditorElement::CustomStyleForLayoutObject`. Approximate: the editor clips
  /// where Blink's scrolls with its scrollbars hidden, which paints the same.
  pub(crate) fn inner_editor(self, host: &RenderContext, node: &Node) -> RenderNode {
    let (mut style, sizing, current_color) =
      host.resolve_pseudo_style(&MatchedDeclarationsView::default());
    let host_line_height = computed_line_height(host).to_float().trunc();
    let removes_line_height = sizing.font_size >= host_line_height
      || match host.style.box_data.height {
        Size::Length(length) if length.has_percentage() => true,
        Size::Length(Length::Auto | Length::Calc(_)) => false,
        Size::Length(length) => length.to_px(&host.sizing, 0.0) > host_line_height,
        _ => false,
      };

    style.base_data_mut().display = Display::FlowRoot;
    style.base_data_mut().overflow_x = Overflow::Hidden;
    style.base_data_mut().overflow_y = Overflow::Hidden;
    style.misc4_data_mut().text_overflow = host.style.misc4_data.text_overflow.clone();

    let inherited = style.inherited_data_mut();

    inherited.white_space_collapse = WhiteSpaceCollapse::Preserve;
    inherited.text_wrap_mode = TextWrapMode::NoWrap;
    if removes_line_height {
      inherited.line_height = LineHeight::Normal;
    }
    style.misc_inherited_data_mut().overflow_wrap = OverflowWrap::Normal;

    let context = RenderContext::from_parent(host, style, sizing, current_color);
    let value = self.value(node);
    // Blink's inner editor is a `<div>` holding the value as its text.
    let editor_node = if value.is_empty() {
      Node::container([])
    } else {
      Node::text(value)
    };
    let mut editor = RenderNode::new(
      context,
      NodeOrigin::Pseudo,
      Some(editor_node.with_tag_name("div")),
      None,
    );

    editor.text_control = Some(TextControlBox::InnerEditor(self));
    editor
  }

  /// The text the editor shows: the sanitized value, masked in a password field.
  fn value(self, node: &Node) -> String {
    let Some(input_type) = InputType::of(node) else {
      return String::new();
    };
    let value = input_type.sanitize_value(node.attribute("value").unwrap_or_default());

    if input_type == InputType::Password {
      // Blink's `LayoutText::SecureText` with `-webkit-text-security: disc`. Approximate: it
      // masks each character, where Blink masks each grapheme cluster.
      value.chars().map(|_| '\u{2022}').collect()
    } else {
      value.into_owned()
    }
  }

  /// Blink's `TextFieldIntrinsicInlineSize`: the content inline size `host` takes.
  pub(crate) fn default_inline_size(self, host: &RenderContext) -> LayoutUnit {
    let Self::SingleLine { size } = self;
    let metrics = primary_font_metrics(host);
    let char_width = avg_char_width(host, metrics);
    let max_char_width = metrics
      .filter(|&metrics| has_valid_avg_char_width(host, metrics))
      .map_or(0.0, |metrics| metrics.max_char_width);
    let mut width = char_width * size as f32;

    // For text inputs, IE adds some extra width.
    if max_char_width > char_width {
      width += max_char_width - char_width;
    }

    LayoutUnit::from_float(width.ceil())
  }

  /// Blink's `TextFieldIntrinsicBlockSize`: the content block size of a host whose inner editor
  /// is `editor`, one of the editor's lines.
  pub(crate) fn default_block_size(editor: &RenderContext) -> LayoutUnit {
    computed_line_height(editor)
  }
}

/// Blink's `BlockLayoutAlgorithm::BaselineForEmptyLine`: where an empty editor with `border_padding`
/// above its content puts its baseline.
pub(crate) fn baseline_for_empty_line(
  editor: &RenderContext,
  border_padding_block_start: LayoutUnit,
) -> Option<LayoutUnit> {
  let metrics = primary_font_metrics(editor)?;
  let font_height = metrics.font_height();
  let baseline = border_padding_block_start
    + LayoutUnit::from_float(metrics.ascent)
    + (computed_line_height(editor) - (font_height.ascent + font_height.descent)) / 2;

  Some(LayoutUnit::from_float(baseline.to_int() as f32))
}

fn primary_font_metrics(context: &RenderContext) -> Option<PrimaryFontMetrics> {
  context.primary_font_metrics(&context.style, context.sizing.font_size)
}

/// Blink's `ComputedLineHeightAsFixed` of `context`.
fn computed_line_height(context: &RenderContext) -> LayoutUnit {
  BoxLineHeight::of(&context.style, context)
    .computed_with_metrics(primary_font_metrics(context), context.sizing.font_size)
}

/// Blink's `layout_text_control::HasValidAvgCharWidth`.
fn has_valid_avg_char_width(context: &RenderContext, metrics: PrimaryFontMetrics) -> bool {
  // Some fonts match avgCharWidth to CJK full-width characters.
  if metrics.has_zero_width && metrics.avg_char_width > metrics.zero_width * 1.7 {
    return false;
  }

  match context.style.inherited_data.font_family.tokens().first() {
    Some(FontFamilyToken::Owned(family)) => {
      !family.is_empty() && !FONT_FAMILIES_WITH_INVALID_CHAR_WIDTH.contains(&family.as_str())
    }
    Some(FontFamilyToken::Generic(_)) => true,
    None => false,
  }
}

/// Blink's `layout_text_control::GetAvgCharWidth`. Approximate: without a valid average width it
/// takes the primary font's `0` advance, where Blink shapes `0` through the whole fallback list.
fn avg_char_width(context: &RenderContext, metrics: Option<PrimaryFontMetrics>) -> f32 {
  match metrics {
    Some(metrics) if has_valid_avg_char_width(context, metrics) => {
      // Rounded only from a fraction of one half up, which keeps the width from shrinking.
      metrics.avg_char_width.max(metrics.avg_char_width.round())
    }
    Some(metrics) => metrics.zero_width,
    None => context.sizing.font_size / 2.0,
  }
}

/// HTML's rules for parsing non-negative integers.
/// <https://html.spec.whatwg.org/multipage/common-microsyntaxes.html#rules-for-parsing-non-negative-integers>
fn parse_non_negative_integer(value: &str) -> Option<u32> {
  let value = value.trim_start_matches(['\t', '\n', '\u{c}', '\r', ' ']);
  let value = value.strip_prefix('+').unwrap_or(value);
  let digits = value
    .find(|character: char| !character.is_ascii_digit())
    .map_or(value, |end| &value[..end]);

  digits.parse().ok()
}

#[cfg(test)]
mod tests {
  use std::{fs::read, path::Path};

  use skrifa::{
    FontRef, MetadataProvider,
    instance::{LocationRef, Size as FontSize},
    raw::TableProvider,
  };

  use super::*;
  use crate::layout::inline::testing::render_root;

  const ARCHIVO: &str = "../assets/fonts/archivo/Archivo-VariableFont_wdth,wght.ttf";
  const FONT_SIZE: f32 = 13.0;

  /// The inline size of a `size="7"` text field in Archivo, registered as `family`.
  fn default_inline_size(family: &str) -> f32 {
    let root = render_root(
      ARCHIVO,
      family,
      Node::container([Node::container([])
        .with_tag_name("input")
        .with_attributes([("size".into(), "7".into())].into())]),
      &format!("input {{ font-family: '{family}'; font-size: {FONT_SIZE}px }}"),
    );
    let host = &root.children.as_deref().unwrap()[0];
    let Some(TextControlBox::Host(control)) = host.text_control else {
      panic!("not a text control host");
    };

    control.default_inline_size(&host.context).to_float()
  }

  #[test]
  fn width_comes_from_the_average_and_maximum_character_widths() {
    let bytes = read(Path::new(env!("CARGO_MANIFEST_DIR")).join(ARCHIVO)).unwrap();
    let font = FontRef::new(&bytes).unwrap();
    let scale = FONT_SIZE / f32::from(font.head().unwrap().units_per_em());
    let head = font.head().unwrap();
    let average = f32::from(font.os2().unwrap().x_avg_char_width()) * scale;
    let average = average.max(average.round());
    let maximum = (f32::from(head.x_max() - head.x_min()) * scale).round();

    assert_eq!(
      default_inline_size("Archivo"),
      (average * 7.0 + (maximum - average).max(0.0)).ceil()
    );
  }

  #[test]
  fn width_comes_from_the_zero_glyph_in_a_family_blink_distrusts() {
    let bytes = read(Path::new(env!("CARGO_MANIFEST_DIR")).join(ARCHIVO)).unwrap();
    let font = FontRef::new(&bytes).unwrap();
    let zero = font
      .glyph_metrics(FontSize::new(FONT_SIZE), LocationRef::default())
      .advance_width(font.charmap().map('0').unwrap())
      .unwrap();

    assert_eq!(default_inline_size("Helvetica"), (zero * 7.0).ceil());
  }

  #[test]
  fn size_parses_as_a_non_negative_integer() {
    assert_eq!(parse_non_negative_integer(" +12px"), Some(12));
    assert_eq!(parse_non_negative_integer("-1"), None);
    assert_eq!(parse_non_negative_integer(""), None);
    assert_eq!(parse_non_negative_integer("99999999999"), None);
  }
}
