pub(crate) mod map;
pub(crate) mod parser;

use std::{borrow::Cow, cmp::Ordering, ops::Neg, str::FromStr};

use serde::{Deserializer, de::Error as DeError};

use crate::layout::{
  Viewport,
  style::{
    tw::{
      map::{FIXED_PROPERTIES, PREFIX_PARSERS},
      parser::*,
    },
    *,
  },
};

/// Tailwind `--spacing` variable value.
pub const TW_VAR_SPACING: f32 = 0.25;

/// Represents a collection of tailwind properties.
#[derive(Debug, Clone, PartialEq)]
pub struct TailwindValues {
  inner: Vec<TailwindValue>,
}

impl FromStr for TailwindValues {
  type Err = String;

  fn from_str(s: &str) -> Result<Self, Self::Err> {
    let mut collected = s
      .split_whitespace()
      .filter_map(TailwindValue::parse)
      .collect::<Vec<_>>();

    // sort in reverse order by is important, then has breakpoint, then rest is last.
    collected.sort_unstable_by(|a, b| {
      // Not important comes before important
      if !a.important && b.important {
        return Ordering::Less;
      }

      if a.important && !b.important {
        return Ordering::Greater;
      }

      // No breakpoint comes before breakpoint
      match (&a.breakpoint, &b.breakpoint) {
        (None, Some(_)) => Ordering::Less,
        (Some(_), None) => Ordering::Greater,
        _ => Ordering::Equal,
      }
    });

    Ok(TailwindValues { inner: collected })
  }
}

impl TailwindValues {
  /// Iterate over the tailwind values.
  pub fn iter(&self) -> impl Iterator<Item = &TailwindValue> {
    self.inner.iter()
  }

  pub(crate) fn apply(&self, style: &mut Style, viewport: Viewport) {
    for value in self.iter() {
      value.apply(style, viewport);
    }
  }
}

impl<'de> Deserialize<'de> for TailwindValues {
  fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
  where
    D: Deserializer<'de>,
  {
    let string = String::deserialize(deserializer)?;

    TailwindValues::from_str(&string).map_err(D::Error::custom)
  }
}

/// Represents a tailwind value.
#[derive(Debug, Clone, PartialEq)]
pub struct TailwindValue {
  /// The tailwind property.
  pub property: TailwindProperty,
  /// The breakpoint.
  pub breakpoint: Option<Breakpoint>,
  /// Whether the value is important.
  pub important: bool,
}

impl TailwindValue {
  pub(crate) fn apply(&self, style: &mut Style, viewport: Viewport) {
    if let Some(breakpoint) = self.breakpoint
      && !breakpoint.matches(viewport)
    {
      return;
    }

    self.property.apply(style);
  }

  /// Parse a tailwind value from a token.
  pub fn parse(mut token: &str) -> Option<Self> {
    let mut important = false;
    let mut breakpoint = None;

    // Breakpoint. sm:mt-0
    if let Some((breakpoint_token, rest)) = token.split_once(':') {
      breakpoint = Some(Breakpoint::parse(breakpoint_token)?);
      token = rest;
    }

    // Check for important flag. !mt-0
    if let Some(stripped) = token.strip_prefix('!') {
      important = true;
      token = stripped;
    }

    // Check for important flag. mt-0!
    if let Some(stripped) = token.strip_suffix('!') {
      important = true;
      token = stripped;
    }

    Some(TailwindValue {
      property: TailwindProperty::parse(token)?,
      breakpoint,
      important,
    })
  }
}

/// Represents a breakpoint.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Breakpoint(pub(crate) Length);

impl Breakpoint {
  /// Parse a breakpoint from a token.
  pub fn parse(token: &str) -> Option<Self> {
    match_ignore_ascii_case! {token,
      "sm" => Some(Breakpoint(Length::Rem(40.0))),
      "md" => Some(Breakpoint(Length::Rem(48.0))),
      "lg" => Some(Breakpoint(Length::Rem(64.0))),
      "xl" => Some(Breakpoint(Length::Rem(80.0))),
      "2xl" => Some(Breakpoint(Length::Rem(96.0))),
      _ => None,
    }
  }

  /// Check if the breakpoint matches the viewport width.
  pub fn matches(&self, viewport: Viewport) -> bool {
    let Some(viewport_width) = viewport.width else {
      return false;
    };

    let breakpoint_width = match self.0 {
      Length::Rem(value) => value * viewport.font_size * viewport.device_pixel_ratio,
      Length::Px(value) => value * viewport.device_pixel_ratio,
      Length::Vw(value) => (value / 100.0) * viewport_width as f32,
      _ => 0.0,
    };

    viewport_width >= breakpoint_width as u32
  }
}

