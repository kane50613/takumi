pub(crate) mod map;

use std::str::FromStr;

use serde::Deserializer;
use smallvec::smallvec;

use crate::layout::style::{
  tw::map::{FIXED_PROPERTIES, PREFIX_PARSERS},
  *,
};

/// Tailwind `--spacing` variable value.
pub const VAR_SPACING: f32 = 0.25;

/// Represents a collection of tailwind properties.
#[derive(Debug, Clone)]
pub struct TailwindProperties {
  inner: Vec<TailwindProperty>,
}

impl FromStr for TailwindProperties {
  type Err = String;

  fn from_str(s: &str) -> Result<Self, Self::Err> {
    Ok(TailwindProperties {
      inner: TailwindProperty::parse_list(s).collect(),
    })
  }
}

impl TailwindProperties {
  /// Iterate over the tailwind properties.
  pub fn iter(&self) -> impl Iterator<Item = &TailwindProperty> {
    self.inner.iter()
  }

  pub(crate) fn apply(&self, style: &mut Style) {
    for property in self.iter() {
      property.apply(style);
    }
  }
}

impl<'de> Deserialize<'de> for TailwindProperties {
  fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
  where
    D: Deserializer<'de>,
  {
    let string = String::deserialize(deserializer)?;

    Ok(TailwindProperties {
      inner: TailwindProperty::parse_list(&string).collect(),
    })
  }
}

/// Represents a tailwind property.
#[derive(Debug, Clone, PartialEq)]
pub enum TailwindProperty {
  /// The box sizing of the element.
  BoxSizing(BoxSizing),
  /// The flex grow of the element.
  FlexGrow(FlexGrow),
  /// The flex shrink of the element.
  FlexShrink(FlexGrow),
  /// The aspect ratio of the element.
  Aspect(AspectRatio),
  /// The alignment of the items in the element.
  Items(AlignItems),
  /// The justification of the content in the element.
  Justify(JustifyContent),
  /// The alignment of the content in the element.
  Content(JustifyContent),
  /// The alignment of the self in the element.
  JustifySelf(AlignItems),
  /// The alignment of the items in the element.
  AlignSelf(AlignItems),
  /// The direction of the flex in the element.
  FlexDirection(FlexDirection),
  /// The wrapping of the flex in the element.
  FlexWrap(FlexWrap),
  /// The flex properties of the element.
  Flex(Flex),
  /// The basis of the flex in the element.
  FlexBasis(LengthUnit),
  /// The overflow of the element.
  Overflow(Overflow),
  /// The overflow of the element on the x-axis.
  OverflowX(Overflow),
  /// The overflow of the element on the y-axis.
  OverflowY(Overflow),
  /// The position of the element.
  Position(Position),
  /// The style of the font in the element.
  FontStyle(FontStyle),
  /// The weight of the font in the element.
  FontWeight(FontWeight),
  /// The family of the font in the element.
  FontFamily(FontFamily),
  /// The line clamp of the element.
  LineClamp(LineClamp),
  /// The overflow of the text in the element.
  TextOverflow(TextOverflow),
  /// The wrap mode of the text in the element.
  TextWrap(TextWrapMode),
  /// The collapse mode of the white space in the element.
  WhiteSpace(WhiteSpace),
  /// The word break of the text in the element.
  WordBreak(WordBreak),
  /// The overflow wrap of the text in the element.
  OverflowWrap(OverflowWrap),
  /// Set `text-overflow: ellipsis`, `white-space: nowrap` and `overflow: hidden`.
  Truncate,
  /// The alignment of the text in the element.
  TextAlign(TextAlign),
  /// The decoration of the text in the element.
  TextDecoration(TextDecoration),
  /// The transformation of the text in the element.
  TextTransform(TextTransform),
  /// The size of the element.
  Size(LengthUnit),
  /// The width of the element.
  Width(LengthUnit),
  /// The height of the element.
  Height(LengthUnit),
  /// The minimum width of the element.
  MinWidth(LengthUnit),
  /// The minimum height of the element.
  MinHeight(LengthUnit),
  /// The maximum width of the element.
  MaxWidth(LengthUnit),
  /// The maximum height of the element.
  MaxHeight(LengthUnit),
  /// The shadow of the element.
  Shadow(BoxShadow),
  /// The display of the element.
  Display(Display),
  /// The object position of the element.
  ObjectPosition(BackgroundPosition),
  /// The object fit of the element.
  ObjectFit(ObjectFit),
  /// The background position of the element.
  BackgroundPosition(BackgroundPosition),
  /// The background size of the element.
  BackgroundSize(BackgroundSize),
  /// The background repeat of the element.
  BackgroundRepeat(BackgroundRepeat),
  /// The gap of the element.
  Gap(LengthUnit),
  /// The gap of the element on the x-axis.
  GapX(LengthUnit),
  /// The gap of the element on the y-axis.
  GapY(LengthUnit),
  /// The width of the border of the element.
  BorderWidth(LengthUnit),
  /// The color of the element.
  Color(ColorInput),
  /// The opacity of the element.
  Opacity(PercentageNumber),
  /// The background color of the element.
  BackgroundColor(ColorInput),
  /// The border color of the element.
  BorderColor(ColorInput),
}

