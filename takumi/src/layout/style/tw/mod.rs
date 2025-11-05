use phf::phf_map;
use smallvec::smallvec;

use crate::layout::style::*;

/// Represents a tailwind property.
#[derive(Debug, Clone, PartialEq)]
pub enum TailwindProperty {
  /// The aspect ratio of the element.
  Aspect(f32),
  /// The alignment of the items in the element.
  Items(AlignItems),
  /// The justification of the content in the element.
  Justify(JustifyContent),
  /// The alignment of the content in the element.
  Content(JustifyContent),
  /// The alignment of the self in the element.
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
  /// The position of the element.
  Position(Position),
  /// The style of the font in the element.
  FontStyle(FontStyle),
  /// The weight of the font in the element.
  FontWeight(FontWeight),
  /// The alignment of the text in the element.
  Text(TextAlign),
  /// The decoration of the text in the element.
  TextDecoration(TextDecoration),
  /// The transformation of the text in the element.
  TextTransform(TextTransform),
  /// The width of the element.
  Width(LengthUnit),
  /// The height of the element.
  Height(LengthUnit),
  /// The shadow of the element.
  Shadow(BoxShadow),
  /// The display of the element.
  Display(Display),
}

impl TailwindProperty {
  /// Parse a list of tailwind properties from a string.
  pub fn parse_list(property: &str) -> Option<Vec<TailwindProperty>> {
    let mut properties = Vec::new();

    for token in property.split_whitespace() {
      if let Some(property) = Self::parse(token) {
        properties.push(property);
      }
    }

    if properties.is_empty() {
      None
    } else {
      Some(properties)
    }
  }

  /// Parse a single tailwind property from a token.
  pub fn parse(token: &str) -> Option<TailwindProperty> {
    if let Some(property) = FIXED_PROPERTIES.get(token) {
      return Some(property.clone());
    }

    None
  }

  pub(crate) fn apply(&self, style: &mut InheritedStyle) {
    match self {
      TailwindProperty::Aspect(ratio) => {
        style.aspect_ratio = AspectRatio::Ratio(*ratio);
      }
      TailwindProperty::Items(align_items) => {
        style.align_items = *align_items;
      }
      TailwindProperty::Justify(justify_content) => {
        style.justify_content = *justify_content;
      }
      TailwindProperty::Content(align_content) => {
        style.align_content = *align_content;
      }
      TailwindProperty::AlignSelf(align_self) => {
        style.align_self = *align_self;
      }
      TailwindProperty::FlexDirection(flex_direction) => {
        style.flex_direction = *flex_direction;
      }
      TailwindProperty::FlexWrap(flex_wrap) => {
        style.flex_wrap = *flex_wrap;
      }
      TailwindProperty::Flex(flex) => {
        style.flex = CssOption::some(*flex);
      }
      TailwindProperty::FlexBasis(flex_basis) => {
        style.flex_basis = CssOption::some(*flex_basis);
      }
      TailwindProperty::Overflow(overflow) => {
        style.overflow = Overflows(*overflow, *overflow);
      }
      TailwindProperty::Position(position) => {
        style.position = *position;
      }
      TailwindProperty::FontStyle(font_style) => {
        style.font_style = *font_style;
      }
      TailwindProperty::FontWeight(font_weight) => {
        style.font_weight = *font_weight;
      }
      TailwindProperty::Text(text_align) => {
        style.text_align = *text_align;
      }
      TailwindProperty::TextDecoration(text_decoration) => {
        style.text_decoration = text_decoration.clone();
      }
      TailwindProperty::TextTransform(text_transform) => {
        style.text_transform = *text_transform;
      }
      TailwindProperty::Width(width) => {
        style.width = *width;
      }
      TailwindProperty::Height(height) => {
        style.height = *height;
      }
      TailwindProperty::Shadow(box_shadow) => {
        style.box_shadow = CssOption::some(BoxShadows(smallvec![*box_shadow]));
      }
      TailwindProperty::Display(display) => {
        style.display = *display;
      }
    }
  }
}

