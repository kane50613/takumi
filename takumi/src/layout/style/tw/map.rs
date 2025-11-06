use phf::phf_map;

use crate::layout::style::tw::{TailwindProperty, TailwindPropertyParser};
use crate::layout::style::*;

/// Function type for parsing tailwind properties with suffix.
pub type PropertyParserFn = fn(&str) -> Option<TailwindProperty>;

/// Macro to create parser functions
macro_rules! make_parser {
  ($name:ident, $type:ty, $variant:ident) => {
    fn $name(suffix: &str) -> Option<TailwindProperty> {
      <$type>::parse_tw(suffix).map(TailwindProperty::$variant)
    }
  };
}

// Define all parser functions using the macro
make_parser!(parse_object_fit, ObjectFit, ObjectFit);
make_parser!(parse_object_position, BackgroundPosition, ObjectPosition);
make_parser!(parse_bg_position, BackgroundPosition, BackgroundPosition);
make_parser!(parse_bg_size, BackgroundSize, BackgroundSize);
make_parser!(parse_bg_repeat, BackgroundRepeat, BackgroundRepeat);
make_parser!(parse_width, LengthUnit, Width);
make_parser!(parse_height, LengthUnit, Height);
make_parser!(parse_min_width, LengthUnit, MinWidth);
make_parser!(parse_min_height, LengthUnit, MinHeight);
make_parser!(parse_max_width, LengthUnit, MaxWidth);
make_parser!(parse_max_height, LengthUnit, MaxHeight);
make_parser!(parse_size, LengthUnit, Size);
make_parser!(parse_font_weight, FontWeight, FontWeight);
make_parser!(parse_gap_x, LengthUnit, GapX);
make_parser!(parse_gap_y, LengthUnit, GapY);
make_parser!(parse_gap, LengthUnit, Gap);
make_parser!(parse_justify, JustifyContent, Justify);
make_parser!(parse_content, JustifyContent, Content);
make_parser!(parse_items, AlignItems, Items);
make_parser!(parse_align_self, AlignItems, AlignSelf);
make_parser!(parse_justify_self, AlignItems, JustifySelf);
make_parser!(parse_overflow_x, Overflow, OverflowX);
make_parser!(parse_overflow_y, Overflow, OverflowY);
make_parser!(parse_overflow, Overflow, Overflow);
make_parser!(parse_border_width, LengthUnit, BorderWidth);
make_parser!(parse_flex_grow, FlexGrow, FlexGrow);
make_parser!(parse_flex_shrink, FlexGrow, FlexShrink);
make_parser!(parse_aspect, AspectRatio, Aspect);
make_parser!(parse_align, TextAlign, TextAlign);
make_parser!(parse_text_color, ColorInput, Color);
make_parser!(parse_opacity, PercentageNumber, Opacity);
make_parser!(parse_background_color, ColorInput, BackgroundColor);
make_parser!(parse_border_color, ColorInput, BorderColor);

pub static PREFIX_PARSERS: phf::Map<&str, &[PropertyParserFn]> = phf_map! {
  "object" => &[parse_object_fit, parse_object_position],
  "bg" => &[parse_background_color, parse_bg_position, parse_bg_size, parse_bg_repeat],
  "w" => &[parse_width],
  "h" => &[parse_height],
  "min-w" => &[parse_min_width],
  "min-h" => &[parse_min_height],
  "max-w" => &[parse_max_width],
  "max-h" => &[parse_max_height],
  "size" => &[parse_size],
  "font" => &[parse_font_weight],
  "gap-x" => &[parse_gap_x],
  "gap-y" => &[parse_gap_y],
  "gap" => &[parse_gap],
  "justify" => &[parse_justify],
  "content" => &[parse_content],
  "items" => &[parse_items],
  "self" => &[parse_align_self],
  "justify-self" => &[parse_justify_self],
  "overflow-x" => &[parse_overflow_x],
  "overflow-y" => &[parse_overflow_y],
  "overflow" => &[parse_overflow],
  "border" => &[parse_border_color, parse_border_width],
  "grow" => &[parse_flex_grow],
  "shrink" => &[parse_flex_shrink],
  "aspect" => &[parse_aspect],
  "text" => &[parse_text_color, parse_align],
  "opacity" => &[parse_opacity],
};

pub static FIXED_PROPERTIES: phf::Map<&str, TailwindProperty> = phf_map! {
  "border" => TailwindProperty::BorderWidth(LengthUnit::Px(1.0)),
  "box-border" => TailwindProperty::BoxSizing(BoxSizing::BorderBox),
  "box-content" => TailwindProperty::BoxSizing(BoxSizing::ContentBox),
  "inline" => TailwindProperty::Display(Display::Inline),
  "block" => TailwindProperty::Display(Display::Block),
  "flex" => TailwindProperty::Display(Display::Flex),
  "grid" => TailwindProperty::Display(Display::Grid),
  "hidden" => TailwindProperty::Display(Display::None),
  "aspect-auto" => TailwindProperty::Aspect(AspectRatio::Auto),
  "aspect-square" => TailwindProperty::Aspect(AspectRatio::Ratio(1.0)),
  "aspect-video" => TailwindProperty::Aspect(AspectRatio::Ratio(16.0 / 9.0)),
  "flex-grow" | "grow" => TailwindProperty::FlexGrow(FlexGrow(1.0)),
  "flex-shrink" | "shrink" => TailwindProperty::FlexShrink(FlexGrow(1.0)),
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
  "absolute" => TailwindProperty::Position(Position::Absolute),
  "relative" => TailwindProperty::Position(Position::Relative),
  "uppercase" => TailwindProperty::TextTransform(TextTransform::Uppercase),
  "lowercase" => TailwindProperty::TextTransform(TextTransform::Lowercase),
  "capitalize" => TailwindProperty::TextTransform(TextTransform::Capitalize),
  "normal-case" => TailwindProperty::TextTransform(TextTransform::None),
  "italic" => TailwindProperty::FontStyle(FontStyle::italic()),
  "not-italic" => TailwindProperty::FontStyle(FontStyle::normal()),
  "basis-auto" => TailwindProperty::FlexBasis(LengthUnit::Auto),
  "flex-basis-auto" => TailwindProperty::FlexBasis(LengthUnit::Auto),
  "w-screen" => TailwindProperty::Width(LengthUnit::Vw(100.0)),
  "h-screen" => TailwindProperty::Height(LengthUnit::Vh(100.0)),
  "min-w-screen" => TailwindProperty::MinWidth(LengthUnit::Vw(100.0)),
  "min-h-screen" => TailwindProperty::MinHeight(LengthUnit::Vh(100.0)),
  "max-w-screen" => TailwindProperty::MaxWidth(LengthUnit::Vw(100.0)),
  "max-h-screen" => TailwindProperty::MaxHeight(LengthUnit::Vh(100.0)),
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