/// A trait for parsing tailwind properties.
pub trait TailwindPropertyParser: Sized {
  /// Parse a tailwind property from a token.
  fn parse_tw(token: &str) -> Option<Self>;
}

impl TailwindProperty {
  /// Parse a list of tailwind properties from a string.
  pub fn parse_list(property: &str) -> impl Iterator<Item = TailwindProperty> {
    property.split_whitespace().filter_map(Self::parse)
  }

  /// Parse a single tailwind property from a token.
  pub fn parse(token: &str) -> Option<TailwindProperty> {
    // Check fixed properties first
    if let Some(property) = FIXED_PROPERTIES.get(token) {
      return Some(property.clone());
    }

    // Handle negative values like "-top-4"
    if let Some(stripped) = token.strip_prefix('-') {
      if let Some(property) = Self::parse_prefix_suffix(stripped) {
        return Some(property);
      }
    }

    Self::parse_prefix_suffix(token)
  }

  fn parse_prefix_suffix(token: &str) -> Option<TailwindProperty> {
    let dash_positions = token.match_indices('-').map(|(i, _)| i);

    // Try different prefix lengths (longest first)
    for dash_pos in dash_positions.rev() {
      let prefix = &token[..dash_pos];

      let Some(parsers) = PREFIX_PARSERS.get(prefix) else {
        continue;
      };

      let suffix = &token[dash_pos + 1..];

      for parser in *parsers {
        if let Some(property) = parser(suffix) {
          return Some(property);
        }
      }
    }

    None
  }

