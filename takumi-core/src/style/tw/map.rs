use phf::phf_map;

use crate::style::{
  LonghandId,
  tw::{Namespace, TailwindProperty, TailwindPropertyParser, parser::*},
  *,
};

/// The namespaces a parser candidate reads: the value type's own list, or the
/// override an entry spells in brackets.
macro_rules! parser_namespaces {
  ($parse:ty) => {
    <$parse as TailwindPropertyParser>::NAMESPACES
  };
  ($parse:ty, $($namespace:expr),+) => {
    &[$($namespace),+]
  };
}

/// Generates the [`PropertyParser`] enum and its `parse()` dispatch from `(Variant, ArgType, ParseType)` triples.
macro_rules! property_parsers {
  ($($variant:ident($arg:ty) => $parse:ty $([$($namespace:expr),+ $(,)?])?),+ $(,)?) => {
    /// Maps a parsed argument type to a [`TailwindProperty`] constructor.
    #[derive(Clone, Copy)]
    pub(crate) enum PropertyParser {
      $(
        #[doc = concat!("Parser producing `", stringify!($variant), "` properties.")]
        $variant(fn($arg) -> TailwindProperty),
      )+
      /// Parser for gradient stop positions.
      GradientPosition(fn(Length) -> TailwindProperty),
    }

    impl PropertyParser {
      /// The variable namespaces this candidate's value type reads.
      pub fn namespaces(&self) -> &'static [Namespace] {
        match self {
          $(Self::$variant(..) => parser_namespaces!($parse $(, $($namespace),+)?),)+
          Self::GradientPosition(..) => <TwGradientPosition as TailwindPropertyParser>::NAMESPACES,
        }
      }

      /// Parses a utility suffix into a property via the wrapped constructor.
      pub fn parse(&self, suffix: &str) -> Option<TailwindProperty> {
        match self {
          $(Self::$variant(f) => <$parse>::parse_tw_with_arbitrary(suffix).map(f),)+
          Self::GradientPosition(f) => {
            TwGradientPosition::parse_tw_with_arbitrary(suffix).map(|p| f(p.0))
          }
        }
      }
    }
  };
}

property_parsers! {
  ObjectFit(ObjectFit) => ObjectFit,
  ListStyleType(ListStyleType) => ListStyleType,
  ListStylePosition(ListStylePosition) => ListStylePosition,
  ListStyleImage(ListStyleImage) => ListStyleImage,
  ObjectPosition(PositionValue) => PositionValue,
  BgPosition(PositionValue) => PositionValue,
  TransformOrigin(PositionValue) => PositionValue,
  BgSize(BackgroundSize) => BackgroundSize,
  BgImage(BackgroundImage) => BackgroundImage,
  LengthAuto(Length) => Length,
  ContainerLength(Length) => Length [Namespace::Container, Namespace::Spacing],
  LengthZero(Length) => Length,
  FontWeight(FontWeight) => FontWeight,
  Justify(JustifyContent) => JustifyContent,
  Align(AlignItems) => AlignItems,
  Overflow(Overflow) => Overflow,
  BorderWidth(LineWidth) => LineWidth,
  BorderStyle(BorderStyle) => BorderStyle,
  Rounded(TwRounded) => TwRounded,
  GridTemplate(TwGridTemplate) => TwGridTemplate,
  GridAuto(GridTrackSize) => GridTrackSize,
  GridLine(GridLine) => GridLine,
  GridPlacement(GridPlacement) => GridPlacement,
  GridSpan(GridPlacementSpan) => GridPlacementSpan,
  LetterSpacing(TwLetterSpacing) => TwLetterSpacing,
  FlexGrow(FlexGrow) => FlexGrow,
  Aspect(AspectRatio) => AspectRatio,
  TextAlign(TextAlign) => TextAlign,
  TextWrap(TextWrap) => TextWrap,
  ColorCurrent(ColorInput) => ColorInput,
  ColorTransparent(ColorInput) => ColorInput,
  StopColor(TwVarColor) => TwVarColor,
  Percentage(PercentageNumber) => PercentageNumber,
  FontFamily(FontFamily) => FontFamily,
  LineClamp(LineClamp) => LineClamp,
  WhiteSpace(WhiteSpace) => WhiteSpace,
  OverflowWrap(OverflowWrap) => OverflowWrap,
  FontSize(TwFontSize) => TwFontSize,
  LineHeight(LineHeight) => LineHeight,
  Flex(Flex) => Flex,
  Angle(Angle) => Angle,
  BackgroundClip(BackgroundClip) => BackgroundClip,
  Blur(TwBlur) => TwBlur,
  Filter(Filters) => Filters,
  BoxShadow(BoxShadow) => BoxShadow,
  DropShadow(TextShadow) => TextShadow,
  TextShadow(TextShadow) => TextShadow,
  BlendMode(BlendMode) => BlendMode,
  FontStretch(FontStretch) => FontStretch,
  VerticalAlign(VerticalAlign) => VerticalAlign,
  DecorationThickness(TextDecorationThickness) => TextDecorationThickness,
  Animation(Animations) => Animations,
}