static FIXED_PROPERTIES: phf::Map<&str, TailwindProperty> = phf_map! {
  "inline" => TailwindProperty::Display(Display::Inline),
  "block" => TailwindProperty::Display(Display::Block),
  "flex" => TailwindProperty::Display(Display::Flex),
  "grid" => TailwindProperty::Display(Display::Grid),
  "hidden" => TailwindProperty::Display(Display::None),
  "aspect-square" => TailwindProperty::Aspect(1.0),
  "aspect-video" => TailwindProperty::Aspect(16.0 / 9.0),
  "items-center" => TailwindProperty::Items(AlignItems::Center),
  "items-start" => TailwindProperty::Items(AlignItems::Start),
  "items-end" => TailwindProperty::Items(AlignItems::End),
  "items-baseline" => TailwindProperty::Items(AlignItems::Baseline),
  "items-stretch" => TailwindProperty::Items(AlignItems::Stretch),
  "justify-start" => TailwindProperty::Justify(JustifyContent::Start),
  "justify-end" => TailwindProperty::Justify(JustifyContent::End),
  "justify-center" => TailwindProperty::Justify(JustifyContent::Center),
  "justify-between" => TailwindProperty::Justify(JustifyContent::SpaceBetween),
  "justify-around" => TailwindProperty::Justify(JustifyContent::SpaceAround),
  "justify-evenly" => TailwindProperty::Justify(JustifyContent::SpaceEvenly),
  "content-start" => TailwindProperty::Content(JustifyContent::Start),
  "content-end" => TailwindProperty::Content(JustifyContent::End),
  "content-between" => TailwindProperty::Content(JustifyContent::SpaceBetween),
  "content-around" => TailwindProperty::Content(JustifyContent::SpaceAround),
  "content-stretch" => TailwindProperty::Content(JustifyContent::Stretch),
  "content-center" => TailwindProperty::Content(JustifyContent::Center),
  "self-start" => TailwindProperty::AlignSelf(AlignItems::Start),
  "self-end" => TailwindProperty::AlignSelf(AlignItems::End),
  "self-center" => TailwindProperty::AlignSelf(AlignItems::Center),
  "self-stretch" => TailwindProperty::AlignSelf(AlignItems::Stretch),
  "self-baseline" => TailwindProperty::AlignSelf(AlignItems::Baseline),
  "flex-row" => TailwindProperty::FlexDirection(FlexDirection::Row),
  "flex-row-reverse" => TailwindProperty::FlexDirection(FlexDirection::RowReverse),
  "flex-col" => TailwindProperty::FlexDirection(FlexDirection::Column),
  "flex-col-reverse" => TailwindProperty::FlexDirection(FlexDirection::ColumnReverse),
  "flex-wrap" => TailwindProperty::FlexWrap(FlexWrap::Wrap),
  "flex-wrap-reverse" => TailwindProperty::FlexWrap(FlexWrap::WrapReverse),
  "flex-nowrap" => TailwindProperty::FlexWrap(FlexWrap::NoWrap),
  "flex-auto" => TailwindProperty::Flex(Flex::auto()),
  "flex-initial" => TailwindProperty::Flex(Flex::initial()),
  "flex-none" => TailwindProperty::Flex(Flex::none()),
  "overflow-hidden" => TailwindProperty::Overflow(Overflow::Hidden),
  "overflow-visible" => TailwindProperty::Overflow(Overflow::Visible),
  "absolute" => TailwindProperty::Position(Position::Absolute),
  "relative" => TailwindProperty::Position(Position::Relative),
  "text-left" => TailwindProperty::Text(TextAlign::Left),
  "text-center" => TailwindProperty::Text(TextAlign::Center),
  "text-right" => TailwindProperty::Text(TextAlign::Right),
  "text-justify" => TailwindProperty::Text(TextAlign::Justify),
  "text-auto" => TailwindProperty::Text(TextAlign::Start),
  "uppercase" => TailwindProperty::TextTransform(TextTransform::Uppercase),
  "lowercase" => TailwindProperty::TextTransform(TextTransform::Lowercase),
  "capitalize" => TailwindProperty::TextTransform(TextTransform::Capitalize),
  "normal-case" => TailwindProperty::TextTransform(TextTransform::None),
  "w-auto" => TailwindProperty::Width(LengthUnit::Auto),
  "h-auto" => TailwindProperty::Height(LengthUnit::Auto),
  "basis-auto" => TailwindProperty::FlexBasis(LengthUnit::Auto),
  "flex-basis-auto" => TailwindProperty::FlexBasis(LengthUnit::Auto),
  "shadow-sm" => TailwindProperty::Shadow(BoxShadow {
    inset: false,
    offset_x: LengthUnit::Px(1.0),
    offset_y: LengthUnit::Px(1.0),
    blur_radius: LengthUnit::Px(1.0),
    spread_radius: LengthUnit::Px(0.0),
    color: ColorInput::Value(Color([0, 0, 0, 6])),
  }),
  "shadow" => TailwindProperty::Shadow(BoxShadow {
    inset: false,
    offset_x: LengthUnit::Px(1.0),
    offset_y: LengthUnit::Px(1.0),
    blur_radius: LengthUnit::Px(1.0),
    spread_radius: LengthUnit::Px(0.0),
    color: ColorInput::Value(Color([0, 0, 0, 19])),
  }),
  "shadow-md" => TailwindProperty::Shadow(BoxShadow {
    inset: false,
    offset_x: LengthUnit::Px(1.0),
    offset_y: LengthUnit::Px(1.0),
    blur_radius: LengthUnit::Px(3.0),
    spread_radius: LengthUnit::Px(0.0),
    color: ColorInput::Value(Color([0, 0, 0, 32])),
  }),
  "shadow-lg" => TailwindProperty::Shadow(BoxShadow {
    inset: false,
    offset_x: LengthUnit::Px(1.0),
    offset_y: LengthUnit::Px(1.0),
    blur_radius: LengthUnit::Px(8.0),
    spread_radius: LengthUnit::Px(0.0),
    color: ColorInput::Value(Color([0, 0, 0, 38])),
  }),
  "shadow-xl" => TailwindProperty::Shadow(BoxShadow {
    inset: false,
    offset_x: LengthUnit::Px(1.0),
    offset_y: LengthUnit::Px(1.0),
    blur_radius: LengthUnit::Px(20.0),
    spread_radius: LengthUnit::Px(0.0),
    color: ColorInput::Value(Color([0, 0, 0, 48])),
  }),
  "shadow-2xl" => TailwindProperty::Shadow(BoxShadow {
    inset: false,
    offset_x: LengthUnit::Px(1.0),
    offset_y: LengthUnit::Px(1.0),
    blur_radius: LengthUnit::Px(30.0),
    spread_radius: LengthUnit::Px(0.0),
    color: ColorInput::Value(Color([0, 0, 0, 64])),
  }),
  "shadow-none" => TailwindProperty::Shadow(BoxShadow {
    inset: false,
    offset_x: LengthUnit::Px(0.0),
    offset_y: LengthUnit::Px(0.0),
    blur_radius: LengthUnit::Px(0.0),
    spread_radius: LengthUnit::Px(0.0),
    color: ColorInput::Value(Color([0, 0, 0, 0])),
  }),
};