/// Represents a tailwind property.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum TailwindProperty {
  /// `background-clip` property.
  BackgroundClip(BackgroundClip),
  /// `box-sizing` property.
  BoxSizing(BoxSizing),
  /// `flex-grow` property.
  FlexGrow(FlexGrow),
  /// `flex-shrink` property.
  FlexShrink(FlexGrow),
  /// `aspect-ratio` property.
  Aspect(AspectRatio),
  /// `align-items` property.
  Items(AlignItems),
  /// `justify-content` property.
  Justify(JustifyContent),
  /// `align-content` property.
  Content(JustifyContent),
  /// `align-self` property.
  JustifySelf(AlignItems),
  /// `justify-items` property.
  JustifyItems(AlignItems),
  /// `flex-direction` property.
  AlignSelf(AlignItems),
  /// `flex-direction` property.
  FlexDirection(FlexDirection),
  /// `flex-wrap` property.
  FlexWrap(FlexWrap),
  /// `flex` property.
  Flex(Flex),
  /// `flex-basis` property.
  FlexBasis(Length),
  /// `overflow` property.
  Overflow(Overflow),
  /// `overflow-x` property.
  OverflowX(Overflow),
  /// `overflow-y` property.
  OverflowY(Overflow),
  /// `position` property.
  Position(Position),
  /// `font-style` property.
  FontStyle(FontStyle),
  /// `font-weight` property.
  FontWeight(FontWeight),
  /// `font-stretch` property.
  FontStretch(FontStretch),
  /// `font-family` property.
  FontFamily(FontFamily),
  /// `line-clamp` property.
  LineClamp(LineClamp),
  /// `text-overflow` property.
  TextOverflow(TextOverflow),
  /// `text-wrap` property.
  TextWrap(TextWrap),
  /// `white-space` property.
  WhiteSpace(WhiteSpace),
  /// `word-break` property.
  WordBreak(WordBreak),
  /// `overflow-wrap` property.
  OverflowWrap(OverflowWrap),
  /// Set `text-overflow: ellipsis`, `white-space: nowrap` and `overflow: hidden`.
  Truncate,
  /// `text-align` property.
  TextAlign(TextAlign),
  /// `text-decoration` property.
  TextDecorationLine(TextDecorationLines),
  /// `text-decoration-color` property.
  TextDecorationColor(ColorInput),
  /// `text-transform` property.
  TextTransform(TextTransform),
  /// `width` and `height` property.
  Size(Length),
  /// `width` property.
  Width(Length),
  /// `height` property.
  Height(Length),
  /// `min-width` property.
  MinWidth(Length),
  /// `min-height` property.
  MinHeight(Length),
  /// `max-width` property.
  MaxWidth(Length),
  /// `max-height` property.
  MaxHeight(Length),
  /// `box-shadow` property.
  Shadow(BoxShadow),
  /// `display` property.
  Display(Display),
  /// `object-position` property.
  ObjectPosition(BackgroundPosition),
  /// `object-fit` property.
  ObjectFit(ObjectFit),
  /// `background-position` property.
  BackgroundPosition(BackgroundPosition),
  /// `background-size` property.
  BackgroundSize(BackgroundSize),
  /// `background-repeat` property.
  BackgroundRepeat(BackgroundRepeat),
  /// `background-image` property.
  BackgroundImage(BackgroundImage),
  /// `gap` property.
  Gap(Length<false>),
  /// `column-gap` property.
  GapX(Length<false>),
  /// `row-gap` property.
  GapY(Length<false>),
  /// `grid-auto-flow` property.
  GridAutoFlow(GridAutoFlow),
  /// `grid-auto-columns` property.
  GridAutoColumns(GridTrackSize),
  /// `grid-auto-rows` property.
  GridAutoRows(GridTrackSize),
  /// `grid-column` property.
  GridColumn(GridLine),
  /// `grid-row` property.
  GridRow(GridLine),
  /// `grid-column: span <number> / span <number>` property.
  GridColumnSpan(GridPlacementSpan),
  /// `grid-row: span <number> / span <number>` property.
  GridRowSpan(GridPlacementSpan),
  /// `grid-column-start` property.
  GridColumnStart(GridPlacement),
  /// `grid-column-end` property.
  GridColumnEnd(GridPlacement),
  /// `grid-row-start` property.
  GridRowStart(GridPlacement),
  /// `grid-row-end` property.
  GridRowEnd(GridPlacement),
  /// `grid-template-columns` property.
  GridTemplateColumns(TwGridTemplate),
  /// `grid-template-rows` property.
  GridTemplateRows(TwGridTemplate),
  /// `letter-spacing` property.
  LetterSpacing(TwLetterSpacing),
  /// Tailwind `border` utility (`border-width: 1px; border-style: solid`).
  BorderDefault,
  /// `border-width` property.
  BorderWidth(TwBorderWidth),
  /// `border-style` property.
  BorderStyle(BorderStyle),
  /// `color` property.
  Color(ColorInput),
  /// `opacity` property.
  Opacity(PercentageNumber),
  /// `background-color` property.
  BackgroundColor(ColorInput<false>),
  /// `border-color` property.
  BorderColor(ColorInput),
  /// `border-top-width` property.
  BorderTopWidth(TwBorderWidth),
  /// `border-right-width` property.
  BorderRightWidth(TwBorderWidth),
  /// `border-bottom-width` property.
  BorderBottomWidth(TwBorderWidth),
  /// `border-left-width` property.
  BorderLeftWidth(TwBorderWidth),
  /// `border-inline-width` property.
  BorderXWidth(TwBorderWidth),
  /// `border-block-width` property.
  BorderYWidth(TwBorderWidth),
  /// Tailwind `outline` utility (`outline-width: 1px; outline-style: solid`).
  OutlineDefault,
  /// `outline-width` property.
  OutlineWidth(TwBorderWidth),
  /// `outline-color` property.
  OutlineColor(ColorInput),
  /// `outline-style` property.
  OutlineStyle(BorderStyle),
  /// `outline-offset` property.
  OutlineOffset(TwBorderWidth),
  /// `border-radius` property.
  Rounded(TwRounded),
  /// `border-top-left-radius` property.
  RoundedTopLeft(TwRounded),
  /// `border-top-right-radius` property.
  RoundedTopRight(TwRounded),
  /// `border-bottom-right-radius` property.
  RoundedBottomRight(TwRounded),
  /// `border-bottom-left-radius` property.
  RoundedBottomLeft(TwRounded),
  /// `border-top-left-radius`, `border-top-right-radius` property.
  RoundedTop(TwRounded),
  /// `border-top-right-radius`, `border-bottom-right-radius` property.
  RoundedRight(TwRounded),
  /// `border-bottom-left-radius`, `border-bottom-right-radius` property.
  RoundedBottom(TwRounded),
  /// `border-top-left-radius`, `border-bottom-left-radius` property.
  RoundedLeft(TwRounded),
  /// `font-size` property.
  FontSize(TwFontSize),
  /// `line-height` property.
  LineHeight(LineHeight),
  /// `translate` property.
  Translate(Length),
  /// `translate-x` property.
  TranslateX(Length),
  /// `translate-y` property.
  TranslateY(Length),
  /// `rotate` property.
  Rotate(Angle),
  /// `scale` property.
  Scale(PercentageNumber),
  /// `scale-x` property.
  ScaleX(PercentageNumber),
  /// `scale-y` property.
  ScaleY(PercentageNumber),
  /// `transform-origin` property.
  TransformOrigin(BackgroundPosition),
  /// `margin` property.
  Margin(Length<false>),
  /// `margin-inline` property.
  MarginX(Length<false>),
  /// `margin-block` property.
  MarginY(Length<false>),
  /// `margin-top` property.
  MarginTop(Length<false>),
  /// `margin-right` property.
  MarginRight(Length<false>),
  /// `margin-bottom` property.
  MarginBottom(Length<false>),
  /// `margin-left` property.
  MarginLeft(Length<false>),
  /// `padding` property.
  Padding(Length<false>),
  /// `padding-inline` property.
  PaddingX(Length<false>),
  /// `padding-block` property.
  PaddingY(Length<false>),
  /// `padding-top` property.
  PaddingTop(Length<false>),
  /// `padding-right` property.
  PaddingRight(Length<false>),
  /// `padding-bottom` property.
  PaddingBottom(Length<false>),
  /// `padding-left` property.
  PaddingLeft(Length<false>),
  /// `inset` property.
  Inset(Length),
  /// `inset-inline` property.
  InsetX(Length),
  /// `inset-block` property.
  InsetY(Length),
  /// `top` property.
  Top(Length),
  /// `right` property.
  Right(Length),
  /// `bottom` property.
  Bottom(Length),
  /// `left` property.
  Left(Length),
  /// `filter: blur()` property.
  Blur(TwBlur),
  /// `filter: brightness()` property.
  Brightness(PercentageNumber),
  /// `filter: contrast()` property.
  Contrast(PercentageNumber),
  /// `filter: drop-shadow()` property.
  DropShadow(TextShadow),
  /// `filter: grayscale()` property.
  Grayscale(PercentageNumber),
  /// `filter: hue-rotate()` property.
  HueRotate(Angle),
  /// `filter: invert()` property.
  Invert(PercentageNumber),
  /// `filter: saturate()` property.
  Saturate(PercentageNumber),
  /// `filter: sepia()` property.
  Sepia(PercentageNumber),
  /// `filter` property.
  Filter(Filters),
  /// `backdrop-filter: blur()` property.
  BackdropBlur(TwBlur),
  /// `backdrop-filter: brightness()` property.
  BackdropBrightness(PercentageNumber),
  /// `backdrop-filter: contrast()` property.
  BackdropContrast(PercentageNumber),
  /// `backdrop-filter: grayscale()` property.
  BackdropGrayscale(PercentageNumber),
  /// `backdrop-filter: hue-rotate()` property.
  BackdropHueRotate(Angle),
  /// `backdrop-filter: invert()` property.
  BackdropInvert(PercentageNumber),
  /// `backdrop-filter: opacity()` property.
  BackdropOpacity(PercentageNumber),
  /// `backdrop-filter: saturate()` property.
  BackdropSaturate(PercentageNumber),
  /// `backdrop-filter: sepia()` property.
  BackdropSepia(PercentageNumber),
  /// `backdrop-filter` property.
  BackdropFilter(Filters),
  /// `text-shadow` property.
  TextShadow(TextShadow),
  /// `isolation` property.
  Isolation(Isolation),
  /// `mix-blend-mode` property.
  MixBlendMode(BlendMode),
  /// `background-blend-mode` property.
  BackgroundBlendMode(BlendMode),
  /// `visibility` property.
  Visibility(Visibility),
  /// `vertical-align` property.
  VerticalAlign(VerticalAlign),
}