/// What a prefix writes for a token the built-in scales do not know, as the
/// namespaces it reads paired with the longhands each one fills. `bg-brand-500`
/// has no built-in value to expand, so this is the only place its target is
/// written down; a prefix reading two namespaces emits one variable per group,
/// and the undefined ones leave their longhands unset.
pub(crate) static VAR_TARGETS: phf::Map<&str, &[(Namespace, &[LonghandId])]> = phf_map! {
  "aspect" => &[(Namespace::Aspect, &[LonghandId::AspectRatio])],
  "basis" => &[(Namespace::Spacing, &[LonghandId::FlexBasis])],
  "bg" => &[(Namespace::Color, &[LonghandId::BackgroundColor])],
  "border" => &[(Namespace::Color, &[LonghandId::BorderTopColor, LonghandId::BorderRightColor, LonghandId::BorderBottomColor, LonghandId::BorderLeftColor])],
  "border-b" => &[(Namespace::Color, &[LonghandId::BorderBottomColor])],
  "border-l" => &[(Namespace::Color, &[LonghandId::BorderLeftColor])],
  "border-r" => &[(Namespace::Color, &[LonghandId::BorderRightColor])],
  "border-t" => &[(Namespace::Color, &[LonghandId::BorderTopColor])],
  "border-x" => &[(Namespace::Color, &[LonghandId::BorderLeftColor, LonghandId::BorderRightColor])],
  "border-y" => &[(Namespace::Color, &[LonghandId::BorderTopColor, LonghandId::BorderBottomColor])],
  "bottom" => &[(Namespace::Spacing, &[LonghandId::Bottom])],
  "decoration" => &[(Namespace::Color, &[LonghandId::TextDecorationColor])],
  "font" => &[(Namespace::Font, &[LonghandId::FontFamily]), (Namespace::FontWeight, &[LonghandId::FontWeight])],
  "gap" => &[(Namespace::Spacing, &[LonghandId::ColumnGap, LonghandId::RowGap])],
  "gap-x" => &[(Namespace::Spacing, &[LonghandId::ColumnGap])],
  "gap-y" => &[(Namespace::Spacing, &[LonghandId::RowGap])],
  "h" => &[(Namespace::Spacing, &[LonghandId::Height])],
  "inset" => &[(Namespace::Spacing, &[LonghandId::Top, LonghandId::Right, LonghandId::Bottom, LonghandId::Left])],
  "inset-x" => &[(Namespace::Spacing, &[LonghandId::Left, LonghandId::Right])],
  "inset-y" => &[(Namespace::Spacing, &[LonghandId::Top, LonghandId::Bottom])],
  "leading" => &[(Namespace::Leading, &[LonghandId::LineHeight])],
  "left" => &[(Namespace::Spacing, &[LonghandId::Left])],
  "m" => &[(Namespace::Spacing, &[LonghandId::MarginTop, LonghandId::MarginRight, LonghandId::MarginBottom, LonghandId::MarginLeft])],
  "max-h" => &[(Namespace::Spacing, &[LonghandId::MaxHeight])],
  // Groups apply in order, so when two namespaces fill the same longhand the
  // one Tailwind prefers goes last.
  "max-w" => &[(Namespace::Spacing, &[LonghandId::MaxWidth]), (Namespace::Container, &[LonghandId::MaxWidth])],
  "mb" => &[(Namespace::Spacing, &[LonghandId::MarginBottom])],
  "me" => &[(Namespace::Spacing, &[LonghandId::MarginInlineEnd])],
  "min-h" => &[(Namespace::Spacing, &[LonghandId::MinHeight])],
  "min-w" => &[(Namespace::Spacing, &[LonghandId::MinWidth])],
  "ml" => &[(Namespace::Spacing, &[LonghandId::MarginLeft])],
  "mr" => &[(Namespace::Spacing, &[LonghandId::MarginRight])],
  "ms" => &[(Namespace::Spacing, &[LonghandId::MarginInlineStart])],
  "mt" => &[(Namespace::Spacing, &[LonghandId::MarginTop])],
  "mx" => &[(Namespace::Spacing, &[LonghandId::MarginLeft, LonghandId::MarginRight])],
  "my" => &[(Namespace::Spacing, &[LonghandId::MarginTop, LonghandId::MarginBottom])],
  "outline" => &[(Namespace::Color, &[LonghandId::OutlineColor])],
  "p" => &[(Namespace::Spacing, &[LonghandId::PaddingTop, LonghandId::PaddingRight, LonghandId::PaddingBottom, LonghandId::PaddingLeft])],
  "pb" => &[(Namespace::Spacing, &[LonghandId::PaddingBottom])],
  "pe" => &[(Namespace::Spacing, &[LonghandId::PaddingInlineEnd])],
  "pl" => &[(Namespace::Spacing, &[LonghandId::PaddingLeft])],
  "pr" => &[(Namespace::Spacing, &[LonghandId::PaddingRight])],
  "ps" => &[(Namespace::Spacing, &[LonghandId::PaddingInlineStart])],
  "pt" => &[(Namespace::Spacing, &[LonghandId::PaddingTop])],
  "px" => &[(Namespace::Spacing, &[LonghandId::PaddingLeft, LonghandId::PaddingRight])],
  "py" => &[(Namespace::Spacing, &[LonghandId::PaddingTop, LonghandId::PaddingBottom])],
  "right" => &[(Namespace::Spacing, &[LonghandId::Right])],
  "rounded" => &[(Namespace::Radius, &[LonghandId::BorderTopLeftRadius, LonghandId::BorderTopRightRadius, LonghandId::BorderBottomRightRadius, LonghandId::BorderBottomLeftRadius])],
  "rounded-b" => &[(Namespace::Radius, &[LonghandId::BorderBottomRightRadius, LonghandId::BorderBottomLeftRadius])],
  "rounded-bl" => &[(Namespace::Radius, &[LonghandId::BorderBottomLeftRadius])],
  "rounded-br" => &[(Namespace::Radius, &[LonghandId::BorderBottomRightRadius])],
  "rounded-l" => &[(Namespace::Radius, &[LonghandId::BorderTopLeftRadius, LonghandId::BorderBottomLeftRadius])],
  "rounded-r" => &[(Namespace::Radius, &[LonghandId::BorderTopRightRadius, LonghandId::BorderBottomRightRadius])],
  "rounded-t" => &[(Namespace::Radius, &[LonghandId::BorderTopLeftRadius, LonghandId::BorderTopRightRadius])],
  "rounded-tl" => &[(Namespace::Radius, &[LonghandId::BorderTopLeftRadius])],
  "rounded-tr" => &[(Namespace::Radius, &[LonghandId::BorderTopRightRadius])],
  "size" => &[(Namespace::Spacing, &[LonghandId::Width, LonghandId::Height])],
  "text" => &[(Namespace::Text, &[LonghandId::FontSize, LonghandId::LineHeight]), (Namespace::Color, &[LonghandId::Color])],
  "top" => &[(Namespace::Spacing, &[LonghandId::Top])],
  "tracking" => &[(Namespace::Tracking, &[LonghandId::LetterSpacing])],
  "w" => &[(Namespace::Spacing, &[LonghandId::Width])],
};

