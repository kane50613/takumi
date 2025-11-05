use phf::phf_map;
use serde::Deserializer;
use smallvec::smallvec;

use crate::layout::style::*;

/// Represents a collection of tailwind properties.
#[derive(Debug, Clone)]
pub struct TailwindProperties {
  inner: Vec<TailwindProperty>,
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
    let string = <&str>::deserialize(deserializer)?;

    Ok(TailwindProperties {
      inner: TailwindProperty::parse_list(string).collect(),
    })
  }
}

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
  Overflow(Overflows),
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
  /// The alignment of the text in the element.
  Text(TextAlign),
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
}

/// A trait for parsing tailwind properties.
pub trait TailwindPropertyParser: Sized {
  /// Parse a tailwind property from a token.
  fn parse_tw(token: &str) -> Option<Self>;
}

// property_check!("object-", ObjectPosition) -> Option<TailwindProperty>
macro_rules! property_check {
  ($prefix:literal, $wrapper:ident($property:ident), $token:ident) => {
    if $token.starts_with($prefix)
      && let Some(property) = $property::parse_tw(&$token[$prefix.len()..])
    {
      return Some(TailwindProperty::$wrapper(property));
    }
  };
}

impl TailwindProperty {
  /// Parse a list of tailwind properties from a string.
  pub fn parse_list(property: &str) -> impl Iterator<Item = TailwindProperty> {
    property.split_whitespace().filter_map(Self::parse)
  }

  /// Parse a single tailwind property from a token.
  pub fn parse(token: &str) -> Option<TailwindProperty> {
    if let Some(property) = FIXED_PROPERTIES.get(token) {
      return Some(property.clone());
    }

    property_check!("object-", ObjectFit(ObjectFit), token);
    property_check!("object-", ObjectPosition(BackgroundPosition), token);
    property_check!("bg-", BackgroundPosition(BackgroundPosition), token);
    property_check!("bg-", BackgroundSize(BackgroundSize), token);
    property_check!("bg-", BackgroundRepeat(BackgroundRepeat), token);
    property_check!("w-", Width(LengthUnit), token);
    property_check!("h-", Height(LengthUnit), token);
    property_check!("min-w-", MinWidth(LengthUnit), token);
    property_check!("min-h-", MinHeight(LengthUnit), token);
    property_check!("max-w-", MaxWidth(LengthUnit), token);
    property_check!("max-h-", MaxHeight(LengthUnit), token);
    property_check!("size-", Size(LengthUnit), token);
    property_check!("font-", FontWeight(FontWeight), token);

    None
  }

  pub(crate) fn apply(&self, style: &mut Style) {
    match *self {
      TailwindProperty::Aspect(ratio) => {
        style.aspect_ratio = AspectRatio::Ratio(ratio).into();
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
        style.overflow = overflow.into();
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
      TailwindProperty::Text(text_align) => {
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
  "overflow-hidden" => TailwindProperty::Overflow(Overflows(SpacePair::from_single(Overflow::Hidden))),
  "overflow-visible" => TailwindProperty::Overflow(Overflows(SpacePair::from_single(Overflow::Visible))),
  "overflow-x-hidden" => TailwindProperty::OverflowX(Overflow::Hidden),
  "overflow-y-hidden" => TailwindProperty::OverflowY(Overflow::Hidden),
  "overflow-x-visible" => TailwindProperty::OverflowX(Overflow::Visible),
  "overflow-y-visible" => TailwindProperty::OverflowY(Overflow::Visible),
  "absolute" => TailwindProperty::Position(Position::Absolute),
  "relative" => TailwindProperty::Position(Position::Relative),
  "text-start" => TailwindProperty::Text(TextAlign::Start),
  "text-end" => TailwindProperty::Text(TextAlign::End),
  "text-left" => TailwindProperty::Text(TextAlign::Left),
  "text-center" => TailwindProperty::Text(TextAlign::Center),
  "text-right" => TailwindProperty::Text(TextAlign::Right),
  "text-justify" => TailwindProperty::Text(TextAlign::Justify),
  "text-auto" => TailwindProperty::Text(TextAlign::Start),
  "uppercase" => TailwindProperty::TextTransform(TextTransform::Uppercase),
  "lowercase" => TailwindProperty::TextTransform(TextTransform::Lowercase),
  "capitalize" => TailwindProperty::TextTransform(TextTransform::Capitalize),
  "normal-case" => TailwindProperty::TextTransform(TextTransform::None),
  "italic" => TailwindProperty::FontStyle(FontStyle::italic()),
  "not-italic" => TailwindProperty::FontStyle(FontStyle::normal()),
  "basis-auto" => TailwindProperty::FlexBasis(LengthUnit::Auto),
  "flex-basis-auto" => TailwindProperty::FlexBasis(LengthUnit::Auto),
  "w-screen" | "w-dvw" => TailwindProperty::Width(LengthUnit::Vw(100.0)),
  "h-screen" | "h-dvh" => TailwindProperty::Height(LengthUnit::Vh(100.0)),
  "min-w-scren" | "min-w-dvw" => TailwindProperty::MinWidth(LengthUnit::Vw(100.0)),
  "min-h-screen" | "min-h-dvh" => TailwindProperty::MinHeight(LengthUnit::Vh(100.0)),
  "max-w-screen" | "max-w-dvw" => TailwindProperty::MaxWidth(LengthUnit::Vw(100.0)),
  "max-h-screen" | "max-h-dvh" => TailwindProperty::MaxHeight(LengthUnit::Vh(100.0)),
  "size-dvw" => TailwindProperty::Width(LengthUnit::Vw(100.0)),
  "size-dvh" => TailwindProperty::Height(LengthUnit::Vh(100.0)),
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
