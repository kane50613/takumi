use phf::phf_map;
use std::borrow::Cow;

use crate::layout::style::{
  tw::{TailwindProperty, TailwindPropertyParser, parser::*},
  *,
};

/// Function type for parsing tailwind properties with suffix.
pub type PropertyParserFn = fn(&str) -> Option<TailwindProperty>;

/// Macro to create parser functions
macro_rules! make_parser {
  ($name:ident, $type:ty, $variant:ident) => {
    fn $name(suffix: &str) -> Option<TailwindProperty> {
      if suffix.starts_with('[') && suffix.ends_with(']') {
        let value = &suffix[1..suffix.len() - 1];
        let value = if value.contains('_') {
          Cow::Owned(value.replace('_', " "))
        } else {
          Cow::Borrowed(value)
        };

        return <$type>::from_str(&value)
          .ok()
          .map(TailwindProperty::$variant);
      }

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
make_parser!(parse_border_width, TwBorderWidth, BorderWidth);
make_parser!(parse_border_top_width, TwBorderWidth, BorderTopWidth);
make_parser!(parse_border_right_width, TwBorderWidth, BorderRightWidth);
make_parser!(parse_border_bottom_width, TwBorderWidth, BorderBottomWidth);
make_parser!(parse_border_left_width, TwBorderWidth, BorderLeftWidth);
make_parser!(parse_border_x_width, TwBorderWidth, BorderXWidth);
make_parser!(parse_border_y_width, TwBorderWidth, BorderYWidth);
make_parser!(parse_border_radius, TwRound, BorderRadius);
make_parser!(parse_border_tl_radius, LengthUnit, BorderTopLeftRadius);
make_parser!(parse_border_tr_radius, LengthUnit, BorderTopRightRadius);
make_parser!(parse_border_br_radius, LengthUnit, BorderBottomRightRadius);
make_parser!(parse_border_bl_radius, LengthUnit, BorderBottomLeftRadius);
make_parser!(parse_rounded_t, LengthUnit, RoundedT);
make_parser!(parse_rounded_r, LengthUnit, RoundedR);
make_parser!(parse_rounded_b, LengthUnit, RoundedB);
make_parser!(parse_rounded_l, LengthUnit, RoundedL);
make_parser!(
  parse_grid_template_columns,
  TwGridTemplate,
  GridTemplateColumns
);
make_parser!(parse_grid_template_rows, TwGridTemplate, GridTemplateRows);
make_parser!(parse_grid_auto_columns, TwGridAutoSize, GridAutoColumns);
make_parser!(parse_grid_auto_rows, TwGridAutoSize, GridAutoRows);
make_parser!(parse_grid_column_start, TwGridPlacement, GridColumnStart);
make_parser!(parse_grid_column_end, TwGridPlacement, GridColumnEnd);
make_parser!(parse_grid_row_start, TwGridPlacement, GridRowStart);
make_parser!(parse_grid_row_end, TwGridPlacement, GridRowEnd);
make_parser!(parse_grid_column, TwGridSpan, GridColumn);
make_parser!(parse_grid_row, TwGridSpan, GridRow);
make_parser!(parse_letter_spacing, TwLetterSpacing, LetterSpacing);
make_parser!(parse_flex_grow, FlexGrow, FlexGrow);
make_parser!(parse_flex_shrink, FlexGrow, FlexShrink);
make_parser!(parse_aspect, AspectRatio, Aspect);
make_parser!(parse_align, TextAlign, TextAlign);
make_parser!(parse_text_color, ColorInput, Color);
make_parser!(parse_text_decoration_color, ColorInput, TextDecorationColor);
make_parser!(parse_opacity, PercentageNumber, Opacity);
make_parser!(parse_background_color, ColorInput, BackgroundColor);
make_parser!(parse_border_color, ColorInput, BorderColor);
make_parser!(parse_font_family, FontFamily, FontFamily);
make_parser!(parse_line_clamp, LineClamp, LineClamp);
make_parser!(parse_white_space, WhiteSpace, WhiteSpace);
make_parser!(parse_overflow_wrap, OverflowWrap, OverflowWrap);
make_parser!(parse_font_size, TwFontSize, FontSize);
make_parser!(parse_line_height, LineHeight, LineHeight);
make_parser!(parse_basis, LengthUnit, FlexBasis);
make_parser!(parse_flex, Flex, Flex);
make_parser!(parse_justify_items, AlignItems, JustifyItems);
make_parser!(parse_rotate, Angle, Rotate);
make_parser!(parse_scale, PercentageNumber, Scale);
make_parser!(parse_scale_x, PercentageNumber, ScaleX);
make_parser!(parse_scale_y, PercentageNumber, ScaleY);
make_parser!(parse_transform_origin, BackgroundPosition, TransformOrigin);
make_parser!(parse_translate, LengthUnit, Translate);
make_parser!(parse_translate_x, LengthUnit, TranslateX);
make_parser!(parse_translate_y, LengthUnit, TranslateY);
make_parser!(parse_margin, LengthUnit, Margin);
make_parser!(parse_margin_x, LengthUnit, MarginX);
make_parser!(parse_margin_y, LengthUnit, MarginY);
make_parser!(parse_margin_top, LengthUnit, MarginTop);
make_parser!(parse_margin_right, LengthUnit, MarginRight);
make_parser!(parse_margin_bottom, LengthUnit, MarginBottom);
make_parser!(parse_margin_left, LengthUnit, MarginLeft);
make_parser!(parse_padding, LengthUnit, Padding);
make_parser!(parse_padding_x, LengthUnit, PaddingX);
make_parser!(parse_padding_y, LengthUnit, PaddingY);
make_parser!(parse_padding_top, LengthUnit, PaddingTop);
make_parser!(parse_padding_right, LengthUnit, PaddingRight);
make_parser!(parse_padding_bottom, LengthUnit, PaddingBottom);
make_parser!(parse_padding_left, LengthUnit, PaddingLeft);
make_parser!(parse_inset, LengthUnit, Inset);
make_parser!(parse_inset_x, LengthUnit, InsetX);
make_parser!(parse_inset_y, LengthUnit, InsetY);
make_parser!(parse_top, LengthUnit, Top);
make_parser!(parse_right, LengthUnit, Right);
make_parser!(parse_bottom, LengthUnit, Bottom);
make_parser!(parse_left, LengthUnit, Left);

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
  "font" => &[parse_font_weight, parse_font_family],
  "gap-x" => &[parse_gap_x],
  "gap-y" => &[parse_gap_y],
  "gap" => &[parse_gap],
  "justify" => &[parse_justify],
  "content" => &[parse_content],
  "items" => &[parse_items],
  "self" => &[parse_align_self],
  "justify-self" => &[parse_justify_self],
  "justify-items" => &[parse_justify_items],
  "overflow-x" => &[parse_overflow_x],
  "overflow-y" => &[parse_overflow_y],
  "overflow" => &[parse_overflow],
  "border" => &[parse_border_color, parse_border_width],
  "border-t" => &[parse_border_top_width],
  "border-r" => &[parse_border_right_width],
  "border-b" => &[parse_border_bottom_width],
  "border-l" => &[parse_border_left_width],
  "border-x" => &[parse_border_x_width],
  "border-y" => &[parse_border_y_width],
  "grow" | "flex-grow" => &[parse_flex_grow],
  "shrink" | "flex-shrink" => &[parse_flex_shrink],
  "basis" | "flex-basis" => &[parse_basis],
  "aspect" => &[parse_aspect],
  "text" => &[parse_font_size, parse_text_color, parse_align],
  "decoration" => &[parse_text_decoration_color],
  "leading" => &[parse_line_height],
  "opacity" => &[parse_opacity],
  "line-clamp" => &[parse_line_clamp],
  "whitespace" => &[parse_white_space],
  "wrap" => &[parse_overflow_wrap],
  "flex" => &[parse_flex],
  "origin" => &[parse_transform_origin],
  "translate" => &[parse_translate],
  "rotate" => &[parse_rotate],
  "scale" => &[parse_scale],
  "scale-x" => &[parse_scale_x],
  "scale-y" => &[parse_scale_y],
  "translate-x" => &[parse_translate_x],
  "translate-y" => &[parse_translate_y],
  "m" => &[parse_margin],
  "mx" | "ms" => &[parse_margin_x],
  "my" | "me" => &[parse_margin_y],
  "mt" => &[parse_margin_top],
  "mr" => &[parse_margin_right],
  "mb" => &[parse_margin_bottom],
  "ml" => &[parse_margin_left],
  "p" => &[parse_padding],
  "px" | "ps" => &[parse_padding_x],
  "py" | "pe" => &[parse_padding_y],
  "pt" => &[parse_padding_top],
  "pr" => &[parse_padding_right],
  "pb" => &[parse_padding_bottom],
  "pl" => &[parse_padding_left],
  "inset" => &[parse_inset],
  "inset-x" => &[parse_inset_x],
  "inset-y" => &[parse_inset_y],
  "top" => &[parse_top],
  "right" => &[parse_right],
  "bottom" => &[parse_bottom],
  "left" => &[parse_left],
  "rounded" => &[parse_border_radius],
  "rounded-t" => &[parse_rounded_t],
  "rounded-r" => &[parse_rounded_r],
  "rounded-b" => &[parse_rounded_b],
  "rounded-l" => &[parse_rounded_l],
  "rounded-tl" => &[parse_border_tl_radius],
  "rounded-tr" => &[parse_border_tr_radius],
  "rounded-br" => &[parse_border_br_radius],
  "rounded-bl" => &[parse_border_bl_radius],
  "grid-cols" => &[parse_grid_template_columns],
  "grid-rows" => &[parse_grid_template_rows],
  "auto-cols" => &[parse_grid_auto_columns],
  "auto-rows" => &[parse_grid_auto_rows],
  "col-span" => &[parse_grid_column],
  "row-span" => &[parse_grid_row],
  "col-start" => &[parse_grid_column_start],
  "col-end" => &[parse_grid_column_end],
  "row-start" => &[parse_grid_row_start],
  "row-end" => &[parse_grid_row_end],
  "tracking" => &[parse_letter_spacing],
};

pub static FIXED_PROPERTIES: phf::Map<&str, TailwindProperty> = phf_map! {
  "border" => TailwindProperty::BorderWidth(TwBorderWidth(LengthUnit::Px(1.0))),
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
  "w-screen" => TailwindProperty::Width(LengthUnit::Vw(100.0)),
  "h-screen" => TailwindProperty::Height(LengthUnit::Vh(100.0)),
  "min-w-screen" => TailwindProperty::MinWidth(LengthUnit::Vw(100.0)),
  "min-h-screen" => TailwindProperty::MinHeight(LengthUnit::Vh(100.0)),
  "max-w-screen" => TailwindProperty::MaxWidth(LengthUnit::Vw(100.0)),
  "max-h-screen" => TailwindProperty::MaxHeight(LengthUnit::Vh(100.0)),
  "truncate" => TailwindProperty::Truncate,
  "text-ellipsis" => TailwindProperty::TextOverflow(TextOverflow::Ellipsis),
  "text-clip" => TailwindProperty::TextOverflow(TextOverflow::Clip),
  "text-wrap" => TailwindProperty::TextWrap(TextWrapMode::Wrap),
  "text-nowrap" => TailwindProperty::TextWrap(TextWrapMode::NoWrap),
  "break-normal" => TailwindProperty::WordBreak(WordBreak::Normal),
  "break-all" => TailwindProperty::WordBreak(WordBreak::BreakAll),
  "break-keep" => TailwindProperty::WordBreak(WordBreak::KeepAll),
  "grid-flow-row" => TailwindProperty::GridAutoFlow(GridAutoFlow::row()),
  "grid-flow-col" => TailwindProperty::GridAutoFlow(GridAutoFlow::column()),
  "grid-flow-row-dense" | "grid-flow-dense" => TailwindProperty::GridAutoFlow(GridAutoFlow::row().dense()),
  "grid-flow-col-dense" => TailwindProperty::GridAutoFlow(GridAutoFlow::column().dense()),
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