/// Maps a utility prefix to the parsers tried against its suffix.
pub(crate) static PREFIX_PARSERS: phf::Map<&str, &[PropertyParser]> = phf_map! {
  "list" => &[
    PropertyParser::ListStyleType(TailwindProperty::ListStyleType),
    PropertyParser::ListStylePosition(TailwindProperty::ListStylePosition),
  ],
  "list-image" => &[PropertyParser::ListStyleImage(TailwindProperty::ListStyleImage)],
  "object" => &[
    PropertyParser::ObjectFit(TailwindProperty::ObjectFit),
    PropertyParser::ObjectPosition(TailwindProperty::ObjectPosition),
  ],
  "bg" => &[
    PropertyParser::ColorTransparent(TailwindProperty::BackgroundColor),
    PropertyParser::BgImage(TailwindProperty::BackgroundImage),
    PropertyParser::BgPosition(TailwindProperty::BackgroundPosition),
    PropertyParser::BgSize(TailwindProperty::BackgroundSize),
  ],
  "bg-clip" => &[PropertyParser::BackgroundClip(TailwindProperty::BackgroundClip)],
  "mask" => &[PropertyParser::BgImage(TailwindProperty::MaskImage)],
  "bg-linear" => &[PropertyParser::Angle(TailwindProperty::BgLinearAngle)],
  "bg-conic" => &[PropertyParser::Angle(TailwindProperty::BgConicAngle)],
  "from" => &[
    PropertyParser::GradientPosition(TailwindProperty::GradientFromPosition),
    PropertyParser::StopColor(TailwindProperty::GradientFrom),
  ],
  "to" => &[
    PropertyParser::GradientPosition(TailwindProperty::GradientToPosition),
    PropertyParser::StopColor(TailwindProperty::GradientTo),
  ],
  "via" => &[
    PropertyParser::GradientPosition(TailwindProperty::GradientViaPosition),
    PropertyParser::StopColor(TailwindProperty::GradientVia),
  ],
  "bg-size" => &[PropertyParser::BgSize(TailwindProperty::BackgroundSize)],
  "bg-position" => &[PropertyParser::BgPosition(TailwindProperty::BackgroundPosition)],
  "w" => &[PropertyParser::LengthAuto(TailwindProperty::Width)],
  "h" => &[PropertyParser::LengthAuto(TailwindProperty::Height)],
  "min-w" => &[PropertyParser::LengthAuto(TailwindProperty::MinWidth)],
  "min-h" => &[PropertyParser::LengthAuto(TailwindProperty::MinHeight)],
  "max-w" => &[PropertyParser::ContainerLength(TailwindProperty::MaxWidth)],
  "max-h" => &[PropertyParser::LengthAuto(TailwindProperty::MaxHeight)],
  "size" => &[PropertyParser::LengthAuto(TailwindProperty::Size)],
  "font" => &[
    PropertyParser::FontWeight(TailwindProperty::FontWeight),
    PropertyParser::FontFamily(TailwindProperty::FontFamily),
  ],
  "font-stretch" => &[PropertyParser::FontStretch(TailwindProperty::FontStretch)],
  "gap-x" => &[PropertyParser::LengthZero(TailwindProperty::GapX)],
  "gap-y" => &[PropertyParser::LengthZero(TailwindProperty::GapY)],
  "gap" => &[PropertyParser::LengthZero(TailwindProperty::Gap)],
  "justify" => &[PropertyParser::Justify(TailwindProperty::Justify)],
  "content" => &[PropertyParser::Justify(TailwindProperty::Content)],
  "items" => &[PropertyParser::Align(TailwindProperty::Items)],
  "self" => &[PropertyParser::Align(TailwindProperty::AlignSelf)],
  "justify-self" => &[PropertyParser::Align(TailwindProperty::JustifySelf)],
  "justify-items" => &[PropertyParser::Align(TailwindProperty::JustifyItems)],
  "overflow-x" => &[PropertyParser::Overflow(TailwindProperty::OverflowX)],
  "overflow-y" => &[PropertyParser::Overflow(TailwindProperty::OverflowY)],
  "overflow" => &[PropertyParser::Overflow(TailwindProperty::Overflow)],
  "border" => &[
    PropertyParser::ColorCurrent(TailwindProperty::BorderColor),
    PropertyParser::BorderStyle(TailwindProperty::BorderStyle),
    PropertyParser::BorderWidth(TailwindProperty::BorderWidth),
  ],
  "border-t" => &[
    PropertyParser::ColorCurrent(TailwindProperty::BorderTopColor),
    PropertyParser::BorderWidth(TailwindProperty::BorderTopWidth),
  ],
  "border-r" => &[
    PropertyParser::ColorCurrent(TailwindProperty::BorderRightColor),
    PropertyParser::BorderWidth(TailwindProperty::BorderRightWidth),
  ],
  "border-b" => &[
    PropertyParser::ColorCurrent(TailwindProperty::BorderBottomColor),
    PropertyParser::BorderWidth(TailwindProperty::BorderBottomWidth),
  ],
  "border-l" => &[
    PropertyParser::ColorCurrent(TailwindProperty::BorderLeftColor),
    PropertyParser::BorderWidth(TailwindProperty::BorderLeftWidth),
  ],
  "border-x" => &[
    PropertyParser::ColorCurrent(TailwindProperty::BorderXColor),
    PropertyParser::BorderWidth(TailwindProperty::BorderXWidth),
  ],
  "border-y" => &[
    PropertyParser::ColorCurrent(TailwindProperty::BorderYColor),
    PropertyParser::BorderWidth(TailwindProperty::BorderYWidth),
  ],
  "outline" => &[
    PropertyParser::ColorCurrent(TailwindProperty::OutlineColor),
    PropertyParser::BorderStyle(TailwindProperty::OutlineStyle),
    PropertyParser::BorderWidth(TailwindProperty::OutlineWidth),
  ],
  "shadow" => &[
    PropertyParser::StopColor(TailwindProperty::ShadowColor),
    PropertyParser::BoxShadow(TailwindProperty::Shadow),
  ],
  "outline-offset" => &[PropertyParser::BorderWidth(TailwindProperty::OutlineOffset)],
  "grow" | "flex-grow" => &[PropertyParser::FlexGrow(TailwindProperty::FlexGrow)],
  "shrink" | "flex-shrink" => &[PropertyParser::FlexGrow(TailwindProperty::FlexShrink)],
  "basis" | "flex-basis" => &[PropertyParser::LengthAuto(TailwindProperty::FlexBasis)],
  "aspect" => &[PropertyParser::Aspect(TailwindProperty::Aspect)],
  "text" => &[
    PropertyParser::FontSize(TailwindProperty::FontSize),
    PropertyParser::ColorCurrent(TailwindProperty::Color),
    PropertyParser::TextAlign(TailwindProperty::TextAlign),
    PropertyParser::TextWrap(TailwindProperty::TextWrap),
  ],
  "decoration" => &[
    PropertyParser::ColorCurrent(TailwindProperty::TextDecorationColor),
    PropertyParser::DecorationThickness(TailwindProperty::TextDecorationThickness),
  ],
  "leading" => &[PropertyParser::LineHeight(TailwindProperty::LineHeight)],
  "opacity" => &[PropertyParser::Percentage(TailwindProperty::Opacity)],
  "line-clamp" => &[PropertyParser::LineClamp(TailwindProperty::LineClamp)],
  "whitespace" => &[PropertyParser::WhiteSpace(TailwindProperty::WhiteSpace)],
  "wrap" => &[PropertyParser::OverflowWrap(TailwindProperty::OverflowWrap)],
  "flex" => &[PropertyParser::Flex(TailwindProperty::Flex)],
  "origin" => &[PropertyParser::TransformOrigin(TailwindProperty::TransformOrigin)],
  "translate" => &[PropertyParser::LengthAuto(TailwindProperty::Translate)],
  "rotate" => &[PropertyParser::Angle(TailwindProperty::Rotate)],
  "scale" => &[PropertyParser::Percentage(TailwindProperty::Scale)],
  "scale-x" => &[PropertyParser::Percentage(TailwindProperty::ScaleX)],
  "scale-y" => &[PropertyParser::Percentage(TailwindProperty::ScaleY)],
  "translate-x" => &[PropertyParser::LengthAuto(TailwindProperty::TranslateX)],
  "translate-y" => &[PropertyParser::LengthAuto(TailwindProperty::TranslateY)],
  "m" => &[PropertyParser::LengthZero(TailwindProperty::Margin)],
  "mx" => &[PropertyParser::LengthZero(TailwindProperty::MarginX)],
  "my" => &[PropertyParser::LengthZero(TailwindProperty::MarginY)],
  "mt" => &[PropertyParser::LengthZero(TailwindProperty::MarginTop)],
  "mr" => &[PropertyParser::LengthZero(TailwindProperty::MarginRight)],
  "mb" => &[PropertyParser::LengthZero(TailwindProperty::MarginBottom)],
  "ml" => &[PropertyParser::LengthZero(TailwindProperty::MarginLeft)],
  "ms" => &[PropertyParser::LengthZero(TailwindProperty::MarginInlineStart)],
  "me" => &[PropertyParser::LengthZero(TailwindProperty::MarginInlineEnd)],
  "p" => &[PropertyParser::LengthZero(TailwindProperty::Padding)],
  "px" => &[PropertyParser::LengthZero(TailwindProperty::PaddingX)],
  "py" => &[PropertyParser::LengthZero(TailwindProperty::PaddingY)],
  "pt" => &[PropertyParser::LengthZero(TailwindProperty::PaddingTop)],
  "pr" => &[PropertyParser::LengthZero(TailwindProperty::PaddingRight)],
  "pb" => &[PropertyParser::LengthZero(TailwindProperty::PaddingBottom)],
  "pl" => &[PropertyParser::LengthZero(TailwindProperty::PaddingLeft)],
  "ps" => &[PropertyParser::LengthZero(TailwindProperty::PaddingInlineStart)],
  "pe" => &[PropertyParser::LengthZero(TailwindProperty::PaddingInlineEnd)],
  "inset" => &[PropertyParser::LengthAuto(TailwindProperty::Inset)],
  "inset-x" => &[PropertyParser::LengthAuto(TailwindProperty::InsetX)],
  "inset-y" => &[PropertyParser::LengthAuto(TailwindProperty::InsetY)],
  "top" => &[PropertyParser::LengthAuto(TailwindProperty::Top)],
  "right" => &[PropertyParser::LengthAuto(TailwindProperty::Right)],
  "bottom" => &[PropertyParser::LengthAuto(TailwindProperty::Bottom)],
  "left" => &[PropertyParser::LengthAuto(TailwindProperty::Left)],
  "rounded" => &[PropertyParser::Rounded(TailwindProperty::Rounded)],
  "rounded-t" => &[PropertyParser::Rounded(TailwindProperty::RoundedTop)],
  "rounded-r" => &[PropertyParser::Rounded(TailwindProperty::RoundedRight)],
  "rounded-b" => &[PropertyParser::Rounded(TailwindProperty::RoundedBottom)],
  "rounded-l" => &[PropertyParser::Rounded(TailwindProperty::RoundedLeft)],
  "rounded-tl" => &[PropertyParser::Rounded(TailwindProperty::RoundedTopLeft)],
  "rounded-tr" => &[PropertyParser::Rounded(TailwindProperty::RoundedTopRight)],
  "rounded-br" => &[PropertyParser::Rounded(TailwindProperty::RoundedBottomRight)],
  "rounded-bl" => &[PropertyParser::Rounded(TailwindProperty::RoundedBottomLeft)],
  "grid-cols" => &[PropertyParser::GridTemplate(TailwindProperty::GridTemplateColumns)],
  "grid-rows" => &[PropertyParser::GridTemplate(TailwindProperty::GridTemplateRows)],
  "auto-cols" => &[PropertyParser::GridAuto(TailwindProperty::GridAutoColumns)],
  "auto-rows" => &[PropertyParser::GridAuto(TailwindProperty::GridAutoRows)],
  "col" => &[PropertyParser::GridLine(TailwindProperty::GridColumn)],
  "row" => &[PropertyParser::GridLine(TailwindProperty::GridRow)],
  "col-span" => &[PropertyParser::GridSpan(TailwindProperty::GridColumnSpan)],
  "row-span" => &[PropertyParser::GridSpan(TailwindProperty::GridRowSpan)],
  "col-start" => &[PropertyParser::GridPlacement(TailwindProperty::GridColumnStart)],
  "col-end" => &[PropertyParser::GridPlacement(TailwindProperty::GridColumnEnd)],
  "row-start" => &[PropertyParser::GridPlacement(TailwindProperty::GridRowStart)],
  "row-end" => &[PropertyParser::GridPlacement(TailwindProperty::GridRowEnd)],
  "tracking" => &[PropertyParser::LetterSpacing(TailwindProperty::LetterSpacing)],
  "blur" => &[PropertyParser::Blur(TailwindProperty::Blur)],
  "brightness" => &[PropertyParser::Percentage(TailwindProperty::Brightness)],
  "contrast" => &[PropertyParser::Percentage(TailwindProperty::Contrast)],
  "grayscale" => &[PropertyParser::Percentage(TailwindProperty::Grayscale)],
  "hue-rotate" => &[PropertyParser::Angle(TailwindProperty::HueRotate)],
  "invert" => &[PropertyParser::Percentage(TailwindProperty::Invert)],
  "saturate" => &[PropertyParser::Percentage(TailwindProperty::Saturate)],
  "sepia" => &[PropertyParser::Percentage(TailwindProperty::Sepia)],
  "filter" => &[PropertyParser::Filter(TailwindProperty::Filter)],
  "backdrop-blur" => &[PropertyParser::Blur(TailwindProperty::BackdropBlur)],
  "backdrop-brightness" => &[PropertyParser::Percentage(TailwindProperty::BackdropBrightness)],
  "backdrop-contrast" => &[PropertyParser::Percentage(TailwindProperty::BackdropContrast)],
  "backdrop-grayscale" => &[PropertyParser::Percentage(TailwindProperty::BackdropGrayscale)],
  "backdrop-hue-rotate" => &[PropertyParser::Angle(TailwindProperty::BackdropHueRotate)],
  "backdrop-invert" => &[PropertyParser::Percentage(TailwindProperty::BackdropInvert)],
  "backdrop-opacity" => &[PropertyParser::Percentage(TailwindProperty::BackdropOpacity)],
  "backdrop-saturate" => &[PropertyParser::Percentage(TailwindProperty::BackdropSaturate)],
  "backdrop-sepia" => &[PropertyParser::Percentage(TailwindProperty::BackdropSepia)],
  "backdrop-filter" => &[PropertyParser::Filter(TailwindProperty::BackdropFilter)],
  "drop-shadow" => &[PropertyParser::DropShadow(TailwindProperty::DropShadow)],
  "text-shadow" => &[
    PropertyParser::StopColor(TailwindProperty::TextShadowColor),
    PropertyParser::TextShadow(TailwindProperty::TextShadow),
  ],
  "mix-blend" => &[PropertyParser::BlendMode(TailwindProperty::MixBlendMode)],
  "bg-blend" => &[PropertyParser::BlendMode(TailwindProperty::BackgroundBlendMode)],
  "align" => &[PropertyParser::VerticalAlign(TailwindProperty::VerticalAlign)],
  "animate" => &[PropertyParser::Animation(TailwindProperty::Animation)],
};