fn extract_arbitrary_value(suffix: &str) -> Option<Cow<'_, str>> {
  if suffix.starts_with('[') && suffix.ends_with(']') {
    let value = &suffix[1..suffix.len() - 1];
    if value.contains('_') {
      Some(Cow::Owned(value.replace('_', " ")))
    } else {
      Some(Cow::Borrowed(value))
    }
  } else {
    None
  }
}

/// A trait for parsing tailwind properties.
pub trait TailwindPropertyParser: Sized + for<'i> FromCss<'i> {
  /// Parse a tailwind property from a token.
  fn parse_tw(token: &str) -> Option<Self>;

  /// Parse a tailwind property from a token, with support for arbitrary values.
  fn parse_tw_with_arbitrary(token: &str) -> Option<Self> {
    if let Some(value) = extract_arbitrary_value(token) {
      return Self::from_str(&value).ok();
    }

    Self::parse_tw(token)
  }
}

impl Neg for TailwindProperty {
  type Output = Self;

  fn neg(self) -> Self::Output {
    match self {
      TailwindProperty::Margin(length) => TailwindProperty::Margin(-length),
      TailwindProperty::MarginX(length) => TailwindProperty::MarginX(-length),
      TailwindProperty::MarginY(length) => TailwindProperty::MarginY(-length),
      TailwindProperty::MarginTop(length) => TailwindProperty::MarginTop(-length),
      TailwindProperty::MarginRight(length) => TailwindProperty::MarginRight(-length),
      TailwindProperty::MarginBottom(length) => TailwindProperty::MarginBottom(-length),
      TailwindProperty::MarginLeft(length) => TailwindProperty::MarginLeft(-length),
      TailwindProperty::Padding(length) => TailwindProperty::Padding(-length),
      TailwindProperty::PaddingX(length) => TailwindProperty::PaddingX(-length),
      TailwindProperty::PaddingY(length) => TailwindProperty::PaddingY(-length),
      TailwindProperty::PaddingTop(length) => TailwindProperty::PaddingTop(-length),
      TailwindProperty::PaddingRight(length) => TailwindProperty::PaddingRight(-length),
      TailwindProperty::PaddingBottom(length) => TailwindProperty::PaddingBottom(-length),
      TailwindProperty::PaddingLeft(length) => TailwindProperty::PaddingLeft(-length),
      TailwindProperty::Inset(length) => TailwindProperty::Inset(-length),
      TailwindProperty::InsetX(length) => TailwindProperty::InsetX(-length),
      TailwindProperty::InsetY(length) => TailwindProperty::InsetY(-length),
      TailwindProperty::Top(length) => TailwindProperty::Top(-length),
      TailwindProperty::Right(length) => TailwindProperty::Right(-length),
      TailwindProperty::Bottom(length) => TailwindProperty::Bottom(-length),
      TailwindProperty::Left(length) => TailwindProperty::Left(-length),
      TailwindProperty::Translate(length) => TailwindProperty::Translate(-length),
      TailwindProperty::TranslateX(length) => TailwindProperty::TranslateX(-length),
      TailwindProperty::TranslateY(length) => TailwindProperty::TranslateY(-length),
      TailwindProperty::Scale(percentage_number) => TailwindProperty::Scale(-percentage_number),
      TailwindProperty::ScaleX(percentage_number) => TailwindProperty::ScaleX(-percentage_number),
      TailwindProperty::ScaleY(percentage_number) => TailwindProperty::ScaleY(-percentage_number),
      TailwindProperty::Rotate(angle) => TailwindProperty::Rotate(-angle),
      TailwindProperty::LetterSpacing(length) => TailwindProperty::LetterSpacing(-length),
      TailwindProperty::HueRotate(angle) => TailwindProperty::HueRotate(-angle),
      TailwindProperty::BackdropHueRotate(angle) => TailwindProperty::BackdropHueRotate(-angle),
      _ => self,
    }
  }
}

