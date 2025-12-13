pub(crate) mod map;
pub(crate) mod parser;

use std::{cmp::Ordering, ops::Neg, str::FromStr};

use serde::{Deserializer, de::Error as DeError};
use smallvec::smallvec;

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
pub struct Breakpoint(pub(crate) LengthUnit);

impl Breakpoint {
  /// Parse a breakpoint from a token.
  pub fn parse(token: &str) -> Option<Self> {
    match_ignore_ascii_case! {token,
      "sm" => Some(Breakpoint(LengthUnit::Rem(40.0))),
      "md" => Some(Breakpoint(LengthUnit::Rem(48.0))),
      "lg" => Some(Breakpoint(LengthUnit::Rem(64.0))),
      "xl" => Some(Breakpoint(LengthUnit::Rem(80.0))),
      "2xl" => Some(Breakpoint(LengthUnit::Rem(96.0))),
      _ => None,
    }
  }

  /// Check if the breakpoint matches the viewport width.
  pub fn matches(&self, viewport: Viewport) -> bool {
    let Some(viewport_width) = viewport.width else {
      return false;
    };

    let breakpoint_width = match self.0 {
      LengthUnit::Rem(value) => value * viewport.font_size * viewport.device_pixel_ratio,
      LengthUnit::Px(value) => value * viewport.device_pixel_ratio,
      LengthUnit::Vw(value) => (value / 100.0) * viewport_width as f32,
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
  FlexBasis(LengthUnit),
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
  /// `font-family` property.
  FontFamily(FontFamily),
  /// `line-clamp` property.
  LineClamp(LineClamp),
  /// `text-overflow` property.
  TextOverflow(TextOverflow),
  /// `text-wrap` property.
  TextWrap(TextWrapMode),
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
  TextDecoration(TextDecoration),
  /// `text-decoration-color` property.
  TextDecorationColor(ColorInput),
  /// `text-transform` property.
  TextTransform(TextTransform),
  /// `width` and `height` property.
  Size(LengthUnit),
  /// `width` property.
  Width(LengthUnit),
  /// `height` property.
  Height(LengthUnit),
  /// `min-width` property.
  MinWidth(LengthUnit),
  /// `min-height` property.
  MinHeight(LengthUnit),
  /// `max-width` property.
  MaxWidth(LengthUnit),
  /// `max-height` property.
  MaxHeight(LengthUnit),
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
  Gap(LengthUnit<false>),
  /// `column-gap` property.
  GapX(LengthUnit<false>),
  /// `row-gap` property.
  GapY(LengthUnit<false>),
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
  /// `border-width` property.
  BorderWidth(TwBorderWidth),
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
  Translate(LengthUnit),
  /// `translate-x` property.
  TranslateX(LengthUnit),
  /// `translate-y` property.
  TranslateY(LengthUnit),
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
  Margin(LengthUnit<false>),
  /// `margin-inline` property.
  MarginX(LengthUnit<false>),
  /// `margin-block` property.
  MarginY(LengthUnit<false>),
  /// `margin-top` property.
  MarginTop(LengthUnit<false>),
  /// `margin-right` property.
  MarginRight(LengthUnit<false>),
  /// `margin-bottom` property.
  MarginBottom(LengthUnit<false>),
  /// `margin-left` property.
  MarginLeft(LengthUnit<false>),
  /// `padding` property.
  Padding(LengthUnit<false>),
  /// `padding-inline` property.
  PaddingX(LengthUnit<false>),
  /// `padding-block` property.
  PaddingY(LengthUnit<false>),
  /// `padding-top` property.
  PaddingTop(LengthUnit<false>),
  /// `padding-right` property.
  PaddingRight(LengthUnit<false>),
  /// `padding-bottom` property.
  PaddingBottom(LengthUnit<false>),
  /// `padding-left` property.
  PaddingLeft(LengthUnit<false>),
  /// `inset` property.
  Inset(LengthUnit),
  /// `inset-inline` property.
  InsetX(LengthUnit),
  /// `inset-block` property.
  InsetY(LengthUnit),
  /// `top` property.
  Top(LengthUnit),
  /// `right` property.
  Right(LengthUnit),
  /// `bottom` property.
  Bottom(LengthUnit),
  /// `left` property.
  Left(LengthUnit),
}

/// A trait for parsing tailwind properties.
pub trait TailwindPropertyParser: Sized + for<'i> FromCss<'i> {
  /// Parse a tailwind property from a token.
  fn parse_tw(token: &str) -> Option<Self>;
}

impl Neg for TailwindProperty {
  type Output = Self;

  fn neg(self) -> Self::Output {
    match self {
      TailwindProperty::Margin(length_unit) => TailwindProperty::Margin(-length_unit),
      TailwindProperty::MarginX(length_unit) => TailwindProperty::MarginX(-length_unit),
      TailwindProperty::MarginY(length_unit) => TailwindProperty::MarginY(-length_unit),
      TailwindProperty::MarginTop(length_unit) => TailwindProperty::MarginTop(-length_unit),
      TailwindProperty::MarginRight(length_unit) => TailwindProperty::MarginRight(-length_unit),
      TailwindProperty::MarginBottom(length_unit) => TailwindProperty::MarginBottom(-length_unit),
      TailwindProperty::MarginLeft(length_unit) => TailwindProperty::MarginLeft(-length_unit),
      TailwindProperty::Padding(length_unit) => TailwindProperty::Padding(-length_unit),
      TailwindProperty::PaddingX(length_unit) => TailwindProperty::PaddingX(-length_unit),
      TailwindProperty::PaddingY(length_unit) => TailwindProperty::PaddingY(-length_unit),
      TailwindProperty::PaddingTop(length_unit) => TailwindProperty::PaddingTop(-length_unit),
      TailwindProperty::PaddingRight(length_unit) => TailwindProperty::PaddingRight(-length_unit),
      TailwindProperty::PaddingBottom(length_unit) => TailwindProperty::PaddingBottom(-length_unit),
      TailwindProperty::PaddingLeft(length_unit) => TailwindProperty::PaddingLeft(-length_unit),
      TailwindProperty::Inset(length_unit) => TailwindProperty::Inset(-length_unit),
      TailwindProperty::InsetX(length_unit) => TailwindProperty::InsetX(-length_unit),
      TailwindProperty::InsetY(length_unit) => TailwindProperty::InsetY(-length_unit),
      TailwindProperty::Top(length_unit) => TailwindProperty::Top(-length_unit),
      TailwindProperty::Right(length_unit) => TailwindProperty::Right(-length_unit),
      TailwindProperty::Bottom(length_unit) => TailwindProperty::Bottom(-length_unit),
      TailwindProperty::Left(length_unit) => TailwindProperty::Left(-length_unit),
      TailwindProperty::Translate(length_unit) => TailwindProperty::Translate(-length_unit),
      TailwindProperty::TranslateX(length_unit) => TailwindProperty::TranslateX(-length_unit),
      TailwindProperty::TranslateY(length_unit) => TailwindProperty::TranslateY(-length_unit),
      TailwindProperty::Scale(percentage_number) => TailwindProperty::Scale(-percentage_number),
      TailwindProperty::ScaleX(percentage_number) => TailwindProperty::ScaleX(-percentage_number),
      TailwindProperty::ScaleY(percentage_number) => TailwindProperty::ScaleY(-percentage_number),
      TailwindProperty::Rotate(angle) => TailwindProperty::Rotate(-angle),
      TailwindProperty::LetterSpacing(length_unit) => TailwindProperty::LetterSpacing(-length_unit),
      _ => self,
    }
  }
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
      TailwindProperty::FontFamily(ref font_family) => {
        style.font_family = Some(font_family.clone()).into();
      }
      TailwindProperty::LineClamp(ref line_clamp) => {
        style.line_clamp = Some(line_clamp.clone()).into();
      }
      TailwindProperty::TextAlign(text_align) => {
        style.text_align = text_align.into();
      }
      TailwindProperty::TextDecoration(ref text_decoration) => {
        style.text_decoration = text_decoration.clone().into();
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
        style.box_shadow = Some(smallvec![box_shadow]).into();
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
        style.background_position = Some(smallvec![background_position]).into();
      }
      TailwindProperty::BackgroundSize(background_size) => {
        style.background_size = Some(smallvec![background_size]).into();
      }
      TailwindProperty::BackgroundRepeat(background_repeat) => {
        style.background_repeat = Some(smallvec![background_repeat]).into();
      }
      TailwindProperty::BackgroundImage(ref background_image) => {
        style.background_image = Some(smallvec![background_image.clone()]).into();
      }
      TailwindProperty::BorderWidth(tw_border_width) => {
        style.border_width = Some(Sides([tw_border_width.0; 4])).into();
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
      TailwindProperty::Rounded(rounded) => {
        style.border_radius = BorderRadius(Sides([SpacePair::from_single(rounded.0); 4])).into();
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
      TailwindProperty::TextWrap(text_wrap_mode) => {
        style.text_wrap_mode = Some(text_wrap_mode).into();
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
      TailwindProperty::FontSize(font_size) => {
        style.font_size = Some(font_size.font_size).into();

        if let Some(line_height) = font_size.line_height {
          style.line_height = line_height.into();
        }
      }
      TailwindProperty::LineHeight(line_height) => {
        style.line_height = line_height.into();
      }
      TailwindProperty::Translate(length_unit) => {
        style.translate = Some(SpacePair::from_single(length_unit)).into();
      }
      TailwindProperty::TranslateX(length_unit) => {
        style.translate_x = Some(length_unit).into();
      }
      TailwindProperty::TranslateY(length_unit) => {
        style.translate_y = Some(length_unit).into();
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
      TailwindProperty::Margin(length_unit) => {
        style.margin = Sides([length_unit; 4]).into();
      }
      TailwindProperty::MarginX(length_unit) => {
        style.margin_inline = Some(SpacePair::from_single(length_unit)).into();
      }
      TailwindProperty::MarginY(length_unit) => {
        style.margin_block = Some(SpacePair::from_single(length_unit)).into();
      }
      TailwindProperty::MarginTop(length_unit) => {
        style.margin_top = Some(length_unit).into();
      }
      TailwindProperty::MarginRight(length_unit) => {
        style.margin_right = Some(length_unit).into();
      }
      TailwindProperty::MarginBottom(length_unit) => {
        style.margin_bottom = Some(length_unit).into();
      }
      TailwindProperty::MarginLeft(length_unit) => {
        style.margin_left = Some(length_unit).into();
      }
      TailwindProperty::Padding(length_unit) => {
        style.padding = Sides([length_unit; 4]).into();
      }
      TailwindProperty::PaddingX(length_unit) => {
        style.padding_inline = Some(SpacePair::from_single(length_unit)).into();
      }
      TailwindProperty::PaddingY(length_unit) => {
        style.padding_block = Some(SpacePair::from_single(length_unit)).into();
      }
      TailwindProperty::PaddingTop(length_unit) => {
        style.padding_top = Some(length_unit).into();
      }
      TailwindProperty::PaddingRight(length_unit) => {
        style.padding_right = Some(length_unit).into();
      }
      TailwindProperty::PaddingBottom(length_unit) => {
        style.padding_bottom = Some(length_unit).into();
      }
      TailwindProperty::PaddingLeft(length_unit) => {
        style.padding_left = Some(length_unit).into();
      }
      TailwindProperty::Inset(length_unit) => {
        style.inset = Sides([length_unit; 4]).into();
      }
      TailwindProperty::InsetX(length_unit) => {
        style.inset_inline = Some(SpacePair::from_single(length_unit)).into();
      }
      TailwindProperty::InsetY(length_unit) => {
        style.inset_block = Some(SpacePair::from_single(length_unit)).into();
      }
      TailwindProperty::Top(length_unit) => {
        style.top = Some(length_unit).into();
      }
      TailwindProperty::Right(length_unit) => {
        style.right = Some(length_unit).into();
      }
      TailwindProperty::Bottom(length_unit) => {
        style.bottom = Some(length_unit).into();
      }
      TailwindProperty::Left(length_unit) => {
        style.left = Some(length_unit).into();
      }
      TailwindProperty::GridAutoColumns(grid_auto_size) => {
        style.grid_auto_columns = Some(vec![grid_auto_size]).into();
      }
      TailwindProperty::GridAutoRows(grid_auto_size) => {
        style.grid_auto_rows = Some(vec![grid_auto_size]).into();
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
      Some(TailwindProperty::Width(LengthUnit::Rem(
        64.0 * TW_VAR_SPACING
      )))
    );
    assert_eq!(
      TailwindProperty::parse("h-32"),
      Some(TailwindProperty::Height(LengthUnit::Rem(
        32.0 * TW_VAR_SPACING
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
        basis: LengthUnit::Auto,
      }))
    );
  }

  #[test]
  fn test_parse_negative_margin() {
    assert_eq!(
      TailwindProperty::parse("-ml-4"),
      Some(TailwindProperty::MarginLeft(LengthUnit::Rem(
        -4.0 * TW_VAR_SPACING
      )))
    );
  }

  #[test]
  fn test_parse_border_radius() {
    assert_eq!(
      TailwindProperty::parse("rounded-xs"),
      Some(TailwindProperty::Rounded(TwRounded(LengthUnit::Rem(0.125))))
    );
    assert_eq!(
      TailwindProperty::parse("rounded-full"),
      Some(TailwindProperty::Rounded(TwRounded(LengthUnit::Px(9999.0))))
    );
  }

  #[test]
  fn test_parse_border_width() {
    assert_eq!(
      TailwindProperty::parse("border-t-2"),
      Some(TailwindProperty::BorderTopWidth(TwBorderWidth(
        LengthUnit::Px(2.0)
      )))
    );
    assert_eq!(
      TailwindProperty::parse("border-x-4"),
      Some(TailwindProperty::BorderXWidth(TwBorderWidth(
        LengthUnit::Px(4.0)
      )))
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
      "rounded-lg",
      // Transforms
      "rotate-45",
      "scale-75",
      "translate-x-4",
      // Grid
      "grid-cols-3",
      "col-span-2",
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
        property: TailwindProperty::MarginTop(LengthUnit::Rem(1.0)),
        breakpoint: Some(Breakpoint(LengthUnit::Rem(48.0))),
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
            property: TailwindProperty::MarginTop(LengthUnit::Rem(4.0)),
            breakpoint: None,
            important: false,
          },
          // sm:mt-8
          TailwindValue {
            property: TailwindProperty::MarginTop(LengthUnit::Rem(2.0)),
            breakpoint: Some(Breakpoint(LengthUnit::Rem(40.0))),
            important: false,
          },
          // !mt-12
          TailwindValue {
            property: TailwindProperty::MarginTop(LengthUnit::Rem(3.0)),
            breakpoint: None,
            important: true,
          },
          // md:!mt-4
          TailwindValue {
            property: TailwindProperty::MarginTop(LengthUnit::Rem(1.0)),
            breakpoint: Some(Breakpoint(LengthUnit::Rem(48.0))),
            important: true,
          },
        ]
      })
    )
  }
}