// v4 `theme.css` composites. Alpha: `/ .075` ≈ 19, `/ .1` ≈ 26, `/ .25` ≈ 64.
const fn bs(inset: bool, oy: f32, blur: f32, spread: f32, alpha: u8) -> BoxShadow {
  BoxShadow {
    inset,
    offset_x: Length::Px(0.0),
    offset_y: Length::Px(oy),
    blur_radius: Length::Px(blur),
    spread_radius: Length::Px(spread),
    color: ColorInput::Value(Color([0, 0, 0, alpha])),
  }
}
const fn ts(oy: f32, blur: f32, alpha: u8) -> TextShadow {
  TextShadow {
    offset_x: Length::Px(0.0),
    offset_y: Length::Px(oy),
    blur_radius: Length::Px(blur),
    color: ColorInput::Value(Color([0, 0, 0, alpha])),
  }
}

const SHADOW_SM: [BoxShadow; 2] = [bs(false, 1.0, 3.0, 0.0, 26), bs(false, 1.0, 2.0, -1.0, 26)];
const SHADOW_MD: [BoxShadow; 2] = [bs(false, 4.0, 6.0, -1.0, 26), bs(false, 2.0, 4.0, -2.0, 26)];
const SHADOW_LG: [BoxShadow; 2] = [
  bs(false, 10.0, 15.0, -3.0, 26),
  bs(false, 4.0, 6.0, -4.0, 26),
];
const SHADOW_XL: [BoxShadow; 2] = [
  bs(false, 20.0, 25.0, -5.0, 26),
  bs(false, 8.0, 10.0, -6.0, 26),
];