/// Macro to append a filter to the existing filter list in style.
/// If no filters exist, creates a new filter Vec with the single filter.
macro_rules! append_filter {
  ($style:expr, $field:ident, $filter:expr) => {{
    if let crate::layout::style::CssValue::Value(existing_filters) = &mut $style.$field {
      existing_filters.push($filter);
    } else {
      $style.$field = vec![$filter].into();
    }
  }};
}

impl TailwindProperty {
  /// Parse a single tailwind property from a token.
  pub fn parse(token: &str) -> Option<TailwindProperty> {
    // Check fixed properties first
    if let Some(property) = FIXED_PROPERTIES.get(token) {
      return Some(property.clone());
    }

    // Handle negative values like "-top-4"
    if let Some(stripped) = token.strip_prefix('-') {
      if let Some(property) = Self::parse_prefix_suffix(stripped) {
        return Some(-property);
      }

      return None;
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
        if let Some(property) = parser.parse(suffix) {
          return Some(property);
        }
      }
    }

    None
  }

  pub(crate) fn apply(&self, style: &mut Style) {
    match *self {
      TailwindProperty::BackgroundClip(background_clip) => {
        style.background_clip = background_clip.into();
      }
      TailwindProperty::Gap(gap) => {
        style.gap = SpacePair::from_single(gap).into();
      }
      TailwindProperty::GapX(gap_x) => {
        style.column_gap = Some(gap_x).into();
      }
      TailwindProperty::GapY(gap_y) => {
        style.row_gap = Some(gap_y).into();
      }
      TailwindProperty::BoxSizing(box_sizing) => {
        style.box_sizing = box_sizing.into();
      }
      TailwindProperty::FlexGrow(flex_grow) => {
        style.flex_grow = Some(flex_grow).into();
      }
      TailwindProperty::FlexShrink(flex_shrink) => {
        style.flex_shrink = Some(flex_shrink).into();
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
        style.flex = Some(flex).into();
      }
      TailwindProperty::FlexBasis(flex_basis) => {
        style.flex_basis = Some(flex_basis).into();
      }
      TailwindProperty::Overflow(overflow) => {
        style.overflow = SpacePair::from_single(overflow).into();
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
      TailwindProperty::FontStretch(font_stretch) => {
        style.font_stretch = font_stretch.into();
      }
      TailwindProperty::FontFamily(ref font_family) => {
        style.font_family = Some(font_family.clone()).into();
      }
      TailwindProperty::LineClamp(ref line_clamp) => {
        style.line_clamp = Some(line_clamp.clone()).into();
      }
      TailwindProperty::TextAlign(text_align) => {
        style.text_align = text_align.into();
      }
      TailwindProperty::TextDecorationLine(text_decoration) => {
        style.text_decoration_line = text_decoration.into();
      }
      TailwindProperty::TextDecorationColor(color_input) => {
        style.text_decoration_color = Some(color_input).into();
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
        style.box_shadow = [box_shadow].into();
      }
      TailwindProperty::Display(display) => {
        style.display = display.into();
      }
      TailwindProperty::OverflowX(overflow) => {
        style.overflow_x = Some(overflow).into();
      }
      TailwindProperty::OverflowY(overflow) => {
        style.overflow_y = Some(overflow).into();
      }
      TailwindProperty::ObjectPosition(background_position) => {
        style.object_position = background_position.into();
      }
      TailwindProperty::ObjectFit(object_fit) => {
        style.object_fit = object_fit.into();
      }
      TailwindProperty::BackgroundPosition(background_position) => {
        style.background_position = [background_position].into();
      }
      TailwindProperty::BackgroundSize(background_size) => {
        style.background_size = [background_size].into();
      }
      TailwindProperty::BackgroundRepeat(background_repeat) => {
        style.background_repeat = [background_repeat].into();
      }
      TailwindProperty::BackgroundImage(ref background_image) => {
        style.background_image = [background_image.clone()].into();
      }
      TailwindProperty::BorderDefault => {
        style.border_width = Some(Sides([Length::Px(1.0); 4])).into();
        style.border_style = Some(BorderStyle::Solid).into();
      }
      TailwindProperty::BorderWidth(tw_border_width) => {
        style.border_width = Some(Sides([tw_border_width.0; 4])).into();
      }
      TailwindProperty::BorderStyle(border_style) => {
        style.border_style = Some(border_style).into();
      }
      TailwindProperty::JustifySelf(align_items) => {
        style.justify_self = align_items.into();
      }
      TailwindProperty::JustifyItems(align_items) => {
        style.justify_items = align_items.into();
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
        style.border_color = Some(color_input).into();
      }
      TailwindProperty::BorderTopWidth(tw_border_width) => {
        style.border_top_width = Some(tw_border_width.0).into();
      }
      TailwindProperty::BorderRightWidth(tw_border_width) => {
        style.border_right_width = Some(tw_border_width.0).into();
      }
      TailwindProperty::BorderBottomWidth(tw_border_width) => {
        style.border_bottom_width = Some(tw_border_width.0).into();
      }
      TailwindProperty::BorderLeftWidth(tw_border_width) => {
        style.border_left_width = Some(tw_border_width.0).into();
      }
      TailwindProperty::BorderXWidth(tw_border_width) => {
        style.border_left_width = Some(tw_border_width.0).into();
        style.border_right_width = Some(tw_border_width.0).into();
      }
      TailwindProperty::BorderYWidth(tw_border_width) => {
        style.border_top_width = Some(tw_border_width.0).into();
        style.border_bottom_width = Some(tw_border_width.0).into();
      }
      TailwindProperty::OutlineDefault => {
        style.outline_width = Some(Length::Px(1.0)).into();
        style.outline_style = Some(BorderStyle::Solid).into();
      }
      TailwindProperty::OutlineWidth(tw_border_width) => {
        style.outline_width = Some(tw_border_width.0).into();
      }
      TailwindProperty::OutlineColor(color_input) => {
        style.outline_color = Some(color_input).into();
      }
      TailwindProperty::OutlineStyle(outline_style) => {
        style.outline_style = Some(outline_style).into();
      }
      TailwindProperty::OutlineOffset(outline_offset) => {
        style.outline_offset = Some(outline_offset.0).into();
      }
      TailwindProperty::Rounded(rounded) => {
        style.border_radius = BorderRadius(Sides([SpacePair::from_single(rounded.0); 4])).into();
      }
      TailwindProperty::VerticalAlign(vertical_align) => {
        style.vertical_align = vertical_align.into();
      }
      TailwindProperty::RoundedTopLeft(rounded) => {
        style.border_top_left_radius = Some(SpacePair::from_single(rounded.0)).into();
      }
      TailwindProperty::RoundedTopRight(rounded) => {
        style.border_top_right_radius = Some(SpacePair::from_single(rounded.0)).into();
      }
      TailwindProperty::RoundedBottomRight(rounded) => {
        style.border_bottom_right_radius = Some(SpacePair::from_single(rounded.0)).into();
      }
      TailwindProperty::RoundedBottomLeft(rounded) => {
        style.border_bottom_left_radius = Some(SpacePair::from_single(rounded.0)).into();
      }
      TailwindProperty::RoundedTop(rounded) => {
        style.border_top_left_radius = Some(SpacePair::from_single(rounded.0)).into();
        style.border_top_right_radius = Some(SpacePair::from_single(rounded.0)).into();
      }
      TailwindProperty::RoundedRight(rounded) => {
        style.border_top_right_radius = Some(SpacePair::from_single(rounded.0)).into();
        style.border_bottom_right_radius = Some(SpacePair::from_single(rounded.0)).into();
      }
      TailwindProperty::RoundedBottom(rounded) => {
        style.border_bottom_left_radius = Some(SpacePair::from_single(rounded.0)).into();
        style.border_bottom_right_radius = Some(SpacePair::from_single(rounded.0)).into();
      }
      TailwindProperty::RoundedLeft(rounded) => {
        style.border_top_left_radius = Some(SpacePair::from_single(rounded.0)).into();
        style.border_bottom_left_radius = Some(SpacePair::from_single(rounded.0)).into();
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
        style.overflow = SpacePair::from_single(Overflow::Hidden).into();
      }
      TailwindProperty::TextWrap(text_wrap) => {
        style.text_wrap = text_wrap.into();
      }
      TailwindProperty::WhiteSpace(white_space) => {
        style.white_space = white_space.into();
      }
      TailwindProperty::WordBreak(word_break) => {
        style.word_break = word_break.into();
      }
      TailwindProperty::Isolation(isolation) => {
        style.isolation = isolation.into();
      }
      TailwindProperty::MixBlendMode(blend_mode) => {
        style.mix_blend_mode = blend_mode.into();
      }
      TailwindProperty::BackgroundBlendMode(blend_mode) => {
        style.background_blend_mode = [blend_mode].into();
      }
      TailwindProperty::OverflowWrap(overflow_wrap) => {
        style.overflow_wrap = overflow_wrap.into();
      }
      TailwindProperty::FontSize(font_size) => {
        style.font_size = Some(font_size.font_size).into();

        if let Some(line_height) = font_size.line_height {
          style.line_height = line_height.into();
        }
      }
      TailwindProperty::LineHeight(line_height) => {
        style.line_height = line_height.into();
      }
      TailwindProperty::Translate(length) => {
        style.translate = Some(SpacePair::from_single(length)).into();
      }
      TailwindProperty::TranslateX(length) => {
        style.translate_x = Some(length).into();
      }
      TailwindProperty::TranslateY(length) => {
        style.translate_y = Some(length).into();
      }
      TailwindProperty::Rotate(angle) => {
        style.rotate = Some(angle).into();
      }
      TailwindProperty::Scale(percentage_number) => {
        style.scale = Some(SpacePair::from_single(percentage_number)).into();
      }
      TailwindProperty::ScaleX(percentage_number) => {
        style.scale_x = Some(percentage_number).into();
      }
      TailwindProperty::ScaleY(percentage_number) => {
        style.scale_y = Some(percentage_number).into();
      }
      TailwindProperty::TransformOrigin(background_position) => {
        style.transform_origin = Some(background_position).into();
      }
      TailwindProperty::Margin(length) => {
        style.margin = Sides([length; 4]).into();
      }
      TailwindProperty::MarginX(length) => {
        style.margin_inline = Some(SpacePair::from_single(length)).into();
      }
      TailwindProperty::MarginY(length) => {
        style.margin_block = Some(SpacePair::from_single(length)).into();
      }
      TailwindProperty::MarginTop(length) => {
        style.margin_top = Some(length).into();
      }
      TailwindProperty::MarginRight(length) => {
        style.margin_right = Some(length).into();
      }
      TailwindProperty::MarginBottom(length) => {
        style.margin_bottom = Some(length).into();
      }
      TailwindProperty::MarginLeft(length) => {
        style.margin_left = Some(length).into();
      }
      TailwindProperty::Padding(length) => {
        style.padding = Sides([length; 4]).into();
      }
      TailwindProperty::PaddingX(length) => {
        style.padding_inline = Some(SpacePair::from_single(length)).into();
      }
      TailwindProperty::PaddingY(length) => {
        style.padding_block = Some(SpacePair::from_single(length)).into();
      }
      TailwindProperty::PaddingTop(length) => {
        style.padding_top = Some(length).into();
      }
      TailwindProperty::PaddingRight(length) => {
        style.padding_right = Some(length).into();
      }
      TailwindProperty::PaddingBottom(length) => {
        style.padding_bottom = Some(length).into();
      }
      TailwindProperty::PaddingLeft(length) => {
        style.padding_left = Some(length).into();
      }
      TailwindProperty::Inset(length) => {
        style.inset = Sides([length; 4]).into();
      }
      TailwindProperty::InsetX(length) => {
        style.inset_inline = Some(SpacePair::from_single(length)).into();
      }
      TailwindProperty::InsetY(length) => {
        style.inset_block = Some(SpacePair::from_single(length)).into();
      }
      TailwindProperty::Top(length) => {
        style.top = Some(length).into();
      }
      TailwindProperty::Right(length) => {
        style.right = Some(length).into();
      }
      TailwindProperty::Bottom(length) => {
        style.bottom = Some(length).into();
      }
      TailwindProperty::Left(length) => {
        style.left = Some(length).into();
      }
      TailwindProperty::GridAutoColumns(grid_auto_size) => {
        style.grid_auto_columns = Some([grid_auto_size].into()).into();
      }
      TailwindProperty::GridAutoRows(grid_auto_size) => {
        style.grid_auto_rows = Some([grid_auto_size].into()).into();
      }
      TailwindProperty::GridColumn(ref tw_grid_span) => {
        style.grid_column = Some(tw_grid_span.clone()).into();
      }
      TailwindProperty::GridRow(ref tw_grid_span) => {
        style.grid_row = Some(tw_grid_span.clone()).into();
      }
      TailwindProperty::GridColumnStart(ref tw_grid_placement) => {
        if let CssValue::Value(Some(ref mut existing_grid_column)) = style.grid_column {
          existing_grid_column.start = tw_grid_placement.clone();
        } else {
          style.grid_column = Some(GridLine::start(tw_grid_placement.clone())).into();
        }
      }
      TailwindProperty::GridColumnEnd(ref tw_grid_placement) => {
        if let CssValue::Value(Some(ref mut existing_grid_column)) = style.grid_column {
          existing_grid_column.end = tw_grid_placement.clone();
        } else {
          style.grid_column = Some(GridLine::end(tw_grid_placement.clone())).into();
        }
      }
      TailwindProperty::GridRowStart(ref tw_grid_placement) => {
        if let CssValue::Value(Some(ref mut existing_grid_row)) = style.grid_row {
          existing_grid_row.start = tw_grid_placement.clone();
        } else {
          style.grid_row = Some(GridLine::start(tw_grid_placement.clone())).into();
        }
      }
      TailwindProperty::GridRowEnd(ref tw_grid_placement) => {
        if let CssValue::Value(Some(ref mut existing_grid_row)) = style.grid_row {
          existing_grid_row.end = tw_grid_placement.clone();
        } else {
          style.grid_row = Some(GridLine::end(tw_grid_placement.clone())).into();
        }
      }
      TailwindProperty::GridTemplateColumns(ref tw_grid_template) => {
        style.grid_template_columns = Some(tw_grid_template.0.clone()).into();
      }
      TailwindProperty::GridTemplateRows(ref tw_grid_template) => {
        style.grid_template_rows = Some(tw_grid_template.0.clone()).into();
      }
      TailwindProperty::LetterSpacing(tw_letter_spacing) => {
        style.letter_spacing = Some(tw_letter_spacing.0).into();
      }
      TailwindProperty::GridAutoFlow(grid_auto_flow) => {
        style.grid_auto_flow = Some(grid_auto_flow).into();
      }
      TailwindProperty::GridColumnSpan(grid_placement_span) => {
        style.grid_column = Some(GridLine::span(grid_placement_span)).into();
      }
      TailwindProperty::GridRowSpan(grid_placement_span) => {
        style.grid_row = Some(GridLine::span(grid_placement_span)).into();
      }
      TailwindProperty::Blur(tw_blur) => {
        append_filter!(style, filter, Filter::Blur(tw_blur.0));
      }
      TailwindProperty::Brightness(percentage_number) => {
        append_filter!(style, filter, Filter::Brightness(percentage_number));
      }
      TailwindProperty::Contrast(percentage_number) => {
        append_filter!(style, filter, Filter::Contrast(percentage_number));
      }
      TailwindProperty::DropShadow(text_shadow) => {
        append_filter!(style, filter, Filter::DropShadow(text_shadow));
      }
      TailwindProperty::Grayscale(percentage_number) => {
        append_filter!(style, filter, Filter::Grayscale(percentage_number));
      }
      TailwindProperty::HueRotate(angle) => {
        append_filter!(style, filter, Filter::HueRotate(angle));
      }
      TailwindProperty::Invert(percentage_number) => {
        append_filter!(style, filter, Filter::Invert(percentage_number));
      }
      TailwindProperty::Saturate(percentage_number) => {
        append_filter!(style, filter, Filter::Saturate(percentage_number));
      }
      TailwindProperty::Sepia(percentage_number) => {
        append_filter!(style, filter, Filter::Sepia(percentage_number));
      }
      TailwindProperty::Filter(ref filters) => {
        for f in filters {
          append_filter!(style, filter, *f);
        }
      }
      TailwindProperty::BackdropBlur(tw_blur) => {
        append_filter!(style, backdrop_filter, Filter::Blur(tw_blur.0));
      }
      TailwindProperty::BackdropBrightness(percentage_number) => {
        append_filter!(
          style,
          backdrop_filter,
          Filter::Brightness(percentage_number)
        );
      }
      TailwindProperty::BackdropContrast(percentage_number) => {
        append_filter!(style, backdrop_filter, Filter::Contrast(percentage_number));
      }
      TailwindProperty::BackdropGrayscale(percentage_number) => {
        append_filter!(style, backdrop_filter, Filter::Grayscale(percentage_number));
      }
      TailwindProperty::BackdropHueRotate(angle) => {
        append_filter!(style, backdrop_filter, Filter::HueRotate(angle));
      }
      TailwindProperty::BackdropInvert(percentage_number) => {
        append_filter!(style, backdrop_filter, Filter::Invert(percentage_number));
      }
      TailwindProperty::BackdropOpacity(percentage_number) => {
        append_filter!(style, backdrop_filter, Filter::Opacity(percentage_number));
      }
      TailwindProperty::BackdropSaturate(percentage_number) => {
        append_filter!(style, backdrop_filter, Filter::Saturate(percentage_number));
      }
      TailwindProperty::BackdropSepia(percentage_number) => {
        append_filter!(style, backdrop_filter, Filter::Sepia(percentage_number));
      }
      TailwindProperty::BackdropFilter(ref filters) => {
        for f in filters {
          append_filter!(style, backdrop_filter, *f);
        }
      }
      TailwindProperty::TextShadow(text_shadow) => {
        style.text_shadow = Some([text_shadow].into()).into();
      }
      TailwindProperty::Visibility(visibility) => {
        style.visibility = visibility.into();
      }
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_box_sizing() {
    assert_eq!(
      TailwindProperty::parse("box-border"),
      Some(TailwindProperty::BoxSizing(BoxSizing::BorderBox))
    );
  }

  #[test]
  fn test_parse_width() {
    assert_eq!(
      TailwindProperty::parse("w-64"),
      Some(TailwindProperty::Width(Length::Rem(64.0 * TW_VAR_SPACING)))
    );
    assert_eq!(
      TailwindProperty::parse("h-32"),
      Some(TailwindProperty::Height(Length::Rem(32.0 * TW_VAR_SPACING)))
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

  #[test]
  fn test_parse_decoration_color() {
    assert_eq!(
      TailwindProperty::parse("decoration-red-500"),
      Some(TailwindProperty::TextDecorationColor(ColorInput::Value(
        Color([239, 68, 68, 255])
      )))
    );
  }

  #[test]
  fn test_parse_text_decoration_lines() {
    assert_eq!(
      TailwindProperty::parse("underline"),
      Some(TailwindProperty::TextDecorationLine(
        TextDecorationLines::UNDERLINE
      ))
    );
    assert_eq!(
      TailwindProperty::parse("no-underline"),
      Some(TailwindProperty::TextDecorationLine(
        TextDecorationLines::empty()
      ))
    );
  }

  #[test]
  fn test_parse_arbitrary_color() {
    assert_eq!(
      TailwindProperty::parse("text-[rgb(0, 191, 255)]"),
      Some(TailwindProperty::Color(ColorInput::Value(Color([
        0, 191, 255, 255
      ]))))
    );
  }

  #[test]
  fn test_parse_arbitrary_flex_with_spaces() {
    assert_eq!(
      TailwindProperty::parse("flex-[3_1_auto]"),
      Some(TailwindProperty::Flex(Flex {
        grow: 3.0,
        shrink: 1.0,
        basis: Length::Auto,
      }))
    );
  }

  #[test]
  fn test_parse_negative_margin() {
    assert_eq!(
      TailwindProperty::parse("-ml-4"),
      Some(TailwindProperty::MarginLeft(Length::Rem(
        -4.0 * TW_VAR_SPACING
      )))
    );
  }

  #[test]
  fn test_parse_border_radius() {
    assert_eq!(
      TailwindProperty::parse("rounded-xs"),
      Some(TailwindProperty::Rounded(TwRounded(Length::Rem(0.125))))
    );
    assert_eq!(
      TailwindProperty::parse("rounded-full"),
      Some(TailwindProperty::Rounded(TwRounded(Length::Px(9999.0))))
    );
  }

  #[test]
  fn test_parse_font_size_with_arbitrary_line_height() {
    assert_eq!(
      TailwindProperty::parse("text-base/[12.34]"),
      Some(TailwindProperty::FontSize(TwFontSize {
        font_size: Length::Rem(1.0),
        line_height: Some(LineHeight::Unitless(12.34)),
      }))
    );
  }

  #[test]
  fn test_parse_border_width() {
    assert_eq!(
      TailwindProperty::parse("border"),
      Some(TailwindProperty::BorderDefault)
    );
    assert_eq!(
      TailwindProperty::parse("border-t-2"),
      Some(TailwindProperty::BorderTopWidth(TwBorderWidth(Length::Px(
        2.0
      ))))
    );
    assert_eq!(
      TailwindProperty::parse("border-x-4"),
      Some(TailwindProperty::BorderXWidth(TwBorderWidth(Length::Px(
        4.0
      ))))
    );
    assert_eq!(
      TailwindProperty::parse("border-solid"),
      Some(TailwindProperty::BorderStyle(BorderStyle::Solid))
    );
    assert_eq!(
      TailwindProperty::parse("border-none"),
      Some(TailwindProperty::BorderStyle(BorderStyle::None))
    );
  }

  #[test]
  fn test_parse_outline() {
    assert_eq!(
      TailwindProperty::parse("outline"),
      Some(TailwindProperty::OutlineDefault)
    );
    assert_eq!(
      TailwindProperty::parse("outline-2"),
      Some(TailwindProperty::OutlineWidth(TwBorderWidth(Length::Px(
        2.0
      ))))
    );
    assert_eq!(
      TailwindProperty::parse("outline-red-500"),
      Some(TailwindProperty::OutlineColor(ColorInput::Value(Color([
        239, 68, 68, 255
      ]))))
    );
    assert_eq!(
      TailwindProperty::parse("outline-solid"),
      Some(TailwindProperty::OutlineStyle(BorderStyle::Solid))
    );
    assert_eq!(
      TailwindProperty::parse("outline-offset-4"),
      Some(TailwindProperty::OutlineOffset(TwBorderWidth(Length::Px(
        4.0
      ))))
    );
    assert_eq!(
      TailwindProperty::parse("outline-none"),
      Some(TailwindProperty::OutlineStyle(BorderStyle::None))
    );
  }

  #[test]
  fn test_parse_col_end() {
    assert_eq!(
      TailwindProperty::parse("col-end-1"),
      Some(TailwindProperty::GridColumnEnd(GridPlacement::Line(1)))
    );
  }

  #[test]
  fn test_parse_overflow_clip() {
    assert_eq!(
      TailwindProperty::parse("overflow-clip"),
      Some(TailwindProperty::Overflow(Overflow::Clip))
    );
    assert_eq!(
      TailwindProperty::parse("overflow-x-clip"),
      Some(TailwindProperty::OverflowX(Overflow::Clip))
    );
    assert_eq!(
      TailwindProperty::parse("overflow-y-clip"),
      Some(TailwindProperty::OverflowY(Overflow::Clip))
    );
  }

  #[test]
  fn test_comprehensive_mappings() {
    // Test various prefix mappings to ensure they're working
    let should_parse = vec![
      // Layout
      "flex",
      "grid",
      "hidden",
      "block",
      "inline",
      // Sizing
      "w-4",
      "h-8",
      "size-12",
      "min-w-0",
      "max-h-96",
      // Spacing
      "m-2",
      "mx-4",
      "my-auto",
      "mt-8",
      "mr-6",
      "mb-4",
      "ml-2",
      "p-3",
      "px-5",
      "py-2",
      "pt-1",
      "pr-4",
      "pb-3",
      "pl-2",
      // Colors
      "text-red-500",
      "bg-blue-200",
      "border-gray-300",
      // Typography
      "text-sm",
      "font-bold",
      "font-stretch-condensed",
      "font-stretch-ultra-expanded",
      "font-stretch-75%",
      "uppercase",
      "tracking-wide",
      // Flexbox
      "justify-center",
      "items-end",
      "self-start",
      "flex-grow",
      "shrink",
      // Borders
      "border",
      "border-t-2",
      "border-solid",
      "border-none",
      "outline",
      "outline-2",
      "outline-red-500",
      "outline-solid",
      "outline-offset-2",
      "rounded-lg",
      // Transforms
      "rotate-45",
      "scale-75",
      "translate-x-4",
      // Grid
      "grid-cols-3",
      "col-span-2",
      // Backdrop Filters
      "backdrop-blur-md",
      "backdrop-brightness-50",
      "backdrop-contrast-125",
      "backdrop-grayscale",
      "backdrop-hue-rotate-90",
      "backdrop-invert",
      "backdrop-opacity-50",
      "backdrop-saturate-200",
      "backdrop-sepia",
      "backdrop-filter-[blur(4px)_brightness(0.5)]",
    ];

    let should_not_parse = vec!["nonexistent-class", "invalid-prefix-1", "random-string"];

    for class in should_parse {
      assert!(
        TailwindProperty::parse(class).is_some(),
        "Expected '{}' to parse successfully",
        class
      );
    }

    for class in should_not_parse {
      assert!(
        TailwindProperty::parse(class).is_none(),
        "Expected '{}' to fail parsing",
        class
      );
    }
  }

  #[test]
  fn test_breakpoint_matches() {
    let viewport = (1000, 1000).into();

    assert!(Breakpoint::parse("sm").is_some_and(|bp| bp.matches(viewport)));
  }

  #[test]
  fn test_breakpoint_does_not_match() {
    let viewport = (1000, 1000).into();

    // 80 * 16 = 1280 > 1000
    assert!(Breakpoint::parse("xl").is_some_and(|bp| !bp.matches(viewport)));
  }

  #[test]
  fn test_value_parsing() {
    assert_eq!(
      TailwindValue::parse("md:!mt-4"),
      Some(TailwindValue {
        property: TailwindProperty::MarginTop(Length::Rem(1.0)),
        breakpoint: Some(Breakpoint(Length::Rem(48.0))),
        important: true,
      })
    );
  }

  #[test]
  fn test_values_sorting() {
    assert_eq!(
      TailwindValues::from_str("md:!mt-4 sm:mt-8 !mt-12 mt-16"),
      Ok(TailwindValues {
        inner: vec![
          // mt-16
          TailwindValue {
            property: TailwindProperty::MarginTop(Length::Rem(4.0)),
            breakpoint: None,
            important: false,
          },
          // sm:mt-8
          TailwindValue {
            property: TailwindProperty::MarginTop(Length::Rem(2.0)),
            breakpoint: Some(Breakpoint(Length::Rem(40.0))),
            important: false,
          },
          // !mt-12
          TailwindValue {
            property: TailwindProperty::MarginTop(Length::Rem(3.0)),
            breakpoint: None,
            important: true,
          },
          // md:!mt-4
          TailwindValue {
            property: TailwindProperty::MarginTop(Length::Rem(1.0)),
            breakpoint: Some(Breakpoint(Length::Rem(48.0))),
            important: true,
          },
        ]
      })
    )
  }

  #[test]
  fn test_filters_append() {
    use crate::layout::style::{CssValue, Style, properties::Filter};

    let mut style = Style::default();

    // Apply blur
    if let Some(blur_prop) = TailwindProperty::parse("blur-sm") {
      blur_prop.apply(&mut style);
    }

    // Apply brightness - this should APPEND, not override
    if let Some(brightness_prop) = TailwindProperty::parse("brightness-150") {
      brightness_prop.apply(&mut style);
    }

    // Apply contrast - this should also APPEND
    if let Some(contrast_prop) = TailwindProperty::parse("contrast-125") {
      contrast_prop.apply(&mut style);
    }

    assert_eq!(
      style.filter,
      CssValue::Value(vec![
        Filter::Blur(Length::Px(8.0)),
        Filter::Brightness(PercentageNumber(1.5)),
        Filter::Contrast(PercentageNumber(1.25))
      ])
    )
  }
  #[test]
  fn test_parse_blend_mode() {
    assert_eq!(
      TailwindProperty::parse("mix-blend-multiply"),
      Some(TailwindProperty::MixBlendMode(BlendMode::Multiply))
    );
    assert_eq!(
      TailwindProperty::parse("bg-blend-screen"),
      Some(TailwindProperty::BackgroundBlendMode(BlendMode::Screen))
    );
  }
  #[test]
  fn test_parse_vertical_align() {
    assert_eq!(
      TailwindProperty::parse("align-baseline"),
      Some(TailwindProperty::VerticalAlign(VerticalAlign::Baseline))
    );
    assert_eq!(
      TailwindProperty::parse("align-top"),
      Some(TailwindProperty::VerticalAlign(VerticalAlign::Top))
    );
    assert_eq!(
      TailwindProperty::parse("align-middle"),
      Some(TailwindProperty::VerticalAlign(VerticalAlign::Middle))
    );
    assert_eq!(
      TailwindProperty::parse("align-bottom"),
      Some(TailwindProperty::VerticalAlign(VerticalAlign::Bottom))
    );
    assert_eq!(
      TailwindProperty::parse("align-text-top"),
      Some(TailwindProperty::VerticalAlign(VerticalAlign::TextTop))
    );
    assert_eq!(
      TailwindProperty::parse("align-text-bottom"),
      Some(TailwindProperty::VerticalAlign(VerticalAlign::TextBottom))
    );
    assert_eq!(
      TailwindProperty::parse("align-sub"),
      Some(TailwindProperty::VerticalAlign(VerticalAlign::Sub))
    );
    assert_eq!(
      TailwindProperty::parse("align-super"),
      Some(TailwindProperty::VerticalAlign(VerticalAlign::Super))
    );
  }
}