  pub(crate) fn apply(&self, style: &mut Style) {
    match *self {
      TailwindProperty::Gap(gap) => {
        style.gap = SpacePair::from_single(gap).into();
      }
      TailwindProperty::GapX(gap_x) => {
        style.column_gap = CssOption::some(gap_x).into();
      }
      TailwindProperty::GapY(gap_y) => {
        style.row_gap = CssOption::some(gap_y).into();
      }
      TailwindProperty::BoxSizing(box_sizing) => {
        style.box_sizing = box_sizing.into();
      }
      TailwindProperty::FlexGrow(flex_grow) => {
        style.flex_grow = CssOption::some(flex_grow).into();
      }
      TailwindProperty::FlexShrink(flex_shrink) => {
        style.flex_shrink = CssOption::some(flex_shrink).into();
      }
      TailwindProperty::Aspect(ratio) => {
        style.aspect_ratio = ratio.into();
      }
      TailwindProperty::Items(align_items) => {
        style.align_items = align_items.into();
      }
      TailwindProperty::Justify(justify_content) => {
        style.justify_content = justify_content.into();
      }
      TailwindProperty::Content(align_content) => {
        style.align_content = align_content.into();
      }
      TailwindProperty::AlignSelf(align_self) => {
        style.align_self = align_self.into();
      }
      TailwindProperty::FlexDirection(flex_direction) => {
        style.flex_direction = flex_direction.into();
      }
      TailwindProperty::FlexWrap(flex_wrap) => {
        style.flex_wrap = flex_wrap.into();
      }
      TailwindProperty::Flex(flex) => {
        style.flex = CssOption::some(flex).into();
      }
      TailwindProperty::FlexBasis(flex_basis) => {
        style.flex_basis = CssOption::some(flex_basis).into();
      }
      TailwindProperty::Overflow(overflow) => {
        style.overflow = Overflows(SpacePair::from_single(overflow)).into();
      }
      TailwindProperty::Position(position) => {
        style.position = position.into();
      }
      TailwindProperty::FontStyle(font_style) => {
        style.font_style = font_style.into();
      }
      TailwindProperty::FontWeight(font_weight) => {
        style.font_weight = font_weight.into();
      }
      TailwindProperty::FontFamily(ref font_family) => {
        style.font_family = CssOption::some(font_family.clone()).into();
      }
      TailwindProperty::LineClamp(ref line_clamp) => {
        style.line_clamp = CssOption::some(line_clamp.clone()).into();
      }
      TailwindProperty::TextAlign(text_align) => {
        style.text_align = text_align.into();
      }
      TailwindProperty::TextDecoration(ref text_decoration) => {
        style.text_decoration = text_decoration.clone().into();
      }
      TailwindProperty::TextTransform(text_transform) => {
        style.text_transform = text_transform.into();
      }
      TailwindProperty::Size(size) => {
        style.width = size.into();
        style.height = size.into();
      }
      TailwindProperty::Width(width) => {
        style.width = width.into();
      }
      TailwindProperty::Height(height) => {
        style.height = height.into();
      }
      TailwindProperty::MinWidth(min_width) => {
        style.min_width = min_width.into();
      }
      TailwindProperty::MinHeight(min_height) => {
        style.min_height = min_height.into();
      }
      TailwindProperty::MaxWidth(max_width) => {
        style.max_width = max_width.into();
      }
      TailwindProperty::MaxHeight(max_height) => {
        style.max_height = max_height.into();
      }
      TailwindProperty::Shadow(box_shadow) => {
        style.box_shadow = CssOption::some(BoxShadows(smallvec![box_shadow])).into();
      }
      TailwindProperty::Display(display) => {
        style.display = display.into();
      }
      TailwindProperty::OverflowX(overflow) => {
        style.overflow_x = CssOption::some(overflow).into();
      }
      TailwindProperty::OverflowY(overflow) => {
        style.overflow_y = CssOption::some(overflow).into();
      }
      TailwindProperty::ObjectPosition(background_position) => {
        style.object_position = background_position.into();
      }
      TailwindProperty::ObjectFit(object_fit) => {
        style.object_fit = object_fit.into();
      }
      TailwindProperty::BackgroundPosition(background_position) => {
        style.background_position =
          CssOption::some(BackgroundPositions(vec![background_position])).into();
      }
      TailwindProperty::BackgroundSize(background_size) => {
        style.background_size = CssOption::some(BackgroundSizes(vec![background_size])).into();
      }
      TailwindProperty::BackgroundRepeat(background_repeat) => {
        style.background_repeat =
          CssOption::some(BackgroundRepeats(vec![background_repeat])).into();
      }
      TailwindProperty::BorderWidth(length_unit) => {
        style.border_width = CssOption::some(Sides([length_unit; 4])).into();
      }
      TailwindProperty::JustifySelf(align_items) => {
        style.justify_self = align_items.into();
      }
      TailwindProperty::Color(color_input) => {
        style.color = color_input.into();
      }
      TailwindProperty::Opacity(percentage_number) => {
        style.opacity = percentage_number.into();
      }
      TailwindProperty::BackgroundColor(color_input) => {
        style.background_color = color_input.into();
      }
      TailwindProperty::BorderColor(color_input) => {
        style.border_color = CssOption::some(color_input).into();
      }
      TailwindProperty::TextOverflow(ref text_overflow) => {
        style.text_overflow = text_overflow.clone().into();
      }
      TailwindProperty::Truncate => {
        style.text_overflow = TextOverflow::Ellipsis.into();
        style.white_space = WhiteSpace {
          text_wrap_mode: TextWrapMode::NoWrap,
          white_space_collapse: WhiteSpaceCollapse::Collapse,
        }
        .into();
        style.overflow = Overflows(SpacePair::from_single(Overflow::Hidden)).into();
      }
      TailwindProperty::TextWrap(text_wrap_mode) => {
        style.text_wrap_mode = CssOption::some(text_wrap_mode).into();
      }
      TailwindProperty::WhiteSpace(white_space) => {
        style.white_space = white_space.into();
      }
      TailwindProperty::WordBreak(word_break) => {
        style.word_break = word_break.into();
      }
      TailwindProperty::OverflowWrap(overflow_wrap) => {
        style.overflow_wrap = overflow_wrap.into();
      }
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_box_sizing() {
    let mut tw_style = Style::default();

    TailwindProperties::from_str("box-border")
      .unwrap()
      .apply(&mut tw_style);

    assert_eq!(
      tw_style,
      Style {
        box_sizing: BoxSizing::BorderBox.into(),
        ..Default::default()
      }
    );
  }

  #[test]
  fn test_parse_width() {
    assert_eq!(
      TailwindProperty::parse("w-64"),
      Some(TailwindProperty::Width(LengthUnit::Rem(64.0 * VAR_SPACING)))
    );
    assert_eq!(
      TailwindProperty::parse("h-32"),
      Some(TailwindProperty::Height(LengthUnit::Rem(
        32.0 * VAR_SPACING
      )))
    );
    assert_eq!(
      TailwindProperty::parse("justify-self-center"),
      Some(TailwindProperty::JustifySelf(AlignItems::Center))
    );
  }

  #[test]
  fn test_parse_color() {
    assert_eq!(
      TailwindProperty::parse("text-black/30"),
      Some(TailwindProperty::Color(ColorInput::Value(Color([
        0,
        0,
        0,
        (0.3_f32 * 255.0).round() as u8
      ]))))
    );
  }
}