const TEXT_SHADOW_SM: [TextShadow; 3] = [ts(1.0, 0.0, 19), ts(1.0, 1.0, 19), ts(2.0, 2.0, 19)];
const TEXT_SHADOW_MD: [TextShadow; 3] = [ts(1.0, 1.0, 26), ts(1.0, 2.0, 26), ts(2.0, 4.0, 26)];
const TEXT_SHADOW_LG: [TextShadow; 3] = [ts(1.0, 2.0, 26), ts(3.0, 2.0, 26), ts(4.0, 8.0, 26)];

/// Maps a complete utility token to its fixed property.
pub(crate) static FIXED_PROPERTIES: phf::Map<&str, TailwindProperty> = phf_map! {
  "border" => TailwindProperty::BorderDefault,
  "border-t" => TailwindProperty::BorderTopWidth(LineWidth::Length(Length::Px(1.0))),
  "border-r" => TailwindProperty::BorderRightWidth(LineWidth::Length(Length::Px(1.0))),
  "border-b" => TailwindProperty::BorderBottomWidth(LineWidth::Length(Length::Px(1.0))),
  "border-l" => TailwindProperty::BorderLeftWidth(LineWidth::Length(Length::Px(1.0))),
  "border-x" => TailwindProperty::BorderXWidth(LineWidth::Length(Length::Px(1.0))),
  "border-y" => TailwindProperty::BorderYWidth(LineWidth::Length(Length::Px(1.0))),
  "outline" => TailwindProperty::OutlineDefault,
  "rounded" => TailwindProperty::Rounded(TwRounded(Length::Rem(0.25))),
  "box-border" => TailwindProperty::BoxSizing(BoxSizing::BorderBox),
  "box-content" => TailwindProperty::BoxSizing(BoxSizing::ContentBox),
  "inline" => TailwindProperty::Display(Display::Inline),
  "inline-block" => TailwindProperty::Display(Display::InlineBlock),
  "inline-flex" => TailwindProperty::Display(Display::InlineFlex),
  "bg-radial" => TailwindProperty::BgRadial,
  "bg-conic" => TailwindProperty::BgConicAngle(Angle::zero()),
  "inline-grid" => TailwindProperty::Display(Display::InlineGrid),
  "block" => TailwindProperty::Display(Display::Block),
  "flex" => TailwindProperty::Display(Display::Flex),
  "grid" => TailwindProperty::Display(Display::Grid),
  "hidden" => TailwindProperty::Display(Display::None),
  "list-item" => TailwindProperty::Display(Display::ListItem),
  "bg-repeat" => TailwindProperty::BackgroundRepeat(BackgroundRepeat::repeat()),
  "bg-no-repeat" => TailwindProperty::BackgroundRepeat(BackgroundRepeat::no_repeat()),
  "bg-space" | "bg-repeat-space" => TailwindProperty::BackgroundRepeat(BackgroundRepeat::space()),
  "bg-round" | "bg-repeat-round" => TailwindProperty::BackgroundRepeat(BackgroundRepeat::round()),
  "bg-repeat-x" => TailwindProperty::BackgroundRepeat(BackgroundRepeat(
    BackgroundRepeatStyle::Repeat,
    BackgroundRepeatStyle::NoRepeat,
  )),
  "bg-repeat-y" => TailwindProperty::BackgroundRepeat(BackgroundRepeat(
    BackgroundRepeatStyle::NoRepeat,
    BackgroundRepeatStyle::Repeat,
  )),
  "aspect-auto" => TailwindProperty::Aspect(AspectRatio::Auto),
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
  "static" => TailwindProperty::Position(Position::Static),
  "fixed" => TailwindProperty::Position(Position::Fixed),
  "uppercase" => TailwindProperty::TextTransform(TextTransform::Uppercase),
  "lowercase" => TailwindProperty::TextTransform(TextTransform::Lowercase),
  "capitalize" => TailwindProperty::TextTransform(TextTransform::Capitalize),
  "normal-case" => TailwindProperty::TextTransform(TextTransform::None),
  "underline" => TailwindProperty::TextDecorationLine(TextDecorationLines::UNDERLINE),
  "overline" => TailwindProperty::TextDecorationLine(TextDecorationLines::OVERLINE),
  "line-through" => TailwindProperty::TextDecorationLine(TextDecorationLines::LINE_THROUGH),
  "no-underline" => TailwindProperty::TextDecorationLine(TextDecorationLines::empty()),
  "italic" => TailwindProperty::FontStyle(FontStyle::italic()),
  "not-italic" => TailwindProperty::FontStyle(FontStyle::normal()),
  "w-screen" => TailwindProperty::Width(Length::Vw(100.0)),
  "h-screen" => TailwindProperty::Height(Length::Vh(100.0)),
  "min-w-screen" => TailwindProperty::MinWidth(Length::Vw(100.0)),
  "min-h-screen" => TailwindProperty::MinHeight(Length::Vh(100.0)),
  "max-w-screen" => TailwindProperty::MaxWidth(Length::Vw(100.0)),
  "max-h-screen" => TailwindProperty::MaxHeight(Length::Vh(100.0)),
  "truncate" => TailwindProperty::Truncate,
  "text-ellipsis" => TailwindProperty::TextOverflow(TextOverflow::Ellipsis),
  "text-clip" => TailwindProperty::TextOverflow(TextOverflow::Clip),
  "break-normal" => TailwindProperty::WordBreak(WordBreak::Normal),
  "break-all" => TailwindProperty::WordBreak(WordBreak::BreakAll),
  "break-keep" => TailwindProperty::WordBreak(WordBreak::KeepAll),
  "grid-flow-row" => TailwindProperty::GridAutoFlow(GridAutoFlow::row()),
  "grid-flow-col" => TailwindProperty::GridAutoFlow(GridAutoFlow::column()),
  "grid-flow-row-dense" | "grid-flow-dense" => TailwindProperty::GridAutoFlow(GridAutoFlow::row().dense()),
  "grid-flow-col-dense" => TailwindProperty::GridAutoFlow(GridAutoFlow::column().dense()),
  "col-span-full" => TailwindProperty::GridColumn(GridLine::full()),
  "row-span-full" => TailwindProperty::GridRow(GridLine::full()),
  "col-start-auto" => TailwindProperty::GridColumnStart(GridPlacement::Auto),
  "col-end-auto" => TailwindProperty::GridColumnEnd(GridPlacement::Auto),
  "row-start-auto" => TailwindProperty::GridRowStart(GridPlacement::Auto),
  "row-end-auto" => TailwindProperty::GridRowEnd(GridPlacement::Auto),
  "shadow-2xs" => TailwindProperty::Shadow(bs(false, 1.0, 0.0, 0.0, 13)),
  "shadow-xs" => TailwindProperty::Shadow(bs(false, 1.0, 2.0, 0.0, 13)),
  "shadow-sm" | "shadow" => TailwindProperty::ShadowList(&SHADOW_SM),
  "shadow-md" => TailwindProperty::ShadowList(&SHADOW_MD),
  "shadow-lg" => TailwindProperty::ShadowList(&SHADOW_LG),
  "shadow-xl" => TailwindProperty::ShadowList(&SHADOW_XL),
  "shadow-2xl" => TailwindProperty::Shadow(bs(false, 25.0, 50.0, -12.0, 64)),
  "shadow-none" => TailwindProperty::ShadowList(&[]),
  "grayscale" => TailwindProperty::Grayscale(PercentageNumber(1.0)),
  "invert" => TailwindProperty::Invert(PercentageNumber(1.0)),
  "sepia" => TailwindProperty::Sepia(PercentageNumber(1.0)),
  "backdrop-grayscale" => TailwindProperty::BackdropGrayscale(PercentageNumber(1.0)),
  "backdrop-invert" => TailwindProperty::BackdropInvert(PercentageNumber(1.0)),
  "backdrop-sepia" => TailwindProperty::BackdropSepia(PercentageNumber(1.0)),
  "drop-shadow-xs" => TailwindProperty::DropShadow(ts(1.0, 1.0, 13)),
  "drop-shadow-sm" => TailwindProperty::DropShadow(ts(1.0, 2.0, 38)),
  "drop-shadow" => TailwindProperty::DropShadow(ts(1.0, 2.0, 26)),
  "drop-shadow-md" => TailwindProperty::DropShadow(ts(3.0, 3.0, 31)),
  "drop-shadow-lg" => TailwindProperty::DropShadow(ts(4.0, 4.0, 38)),
  "drop-shadow-xl" => TailwindProperty::DropShadow(ts(9.0, 7.0, 26)),
  "drop-shadow-2xl" => TailwindProperty::DropShadow(ts(25.0, 25.0, 38)),
  "drop-shadow-none" => TailwindProperty::DropShadow(ts(0.0, 0.0, 0)),
  // Inset shadows (--inset-shadow-*)
  "inset-shadow-2xs" => TailwindProperty::Shadow(bs(true, 1.0, 0.0, 0.0, 13)),
  "inset-shadow-xs" => TailwindProperty::Shadow(bs(true, 1.0, 1.0, 0.0, 13)),
  "inset-shadow-sm" => TailwindProperty::Shadow(bs(true, 2.0, 4.0, 0.0, 13)),
  "inset-shadow-none" => TailwindProperty::ShadowList(&[]),
  // Text shadows (--text-shadow-*)
  "text-shadow-2xs" => TailwindProperty::TextShadow(ts(1.0, 0.0, 38)),
  "text-shadow-xs" => TailwindProperty::TextShadow(ts(1.0, 1.0, 51)),
  "text-shadow-sm" => TailwindProperty::TextShadowList(&TEXT_SHADOW_SM),
  "text-shadow-md" => TailwindProperty::TextShadowList(&TEXT_SHADOW_MD),
  "text-shadow-lg" => TailwindProperty::TextShadowList(&TEXT_SHADOW_LG),
  "text-shadow-none" => TailwindProperty::TextShadowList(&[]),
  "isolate" => TailwindProperty::Isolation(Isolation::Isolate),
  "isolation-auto" => TailwindProperty::Isolation(Isolation::Auto),
  "visible" => TailwindProperty::Visibility(Visibility::Visible),
  "invisible" => TailwindProperty::Visibility(Visibility::Hidden),
};
