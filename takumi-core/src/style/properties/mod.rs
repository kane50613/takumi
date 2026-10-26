//! Style properties and related types for the takumi styling system.
//!
//! This module contains CSS-like properties including layout properties,
//! typography settings, positioning, and visual effects.

mod animation;
mod aspect_ratio;
mod background;
mod background_image;
mod background_position;
pub(crate) mod background_repeat;
mod background_size;
mod blend_mode;
mod border;
mod box_alignment;
mod box_shadow;
mod breaks;
mod clip_path;
mod color;
pub(crate) mod conic_gradient;
mod contain;
mod content;
mod corner_shape;
pub(crate) mod filter;
#[cfg(feature = "svg")]
pub(crate) mod filter_reference;
mod flex;
mod flex_grow;
mod font;
mod font_family;
mod font_feature_settings;
mod font_kerning;
mod font_language_override;
mod font_optical_sizing;
mod font_size;
mod font_stretch;
mod font_style;
mod font_synthesis;
mod font_variant;
mod font_variation_settings;
mod font_weight;
mod gap;
pub(crate) mod gradient_utils;
mod grid;
mod hyphens;
mod intrinsic_sizing;
mod length;
mod line_break;
mod line_clamp;
mod line_height;
pub(crate) mod linear_gradient;
mod list_style;
mod max_size;
mod offset_path;
mod order;
mod overflow;
mod overflow_wrap;
mod percentage_number;
pub(crate) mod radial_gradient;
mod sides;
mod size;
mod space_pair;
mod svg;
mod tab_size;
mod text_align_last;
mod text_decoration;
mod text_fit;
mod text_indent;
mod text_justify;
mod text_overflow;
mod text_shadow;
mod text_stroke;
mod text_wrap;
mod traits;
mod transform;
mod unicode_bidi;
mod vertical_align;
mod white_space;
mod word_break;
mod z_index;

use std::{borrow::Cow, fmt};

pub use animation::*;
pub use aspect_ratio::*;
pub use background::*;
pub use background_image::*;
pub use background_position::*;
pub use background_repeat::{BackgroundRepeat, BackgroundRepeatStyle, BackgroundRepeats};
pub use background_size::*;
pub use blend_mode::*;
pub use border::*;
pub use box_alignment::*;
pub use box_shadow::*;
pub use breaks::*;
pub use clip_path::*;
pub use color::*;
pub use conic_gradient::ConicGradient;
pub use contain::*;
pub use content::*;
pub use corner_shape::*;
use cssparser::{ParseError as CssParseError, Parser, Token, match_ignore_ascii_case};
pub use filter::{
  BlurType, Filter, FilterCategory, Filters, LUMA_WEIGHTS, MapDirection, SEPIA_WEIGHTS,
  TransferChannel, TransferTable, expand_filter_bounds,
};
#[cfg(feature = "svg")]
pub use filter_reference::FilterReference;
pub use flex::*;
pub use flex_grow::*;
pub use font::*;
pub use font_family::*;
pub(crate) use font_feature_settings::FontFeatureSettings;
pub use font_feature_settings::{FontFeature, Tag};
pub use font_kerning::*;
pub use font_language_override::*;
pub use font_optical_sizing::*;
pub use font_size::*;
pub use font_stretch::*;
pub use font_style::*;
pub use font_synthesis::*;
pub use font_variant::*;
pub use font_variation_settings::FontVariation;
pub(crate) use font_variation_settings::FontVariationSettings;
pub use font_weight::*;
pub use gap::*;
pub use grid::*;
pub use hyphens::*;
pub use intrinsic_sizing::*;
pub use length::*;
pub use line_break::*;
pub use line_clamp::*;
pub use line_height::*;
pub use linear_gradient::{
  Angle, GradientKeywordDirection, GradientStop, HorizontalKeyword, LinearGradient,
  LinearGradientDirection, ResolvedGradientStop, StopPosition, VerticalKeyword,
};
pub use list_style::*;
pub use max_size::*;
pub use offset_path::*;
pub use order::*;
pub use overflow::*;
pub use overflow_wrap::*;
pub use percentage_number::*;
pub use radial_gradient::{RadialGradient, RadialShape, RadialSize};
use serde::Deserialize;
pub use sides::*;
pub use size::*;
pub use space_pair::*;
pub use svg::*;
pub use tab_size::*;
pub use text_align_last::*;
pub use text_decoration::*;
pub use text_fit::*;
pub use text_indent::*;
pub use text_justify::*;
pub use text_overflow::*;
pub use text_shadow::*;
pub use text_stroke::*;
pub use text_wrap::*;
use tiny_skia::FilterQuality as TinyFilterQuality;
pub use traits::*;
pub(crate) use traits::{declare_box_alignment_enum_impl, impl_comma_list_from_css, impl_css_enum};
pub use transform::*;
pub use unicode_bidi::*;
pub use vertical_align::*;
pub use white_space::*;
pub use word_break::*;
pub use z_index::*;

use crate::{
  layout::floats_utils::{Clear as PhysicalClear, Float as PhysicalFloat},
  style::{SizingContext, tw::TailwindPropertyParser},
};

/// Whether `ident` is a CSS-wide keyword or `default`, which a `<custom-ident>` excludes.
/// <https://drafts.csswg.org/css-values-4/#custom-idents>
pub(crate) fn is_reserved_custom_ident(ident: &str) -> bool {
  [
    "initial",
    "inherit",
    "unset",
    "revert",
    "revert-layer",
    "default",
  ]
  .iter()
  .any(|keyword| ident.eq_ignore_ascii_case(keyword))
}

pub(crate) fn next_is_comma<'i>(input: &mut Parser<'i>) -> bool {
  let state = input.state();
  let is_comma = input.expect_comma().is_ok();
  input.reset(&state);
  is_comma
}

/// Parses a comma-separated list of `parse_item`, requiring at least one.
pub(crate) fn parse_comma_list<'i, T>(
  input: &mut Parser<'i>,
  mut parse_item: impl FnMut(&mut Parser<'i>) -> ParseResult<'i, T>,
) -> ParseResult<'i, Box<[T]>> {
  let mut items = Vec::new();
  items.push(parse_item(input)?);
  while input.try_parse(Parser::expect_comma).is_ok() {
    items.push(parse_item(input)?);
  }
  Ok(items.into_boxed_slice())
}

// These parse Tailwind tokens straight through their `FromCss` value parser.
impl TailwindPropertyParser for ObjectFit {}
impl TailwindPropertyParser for ListStyleType {}
impl TailwindPropertyParser for ListStylePosition {}
impl TailwindPropertyParser for ListStyleImage {}
impl TailwindPropertyParser for TextAlign {}
impl TailwindPropertyParser for LineJoin {}
impl TailwindPropertyParser for AlignItems {}
impl TailwindPropertyParser for BorderStyle {}

macro_rules! unexpected_token {
  ($token:expr $(,)?) => {
    $crate::style::unexpected_token!(Self, $token)
  };
  ($type:ty, $token:expr $(,)?) => {
    $crate::style::build_unexpected_token(
      $token,
      <$type as $crate::style::FromCss>::EXPECT_MESSAGE,
      <$type as $crate::style::FromCss>::VALID_TOKENS,
    )
  };
}

pub(crate) use unexpected_token;

/// Builds the `ParseError` for an unexpected token, out-of-line and `#[cold]` to keep the many `FromCss` call sites tiny.
#[cold]
#[inline(never)]
pub(crate) fn build_unexpected_token<'i>(
  token: &Token<'i>,
  expect: CssExpectedMessage,
  valid_tokens: &'static [&'static str],
) -> CssParseError<ValueError<'i>> {
  CssParseError::custom(ValueError::UnexpectedToken {
    token: token.clone(),
    expect,
    valid_tokens,
  })
}

/// `tokens` as an error message lists them: keywords quoted, `<syntax>` as is, the last joined
/// with `or`.
pub(crate) fn merge_enum_values(tokens: &[&'static str]) -> String {
  let quoted: Vec<Cow<str>> = tokens
    .iter()
    .map(|token| {
      if token.starts_with('<') {
        Cow::Borrowed(*token)
      } else {
        Cow::Owned(format!("'{token}'"))
      }
    })
    .collect();

  match quoted.split_last() {
    None => String::new(),
    Some((last, [])) => last.to_string(),
    Some((last, rest)) => format!("{} or {last}", rest.join(", ")),
  }
}

/// Defines how an image should be resized to fit its container.
///
/// Similar to CSS object-fit property.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum ObjectFit {
  /// The replaced content is sized to fill the element's content box exactly, without maintaining aspect ratio
  #[default]
  Fill,
  /// The replaced content is scaled to maintain its aspect ratio while fitting within the element's content box
  Contain,
  /// The replaced content is sized to maintain its aspect ratio while filling the element's entire content box
  Cover,
  /// The content is sized as if none or contain were specified, whichever would result in a smaller concrete object size
  ScaleDown,
  /// The replaced content is not resized and maintains its intrinsic dimensions
  None,
}

impl_css_enum!(
  ObjectFit,
  "fill" => ObjectFit::Fill,
  "contain" => ObjectFit::Contain,
  "cover" => ObjectFit::Cover,
  "scale-down" => ObjectFit::ScaleDown,
  "none" => ObjectFit::None
);

/// Defines how the background is clipped.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
#[non_exhaustive]
pub enum BackgroundClip {
  /// The background extends to the outside edge of the border
  #[default]
  BorderBox,
  /// The background extends to the outside edge of the padding
  PaddingBox,
  /// The background extends to the inside edge of the content box
  ContentBox,
  /// The background extends to the outside edge of the text
  Text,
  /// The background extends to the outside edge of the border area
  BorderArea,
}

impl_css_enum!(
  BackgroundClip,
  "border-box" => BackgroundClip::BorderBox,
  "padding-box" => BackgroundClip::PaddingBox,
  "content-box" => BackgroundClip::ContentBox,
  "text" => BackgroundClip::Text,
  "border-area" => BackgroundClip::BorderArea
);

impl TailwindPropertyParser for BackgroundClip {
  fn parse_tw(token: &str) -> Option<Self> {
    match_ignore_ascii_case! {token,
      "border" => Some(BackgroundClip::BorderBox),
      "padding" => Some(BackgroundClip::PaddingBox),
      "content" => Some(BackgroundClip::ContentBox),
      "text" => Some(BackgroundClip::Text),
      _ => None,
    }
  }
}

/// Defines the positioning area that `background-position` and `background-size`
/// resolve against.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
#[non_exhaustive]
pub enum BackgroundOrigin {
  /// Position against the border box.
  BorderBox,
  /// Position against the padding box.
  #[default]
  PaddingBox,
  /// Position against the content box.
  ContentBox,
}

impl_css_enum!(
  BackgroundOrigin,
  "border-box" => BackgroundOrigin::BorderBox,
  "padding-box" => BackgroundOrigin::PaddingBox,
  "content-box" => BackgroundOrigin::ContentBox
);

impl TailwindPropertyParser for BackgroundOrigin {
  fn parse_tw(token: &str) -> Option<Self> {
    match_ignore_ascii_case! {token,
      "border" => Some(BackgroundOrigin::BorderBox),
      "padding" => Some(BackgroundOrigin::PaddingBox),
      "content" => Some(BackgroundOrigin::ContentBox),
      _ => None,
    }
  }
}

impl From<BackgroundClip> for Option<BackgroundOrigin> {
  fn from(clip: BackgroundClip) -> Self {
    match clip {
      BackgroundClip::BorderBox => Some(BackgroundOrigin::BorderBox),
      BackgroundClip::PaddingBox => Some(BackgroundOrigin::PaddingBox),
      BackgroundClip::ContentBox => Some(BackgroundOrigin::ContentBox),
      BackgroundClip::Text | BackgroundClip::BorderArea => None,
    }
  }
}

/// Represents the CSS `border-radius` property, supporting elliptical corners.
///
/// Each corner has independent horizontal and vertical radii, allowing for both circular and elliptical shapes.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct BorderRadius(pub Sides<SpacePair<Length>>);

impl From<f32> for BorderRadius {
  fn from(value: f32) -> Self {
    Self(Sides(
      [SpacePair::from_pair(Length::Px(value), Length::Px(value)); 4],
    ))
  }
}

impl MakeComputed for BorderRadius {
  fn make_computed(&mut self, sizing: &SizingContext) {
    self.0.make_computed(sizing);
  }
}

impl Animatable for BorderRadius {
  fn interpolate(
    &mut self,
    from: &Self,
    to: &Self,
    progress: f32,
    sizing: &SizingContext,
    current_color: Color,
  ) {
    self
      .0
      .interpolate(&from.0, &to.0, progress, sizing, current_color);
  }
}

impl<'i> FromCss<'i> for BorderRadius {
  fn from_css(input: &mut Parser<'i>) -> ParseResult<'i, Self> {
    let widths: Sides<Length> = Sides::from_css(input)?;

    let heights = if input.try_parse(|input| input.expect_delim('/')).is_ok() {
      Sides::from_css(input)?
    } else {
      widths
    };

    Ok(BorderRadius(Sides([
      SpacePair::from_pair(widths.0[0], heights.0[0]),
      SpacePair::from_pair(widths.0[1], heights.0[1]),
      SpacePair::from_pair(widths.0[2], heights.0[2]),
      SpacePair::from_pair(widths.0[3], heights.0[3]),
    ])))
  }

  const EXPECT_MESSAGE: CssExpectedMessage = CssExpectedMessage::BorderRadius;

  const VALID_TOKENS: &'static [&'static str] = &["<length>"];
}

/// Defines how the width and height of an element are calculated.
///
/// This enum determines whether the width and height properties include padding and border, or just the content area.
#[derive(Default, Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub enum BoxSizing {
  /// The width and height properties include padding and border, but not the content area
  ContentBox,
  /// The width and height properties include the content area, but not padding and border
  #[default]
  BorderBox,
}

impl_css_enum!(
  BoxSizing,
  "content-box" => BoxSizing::ContentBox,
  "border-box" => BoxSizing::BorderBox
);

/// Text alignment options for text rendering.
///
/// Corresponds to CSS text-align property values.
#[derive(Default, Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub enum TextAlign {
  /// Aligns inline content to the left edge of the line box
  Left,
  /// Aligns inline content to the right edge of the line box
  Right,
  /// Centers inline content within the line box
  Center,
  /// Expands inline content to fill the entire line box
  Justify,
  /// Aligns inline content to the start edge of the line box (language-dependent)
  #[default]
  Start,
  /// Aligns inline content to the end edge of the line box (language-dependent)
  End,
}

impl_css_enum!(
  TextAlign,
  "left" => TextAlign::Left,
  "right" => TextAlign::Right,
  "center" => TextAlign::Center,
  "justify" => TextAlign::Justify,
  "start" => TextAlign::Start,
  "end" => TextAlign::End
);

impl TextAlign {
  /// The physical side `start` and `end` name under `direction`.
  pub(crate) fn resolve(self, direction: Direction) -> Self {
    match (self, direction) {
      (Self::Start, Direction::Ltr) | (Self::End, Direction::Rtl) => Self::Left,
      (Self::Start, Direction::Rtl) | (Self::End, Direction::Ltr) => Self::Right,
      (align, _) => align,
    }
  }
}

/// Defines whether an element creates a new stacking context.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
#[non_exhaustive]
pub enum Isolation {
  /// The element creates a new stacking context.
  Isolate,
  /// Determine by other properties.
  #[default]
  Auto,
}

impl_css_enum!(
  Isolation,
  "isolate" => Isolation::Isolate,
  "auto" => Isolation::Auto
);

/// Defines whether an element is visible.
///
/// This controls whether an element is rendered, but unlike `display: none`,
/// it still takes up space in the layout.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
#[non_exhaustive]
pub enum Visibility {
  /// The element is visible.
  #[default]
  Visible,
  /// The element is invisible (not rendered) but still takes up space.
  Hidden,
  /// A table row, row group, column, or column group takes no space; anything else is `hidden`.
  Collapse,
}

impl_css_enum!(
  Visibility,
  "visible" => Visibility::Visible,
  "hidden" => Visibility::Hidden,
  "collapse" => Visibility::Collapse
);

/// Defines which side of the table a caption renders on.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
#[non_exhaustive]
pub enum CaptionSide {
  /// The caption box renders above the table.
  #[default]
  Top,
  /// The caption box renders below the table.
  Bottom,
}

impl_css_enum!(
  CaptionSide,
  "top" => CaptionSide::Top,
  "bottom" => CaptionSide::Bottom
);

/// Defines whether adjacent table cell borders collapse onto one grid line.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
#[non_exhaustive]
pub enum BorderCollapse {
  /// Every cell paints its own borders, kept apart by `border-spacing`.
  #[default]
  Separate,
  /// Adjacent borders resolve into a single shared border.
  Collapse,
}

impl_css_enum!(
  BorderCollapse,
  "separate" => BorderCollapse::Separate,
  "collapse" => BorderCollapse::Collapse
);

/// Defines whether a table cell with no content paints its borders and background.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
#[non_exhaustive]
pub enum EmptyCells {
  /// Empty cells paint as any other.
  #[default]
  Show,
  /// Empty cells paint nothing, unless the table collapses its borders.
  Hide,
}

impl_css_enum!(
  EmptyCells,
  "show" => EmptyCells::Show,
  "hide" => EmptyCells::Hide
);

/// Defines how a table distributes its column widths.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
#[non_exhaustive]
pub enum TableLayout {
  /// Column widths follow the content.
  #[default]
  Auto,
  /// Column widths come from the first row, and the rest share the space.
  Fixed,
}

impl_css_enum!(
  TableLayout,
  "auto" => TableLayout::Auto,
  "fixed" => TableLayout::Fixed
);

/// A `border-spacing` value: one or two non-negative lengths. A declaration
/// carrying a percentage, `auto`, or a negative length is discarded, so the
/// cascade keeps the previous one.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BorderSpacing(pub SpacePair<Length>);

impl Default for BorderSpacing {
  fn default() -> Self {
    Self(SpacePair::from_single(Length::zero()))
  }
}

impl MakeComputed for BorderSpacing {
  fn make_computed(&mut self, sizing: &SizingContext) {
    self.0.make_computed(sizing);
  }
}

impl Animatable for BorderSpacing {
  fn interpolate(
    &mut self,
    from: &Self,
    to: &Self,
    progress: f32,
    sizing: &SizingContext,
    current_color: Color,
  ) {
    self
      .0
      .interpolate(&from.0, &to.0, progress, sizing, current_color);
  }
}

impl<'i> FromCss<'i> for BorderSpacing {
  const VALID_TOKENS: &'static [&'static str] = SpacePair::<Length>::VALID_TOKENS;
  const EXPECT_MESSAGE: CssExpectedMessage = CssExpectedMessage::OneOrTwoValues;

  fn from_css(input: &mut Parser<'i>) -> ParseResult<'i, Self> {
    let pair = SpacePair::<Length>::from_css(input)?;

    if [pair.x, pair.y]
      .into_iter()
      .any(|length| matches!(length, Length::Auto | Length::Percentage(_)) || length.is_negative())
    {
      return Err(CssParseError::unexpected_token());
    }

    Ok(Self(pair))
  }
}

impl ToCss for BorderSpacing {
  fn to_css<W: fmt::Write>(&self, dest: &mut W) -> fmt::Result {
    self.0.to_css(dest)
  }
}

/// Defines how the corners of text strokes are rendered.
#[derive(Default, Debug, Clone, Copy, PartialEq)]
pub enum LineJoin {
  /// The corners are sharp and pointed.
  #[default]
  Miter,
  /// The corners are rounded.
  Round,
  /// The corners are cut off at a 45-degree angle.
  Bevel,
}

impl_css_enum!(
  LineJoin,
  "miter" => LineJoin::Miter,
  "round" => LineJoin::Round,
  "bevel" => LineJoin::Bevel
);

/// Defines the positioning method for an element.
///
/// This enum determines how an element is positioned within its containing element.
#[derive(Default, Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub enum Position {
  /// The element is laid out in the normal flow and is not a containing block.
  /// Offsets (top, right, bottom, left) have no effect.
  #[default]
  Static,
  /// The element is laid out in the normal flow, then offset relative to itself.
  /// Offsets (top, right, bottom, left) shift it without affecting other boxes.
  Relative,
  /// The element is removed from the normal document flow and positioned relative to its nearest positioned ancestor.
  /// Offsets (top, right, bottom, left) specify the distance from the ancestor.
  Absolute,
  /// The element is removed from the normal document flow and positioned relative to the viewport (root).
  Fixed,
}

impl_css_enum!(
  Position,
  "relative" => Position::Relative,
  "absolute" => Position::Absolute,
  "static" => Position::Static,
  "fixed" => Position::Fixed
);

impl Position {
  /// `position: relative`, which moves a box from where it lays out.
  pub(crate) const fn is_relative(self) -> bool {
    matches!(self, Self::Relative)
  }

  /// A positioned element (anything but `static`): establishes a containing
  /// block for absolutely-positioned descendants and honors `z-index`.
  pub(crate) const fn is_positioned(self) -> bool {
    matches!(self, Self::Relative | Self::Absolute | Self::Fixed)
  }

  /// Removed from normal flow.
  pub(crate) const fn is_out_of_flow(self) -> bool {
    matches!(self, Self::Absolute | Self::Fixed)
  }
}

/// Defines the direction of layout.
#[derive(Default, Debug, Clone, Copy, PartialEq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Direction {
  /// The layout direction is left-to-right.
  #[default]
  Ltr,
  /// The layout direction is right-to-left.
  Rtl,
}

impl Direction {
  /// The physical `(start, end)` inline sides, given as `left` and `right`.
  pub(crate) fn inline_sides<T>(self, left: T, right: T) -> (T, T) {
    match self {
      Self::Ltr => (left, right),
      Self::Rtl => (right, left),
    }
  }

  /// The paragraph embedding level of this base direction, as UAX #9 numbers it.
  pub(crate) fn bidi_level(self) -> u8 {
    match self {
      Self::Ltr => 0,
      Self::Rtl => 1,
    }
  }
}

impl_css_enum!(
  Direction,
  "ltr" => Direction::Ltr,
  "rtl" => Direction::Rtl
);

/// Defines whether an element should be placed along the left or right side of its container.
#[derive(Default, Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub enum Float {
  /// The element is not floated.
  #[default]
  None,
  /// The element floats to the left.
  Left,
  /// The element floats to the right.
  Right,
  /// The element floats to the logical start side.
  InlineStart,
  /// The element floats to the logical end side.
  InlineEnd,
}

impl_css_enum!(
  Float,
  "none" => Float::None,
  "left" => Float::Left,
  "right" => Float::Right,
  "inline-start" => Float::InlineStart,
  "inline-end" => Float::InlineEnd,
);

impl Float {
  /// Resolves the floating direction based on the layout direction.
  pub(crate) fn resolve(self, direction: Direction) -> PhysicalFloat {
    let (start, end) = direction.inline_sides(PhysicalFloat::Left, PhysicalFloat::Right);

    match self {
      Self::None => PhysicalFloat::None,
      Self::Left => PhysicalFloat::Left,
      Self::Right => PhysicalFloat::Right,
      Self::InlineStart => start,
      Self::InlineEnd => end,
    }
  }
}

/// Defines whether an element must be moved below preceding floated elements.
#[derive(Default, Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub enum Clear {
  /// The element is not moved down.
  #[default]
  None,
  /// The element is moved below left-floated elements.
  Left,
  /// The element is moved below right-floated elements.
  Right,
  /// The element is moved below both left- and right-floated elements.
  Both,
  /// The element is moved below logical start-floated elements.
  InlineStart,
  /// The element is moved below logical end-floated elements.
  InlineEnd,
}

impl_css_enum!(
  Clear,
  "none" => Clear::None,
  "left" => Clear::Left,
  "right" => Clear::Right,
  "both" => Clear::Both,
  "inline-start" => Clear::InlineStart,
  "inline-end" => Clear::InlineEnd,
);

impl Clear {
  /// Resolves the clearing direction based on the layout direction.
  pub(crate) fn resolve(self, direction: Direction) -> PhysicalClear {
    let (start, end) = direction.inline_sides(PhysicalClear::Left, PhysicalClear::Right);

    match self {
      Self::None => PhysicalClear::None,
      Self::Left => PhysicalClear::Left,
      Self::Right => PhysicalClear::Right,
      Self::Both => PhysicalClear::Both,
      Self::InlineStart => start,
      Self::InlineEnd => end,
    }
  }
}

/// Defines the direction of flex items within a flex container.
///
/// This enum determines how flex items are laid out along the main axis.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
#[non_exhaustive]
pub enum FlexDirection {
  /// Items are laid out in the same direction as the text direction (left-to-right for English)
  #[default]
  Row,
  /// Items are laid out perpendicular to the text direction (top-to-bottom)
  Column,
  /// Items are laid out in the opposite direction to the text direction (right-to-left for English)
  RowReverse,
  /// Items are laid out opposite to the column direction (bottom-to-top)
  ColumnReverse,
}

impl_css_enum!(
  FlexDirection,
  "row" => FlexDirection::Row,
  "column" => FlexDirection::Column,
  "row-reverse" => FlexDirection::RowReverse,
  "column-reverse" => FlexDirection::ColumnReverse
);

/// Defines how flex items are aligned along the main axis.
///
/// This enum determines how space is distributed between and around flex items
/// along the main axis of the flex container.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
#[non_exhaustive]
pub enum JustifyContent {
  /// The items are distributed using the normal flow of the flex container.
  #[default]
  Normal,
  /// Items are packed toward the start of the line.
  Start,
  /// Items are packed toward the end of the line.
  End,
  /// Items are packed toward the flex container's main-start side.
  /// For flex containers with flex_direction RowReverse or ColumnReverse, this is equivalent
  /// to End. In all other cases it is equivalent to Start.
  FlexStart,
  /// Items are packed toward the flex container's main-end side.
  /// For flex containers with flex_direction RowReverse or ColumnReverse, this is equivalent
  /// to Start. In all other cases it is equivalent to End.
  FlexEnd,
  /// Items are packed toward the center of the line.
  Center,
  /// Items are stretched to fill the container (only applies to flex containers)
  Stretch,
  /// Items are evenly distributed in the line; first item is on the start line,
  /// last item on the end line.
  SpaceBetween,
  /// Items are evenly distributed in the line with equal space around them.
  SpaceEvenly,
  /// Items are evenly distributed in the line; first item is on the start line,
  /// last item on the end line, and the space between items is twice the space
  /// between the start/end items and the container edges.
  SpaceAround,
  /// `safe start`: like `Start`, falling back to start-edge alignment on overflow.
  SafeStart,
  /// `safe end`: like `End`, falling back to start-edge alignment on overflow.
  SafeEnd,
  /// `safe flex-start`: like `FlexStart`, falling back to start-edge alignment on overflow.
  SafeFlexStart,
  /// `safe flex-end`: like `FlexEnd`, falling back to start-edge alignment on overflow.
  SafeFlexEnd,
  /// `safe center`: like `Center`, falling back to start-edge alignment on overflow.
  SafeCenter,
  /// Items are packed toward the start of the flow: the main-start side in a flex container.
  FlowStart,
  /// Items are packed toward the end of the flow: the main-end side in a flex container.
  FlowEnd,
  /// `safe flow-start`.
  SafeFlowStart,
  /// `safe flow-end`.
  SafeFlowEnd,
  /// `unsafe start`.
  UnsafeStart,
  /// `unsafe end`: like `End`, overflowing past the start edge.
  UnsafeEnd,
  /// `unsafe flex-start`.
  UnsafeFlexStart,
  /// `unsafe flex-end`.
  UnsafeFlexEnd,
  /// `unsafe flow-start`.
  UnsafeFlowStart,
  /// `unsafe flow-end`.
  UnsafeFlowEnd,
  /// `unsafe center`: like `Center`, overflowing past both edges.
  UnsafeCenter,
}

declare_box_alignment_enum_impl!(
  JustifyContent,
  safe {
    "start" => Start / SafeStart / UnsafeStart,
    "end" => End / SafeEnd / UnsafeEnd,
    "flex-start" => FlexStart / SafeFlexStart / UnsafeFlexStart,
    "flex-end" => FlexEnd / SafeFlexEnd / UnsafeFlexEnd,
    "flow-start" => FlowStart / SafeFlowStart / UnsafeFlowStart,
    "flow-end" => FlowEnd / SafeFlowEnd / UnsafeFlowEnd,
    "center" => Center / SafeCenter / UnsafeCenter,
  },
  plain {
    "normal" => Normal,
    "stretch" => Stretch,
    "space-between" => SpaceBetween,
    "space-around" => SpaceAround,
    "space-evenly" => SpaceEvenly,
  }
);

impl TailwindPropertyParser for JustifyContent {
  fn parse_tw(token: &str) -> Option<Self> {
    match token {
      "between" => Some(JustifyContent::SpaceBetween),
      "around" => Some(JustifyContent::SpaceAround),
      "evenly" => Some(JustifyContent::SpaceEvenly),
      _ => Self::from_css_str(token).ok(),
    }
  }
}

impl JustifyContent {}

/// This enum determines the layout algorithm used for the children of a node.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
#[non_exhaustive]
pub enum Display {
  /// The element is not displayed
  None,
  /// The element generates a flex container and its children follow the flexbox layout algorithm
  Flex,
  /// The element generates an inline-level flex container
  InlineFlex,
  /// The element generates a grid container and its children follow the CSS Grid layout algorithm
  Grid,
  /// The element generates an inline-level grid container
  InlineGrid,
  /// The element generates an inline container and its children follow the inline layout algorithm
  #[default]
  Inline,
  /// The element creates a block container and its children follow the block layout algorithm
  Block,
  /// The element generates an inline-level block container
  InlineBlock,
  /// The element creates a block container that establishes a new block formatting context
  FlowRoot,
  /// The element creates a block container that also generates a list marker
  ListItem,
  /// The element generates a table wrapper box
  Table,
  /// The element generates an inline-level table wrapper box
  InlineTable,
  /// The element groups rows rendered before every other row group
  TableHeaderGroup,
  /// The element groups rows in source order
  TableRowGroup,
  /// The element groups rows rendered after every other row group
  TableFooterGroup,
  /// The element generates a table row
  TableRow,
  /// The element generates a table cell
  TableCell,
  /// The element generates a table caption
  TableCaption,
  /// The element groups table columns
  TableColumnGroup,
  /// The element describes a table column
  TableColumn,
  /// The legacy `-webkit-box`: a flex container whose axis `-webkit-box-orient` sets
  WebkitBox,
  /// The legacy `-webkit-inline-box`: an inline-level `-webkit-box`
  WebkitInlineBox,
}

impl_css_enum!(
  Display,
  "none" => Display::None,
  "flex" | "-webkit-flex" => Display::Flex,
  "inline-flex" | "-webkit-inline-flex" => Display::InlineFlex,
  "-webkit-box" => Display::WebkitBox,
  "-webkit-inline-box" => Display::WebkitInlineBox,
  "grid" => Display::Grid,
  "inline-grid" => Display::InlineGrid,
  "inline" => Display::Inline,
  "block" => Display::Block,
  "inline-block" => Display::InlineBlock,
  "flow-root" => Display::FlowRoot,
  "list-item" => Display::ListItem,
  "table" => Display::Table,
  "inline-table" => Display::InlineTable,
  "table-header-group" => Display::TableHeaderGroup,
  "table-row-group" => Display::TableRowGroup,
  "table-footer-group" => Display::TableFooterGroup,
  "table-row" => Display::TableRow,
  "table-cell" => Display::TableCell,
  "table-caption" => Display::TableCaption,
  "table-column-group" => Display::TableColumnGroup,
  "table-column" => Display::TableColumn
);

impl Display {
  /// Returns true if the display creates an inline formatting context.
  pub(crate) fn is_inline(&self) -> bool {
    *self == Display::Inline
  }

  /// Returns true if the display participates in the inline flow as an atomic box.
  pub(crate) fn is_inline_level(&self) -> bool {
    matches!(
      self,
      Display::Inline
        | Display::InlineBlock
        | Display::InlineFlex
        | Display::InlineGrid
        | Display::InlineTable
        | Display::WebkitInlineBox
    )
  }

  /// Whether the box is a table wrapper box, block-level or inline-level.
  pub fn is_table(self) -> bool {
    matches!(self, Display::Table | Display::InlineTable)
  }

  /// Blink's `IsTableSection`: a header, footer, or body row group.
  pub(crate) fn is_table_section(self) -> bool {
    matches!(
      self,
      Display::TableHeaderGroup | Display::TableRowGroup | Display::TableFooterGroup
    )
  }

  /// Blink's `IsLayoutTableCol`: a column or column group.
  pub(crate) fn is_table_column(self) -> bool {
    matches!(self, Display::TableColumn | Display::TableColumnGroup)
  }

  /// Whether the box is a flex container, the legacy `-webkit-box` included.
  pub(crate) fn is_flex(self) -> bool {
    matches!(
      self,
      Display::Flex | Display::InlineFlex | Display::WebkitBox | Display::WebkitInlineBox
    )
  }

  pub(crate) fn is_grid(self) -> bool {
    matches!(self, Display::Grid | Display::InlineGrid)
  }

  /// Blink's `IsDeprecatedWebkitBox`.
  pub(crate) fn is_deprecated_webkit_box(self) -> bool {
    matches!(self, Display::WebkitBox | Display::WebkitInlineBox)
  }

  /// Blink's `IsTablePart`: a row group, row, cell, caption, column, or column group.
  pub(crate) fn is_table_part(self) -> bool {
    matches!(
      self,
      Display::TableRow
        | Display::TableRowGroup
        | Display::TableHeaderGroup
        | Display::TableFooterGroup
        | Display::TableCell
        | Display::TableCaption
        | Display::TableColumn
        | Display::TableColumnGroup
    )
  }

  /// Returns true if the display makes the children blockified (e.g., flex or grid).
  pub(crate) fn should_blockify_children(&self) -> bool {
    self.is_flex() || matches!(self, Display::Grid | Display::InlineGrid)
  }

  /// Cast the display to block level.
  /// Blink's `EquivalentBlockDisplay`, which turns the parts of a table into blocks as well.
  /// <https://drafts.csswg.org/css-display/#blockify>
  pub(crate) fn as_blockified(self) -> Self {
    match self {
      Display::Inline
      | Display::InlineBlock
      | Display::TableRowGroup
      | Display::TableHeaderGroup
      | Display::TableFooterGroup
      | Display::TableRow
      | Display::TableColumnGroup
      | Display::TableColumn
      | Display::TableCell
      | Display::TableCaption => Display::Block,
      Display::InlineTable => Display::Table,
      Display::InlineFlex => Display::Flex,
      Display::WebkitInlineBox => Display::WebkitBox,
      Display::InlineGrid => Display::Grid,
      _ => self,
    }
  }

  /// Mutate the display to be block level.
  pub(crate) fn blockify(&mut self) {
    *self = self.as_blockified();
  }
}

/// Legacy `-webkit-box-orient`: the axis of a `display: -webkit-box`, which ignores `flex-direction`.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
#[non_exhaustive]
pub enum BoxOrient {
  /// `horizontal` or `inline-axis`: children flow as a row.
  #[default]
  Horizontal,
  /// `vertical` or `block-axis`: children flow as a column.
  Vertical,
}

impl_css_enum!(
  BoxOrient,
  "horizontal" | "inline-axis" => BoxOrient::Horizontal,
  "vertical" | "block-axis" => BoxOrient::Vertical
);

/// Legacy `-webkit-box-pack` main-axis alignment, lowered to `justify-content`.
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub enum BoxPack {
  /// `start`: pack children at the main-axis start.
  Start,
  /// `end`: pack children at the main-axis end.
  End,
  /// `center`: pack children at the main-axis center.
  Center,
  /// `justify`: distribute children with space between them.
  Justify,
}

impl_css_enum!(
  BoxPack,
  "start" => BoxPack::Start,
  "end" => BoxPack::End,
  "center" => BoxPack::Center,
  "justify" => BoxPack::Justify
);

/// Legacy `-webkit-box-align` cross-axis alignment, lowered to `align-items`.
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub enum BoxAlign {
  /// `start`: align children at the cross-axis start.
  Start,
  /// `end`: align children at the cross-axis end.
  End,
  /// `center`: align children at the cross-axis center.
  Center,
  /// `baseline`: align children on their text baseline.
  Baseline,
  /// `stretch`: stretch children across the cross axis.
  Stretch,
}

impl_css_enum!(
  BoxAlign,
  "start" => BoxAlign::Start,
  "end" => BoxAlign::End,
  "center" => BoxAlign::Center,
  "baseline" => BoxAlign::Baseline,
  "stretch" => BoxAlign::Stretch
);

impl From<BoxOrient> for FlexDirection {
  fn from(orient: BoxOrient) -> Self {
    match orient {
      BoxOrient::Horizontal => FlexDirection::Row,
      BoxOrient::Vertical => FlexDirection::Column,
    }
  }
}

impl From<BoxPack> for JustifyContent {
  fn from(pack: BoxPack) -> Self {
    match pack {
      BoxPack::Start => JustifyContent::FlexStart,
      BoxPack::End => JustifyContent::FlexEnd,
      BoxPack::Center => JustifyContent::Center,
      BoxPack::Justify => JustifyContent::SpaceBetween,
    }
  }
}

impl From<BoxAlign> for AlignItems {
  fn from(align: BoxAlign) -> Self {
    match align {
      BoxAlign::Start => AlignItems::FlexStart,
      BoxAlign::End => AlignItems::FlexEnd,
      BoxAlign::Center => AlignItems::Center,
      BoxAlign::Baseline => AlignItems::Baseline,
      BoxAlign::Stretch => AlignItems::Stretch,
    }
  }
}

/// Defines how flex items are aligned along the cross axis.
///
/// This enum determines how items are aligned within the flex container
/// along the cross axis (perpendicular to the main axis).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
#[non_exhaustive]
pub enum AlignItems {
  /// The items are distributed using the normal flow of the flex container.
  #[default]
  Normal,
  /// Items are aligned to the start of the line in the cross axis
  Start,
  /// Items are aligned to the end of the line in the cross axis
  End,
  /// Items are aligned to the flex container's cross-start side
  FlexStart,
  /// Items are aligned to the flex container's cross-end side
  FlexEnd,
  /// Items are aligned to the start of their own writing mode's cross axis
  SelfStart,
  /// Items are aligned to the end of their own writing mode's cross axis
  SelfEnd,
  /// Items are centered in the cross axis
  Center,
  /// Items are aligned so that their baselines align
  Baseline,
  /// Items are stretched to fill the container in the cross axis
  Stretch,
  /// `safe start`: like `Start`, falling back to start-edge alignment on overflow.
  SafeStart,
  /// `safe end`: like `End`, falling back to start-edge alignment on overflow.
  SafeEnd,
  /// `safe flex-start`: like `FlexStart`, falling back to start-edge alignment on overflow.
  SafeFlexStart,
  /// `safe flex-end`: like `FlexEnd`, falling back to start-edge alignment on overflow.
  SafeFlexEnd,
  /// `safe self-start`: like `SelfStart`, falling back to start-edge alignment on overflow.
  SafeSelfStart,
  /// `safe self-end`: like `SelfEnd`, falling back to start-edge alignment on overflow.
  SafeSelfEnd,
  /// `safe center`: like `Center`, falling back to start-edge alignment on overflow.
  SafeCenter,
}

declare_box_alignment_enum_impl!(
  AlignItems,
  safe {
    "start" => Start / SafeStart,
    "end" => End / SafeEnd,
    "flex-start" => FlexStart / SafeFlexStart,
    "flex-end" => FlexEnd / SafeFlexEnd,
    "self-start" => SelfStart / SafeSelfStart,
    "self-end" => SelfEnd / SafeSelfEnd,
    "center" => Center / SafeCenter,
  },
  plain {
    "normal" => Normal,
    "baseline" => Baseline,
    "stretch" => Stretch,
  }
);

impl AlignItems {}

/// The case `text-transform` maps text to.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
#[non_exhaustive]
pub enum TextTransformCase {
  /// Leave the case.
  #[default]
  None,
  /// Transform all characters to uppercase
  Uppercase,
  /// Transform all characters to lowercase
  Lowercase,
  /// Uppercase the first letter of each word
  Capitalize,
}

/// `text-transform`: `none | [capitalize | uppercase | lowercase] || full-width || full-size-kana`.
/// <https://drafts.csswg.org/css-text-3/#text-transform-property>
#[derive(Debug, Clone, Copy, PartialEq, Default)]
#[non_exhaustive]
pub struct TextTransform {
  /// The case it maps text to.
  pub case: TextTransformCase,
  /// Whether it puts characters in their full-width forms.
  pub full_width: bool,
  /// Whether it puts small kana in their full-size forms.
  pub full_size_kana: bool,
}

impl From<TextTransformCase> for TextTransform {
  fn from(case: TextTransformCase) -> Self {
    Self {
      case,
      ..Self::default()
    }
  }
}

impl MakeComputed for TextTransform {}

impl Animatable for TextTransform {}

impl<'i> FromCss<'i> for TextTransform {
  fn from_css(input: &mut Parser<'i>) -> ParseResult<'i, Self> {
    if input
      .try_parse(|input| input.expect_ident_matching("none"))
      .is_ok()
    {
      return Ok(Self::default());
    }

    let mut transform = Self::default();
    let mut has_case = false;
    let mut parsed_any = false;

    // Each of the three may come once, in any order.
    while let Ok(ident) = input.try_parse(Parser::expect_ident_cloned) {
      match_ignore_ascii_case! { &ident,
        "uppercase" | "lowercase" | "capitalize" if !has_case => {
          has_case = true;
          transform.case = match_ignore_ascii_case! { &ident,
            "uppercase" => TextTransformCase::Uppercase,
            "lowercase" => TextTransformCase::Lowercase,
            _ => TextTransformCase::Capitalize,
          };
        },
        "full-width" if !transform.full_width => transform.full_width = true,
        "full-size-kana" if !transform.full_size_kana => transform.full_size_kana = true,
        _ => return Err(CssParseError::unexpected_token()),
      }

      parsed_any = true;
    }

    if !parsed_any {
      return Err(input.new_error_for_next_token());
    }

    Ok(transform)
  }

  const VALID_TOKENS: &'static [&'static str] = &[
    "none",
    "uppercase",
    "lowercase",
    "capitalize",
    "full-width",
    "full-size-kana",
  ];
}

impl ToCss for TextTransform {
  fn to_css<W: fmt::Write>(&self, dest: &mut W) -> fmt::Result {
    let case = match self.case {
      TextTransformCase::None => None,
      TextTransformCase::Uppercase => Some("uppercase"),
      TextTransformCase::Lowercase => Some("lowercase"),
      TextTransformCase::Capitalize => Some("capitalize"),
    };
    let parts: Vec<&str> = case
      .into_iter()
      .chain(self.full_width.then_some("full-width"))
      .chain(self.full_size_kana.then_some("full-size-kana"))
      .collect();

    if parts.is_empty() {
      dest.write_str("none")
    } else {
      dest.write_str(&parts.join(" "))
    }
  }
}

/// Controls whether text decoration should skip descenders.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
#[non_exhaustive]
pub enum TextDecorationSkipInk {
  /// Skip descenders and glyph interiors when painting decorations.
  #[default]
  Auto,
  /// Do not skip ink; paint decoration continuously.
  None,
}

impl_css_enum!(
  TextDecorationSkipInk,
  "auto" => TextDecorationSkipInk::Auto,
  "none" => TextDecorationSkipInk::None
);

/// Controls how whitespace should be collapsed.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
#[non_exhaustive]
pub enum WhiteSpaceCollapse {
  /// Preserve whitespace as is—spaces and tabs are not collapsed.
  Preserve,
  /// Collapse whitespace—spaces and tabs are collapsed.
  #[default]
  Collapse,
  /// Preserve spaces and remove breaks.
  PreserveSpaces,
  /// Preserve breaks and collapse spaces.
  PreserveBreaks,
  /// Preserve whitespace, with a soft wrap opportunity after every preserved space.
  BreakSpaces,
}

impl_css_enum!(
  WhiteSpaceCollapse,
  "preserve" => WhiteSpaceCollapse::Preserve,
  "collapse" => WhiteSpaceCollapse::Collapse,
  "preserve-spaces" => WhiteSpaceCollapse::PreserveSpaces,
  "preserve-breaks" => WhiteSpaceCollapse::PreserveBreaks,
  "break-spaces" => WhiteSpaceCollapse::BreakSpaces,
);

impl WhiteSpaceCollapse {
  /// Blink's `ShouldPreserveWhiteSpaces`.
  pub(crate) fn should_preserve_white_spaces(self) -> bool {
    matches!(
      self,
      Self::Preserve | Self::PreserveSpaces | Self::BreakSpaces
    )
  }

  /// Blink's `ShouldCollapseWhiteSpaces`.
  pub(crate) fn should_collapse_white_spaces(self) -> bool {
    !self.should_preserve_white_spaces()
  }

  /// Blink's `ShouldPreserveBreaks`.
  pub(crate) fn should_preserve_breaks(self) -> bool {
    matches!(
      self,
      Self::Preserve | Self::PreserveBreaks | Self::BreakSpaces
    )
  }

  /// Blink's `ShouldBreakSpaces`.
  pub(crate) fn should_break_spaces(self) -> bool {
    self == Self::BreakSpaces
  }
}

/// Defines how images should be scaled when rendered.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
#[non_exhaustive]
pub enum ImageScalingAlgorithm {
  /// The image is scaled using Catmull-Rom interpolation.
  /// This is balanced for speed and quality.
  #[default]
  Auto,
  /// The image is scaled using Lanczos3 resampling.
  /// This provides high-quality scaling but may be slower.
  Smooth,
  /// The image is scaled using nearest neighbor interpolation,
  /// which is suitable for pixel art or images where sharp edges are desired.
  Pixelated,
}

impl From<ImageScalingAlgorithm> for TinyFilterQuality {
  fn from(algorithm: ImageScalingAlgorithm) -> Self {
    match algorithm {
      ImageScalingAlgorithm::Pixelated => Self::Nearest,
      _ => Self::Bilinear,
    }
  }
}

impl_css_enum!(
  ImageScalingAlgorithm,
  "auto" | "optimizeQuality" => ImageScalingAlgorithm::Auto,
  "smooth" | "high-quality" => ImageScalingAlgorithm::Smooth,
  "pixelated" | "crisp-edges" | "optimizeSpeed" => ImageScalingAlgorithm::Pixelated
);

#[cfg(test)]
mod image_scaling_algorithm_tests {
  use crate::style::{FromCssStr, ImageScalingAlgorithm};

  #[test]
  fn svg_and_css_names_pick_the_same_algorithm() {
    for (keyword, algorithm) in [
      ("optimizeQuality", ImageScalingAlgorithm::Auto),
      ("high-quality", ImageScalingAlgorithm::Smooth),
      ("crisp-edges", ImageScalingAlgorithm::Pixelated),
      ("optimizeSpeed", ImageScalingAlgorithm::Pixelated),
    ] {
      assert_eq!(ImageScalingAlgorithm::from_css_str(keyword), Ok(algorithm));
    }
  }
}

/// Represents border style options.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub enum BorderStyle {
  /// No border will be rendered.
  #[default]
  None,
  /// Forces the border to be hidden.
  Hidden,
  /// Dotted border style.
  Dotted,
  /// Dashed border style.
  Dashed,
  /// Solid border style.
  Solid,
  /// Double border style.
  Double,
  /// Groove border style.
  Groove,
  /// Ridge border style.
  Ridge,
  /// Inset border style.
  Inset,
  /// Outset border style.
  Outset,
}

impl BorderStyle {
  /// Returns whether this border style should paint and reserve border width.
  pub const fn is_rendered(self) -> bool {
    !matches!(self, Self::None | Self::Hidden)
  }
}

impl_css_enum!(
  BorderStyle,
  "none" => BorderStyle::None,
  "hidden" => BorderStyle::Hidden,
  "dotted" => BorderStyle::Dotted,
  "dashed" => BorderStyle::Dashed,
  "solid" => BorderStyle::Solid,
  "double" => BorderStyle::Double,
  "groove" => BorderStyle::Groove,
  "ridge" => BorderStyle::Ridge,
  "inset" => BorderStyle::Inset,
  "outset" => BorderStyle::Outset,
);

#[cfg(test)]
mod display_tests {
  use crate::style::{Display, FromCssStr};

  #[test]
  fn legacy_webkit_values_map_to_flex_layout() {
    let webkit_box = Display::from_css_str("-webkit-box").unwrap();
    let webkit_inline_box = Display::from_css_str("-webkit-inline-box").unwrap();

    assert_eq!(webkit_box, Display::WebkitBox);
    assert_eq!(webkit_inline_box.as_blockified(), Display::WebkitBox);
    assert!(webkit_inline_box.is_inline_level() && webkit_inline_box.is_flex());
    assert_eq!(Display::from_css_str("-webkit-flex"), Ok(Display::Flex));
    assert_eq!(
      Display::from_css_str("-webkit-inline-flex"),
      Ok(Display::InlineFlex)
    );
  }

  #[test]
  fn flow_root_is_a_block_level_box() {
    let flow_root = Display::from_css_str("flow-root").unwrap();

    assert_eq!(flow_root, Display::FlowRoot);
    assert_eq!(flow_root.as_blockified(), Display::FlowRoot);
    assert!(!flow_root.is_inline_level());
  }
}

#[cfg(test)]
mod border_spacing_tests {
  use crate::style::{BorderSpacing, Length, SpacePair, properties::traits::FromCssStr};

  #[test]
  fn border_spacing_takes_one_or_two_lengths() {
    assert_eq!(
      BorderSpacing::from_css_str("4px"),
      Ok(BorderSpacing(SpacePair::from_single(Length::Px(4.0))))
    );
    assert_eq!(
      BorderSpacing::from_css_str("4px 8px"),
      Ok(BorderSpacing(SpacePair::from_pair(
        Length::Px(4.0),
        Length::Px(8.0)
      )))
    );
  }

  #[test]
  fn border_spacing_rejects_what_its_grammar_forbids() {
    assert!(BorderSpacing::from_css_str("auto").is_err());
    assert!(BorderSpacing::from_css_str("5%").is_err());
    assert!(BorderSpacing::from_css_str("4px 5%").is_err());
    assert!(BorderSpacing::from_css_str("-4px").is_err());
    assert!(BorderSpacing::from_css_str("4px -8px").is_err());
  }
}

#[cfg(test)]
mod text_transform_tests {
  use crate::style::{FromCssStr, TextTransform, TextTransformCase, ToCss};

  #[test]
  fn parses_a_case_full_width_and_full_size_kana_in_any_order() {
    let transform = TextTransform::from_css_str("full-size-kana uppercase full-width").unwrap();

    assert_eq!(
      transform,
      TextTransform {
        case: TextTransformCase::Uppercase,
        full_width: true,
        full_size_kana: true,
      }
    );
    assert_eq!(
      transform.to_css_string(),
      "uppercase full-width full-size-kana"
    );
    assert_eq!(
      TextTransform::from_css_str("none").unwrap().to_css_string(),
      "none"
    );
  }

  #[test]
  fn rejects_a_repeated_part() {
    for value in [
      "uppercase lowercase",
      "full-width full-width",
      "full-size-kana capitalize full-size-kana",
      "",
    ] {
      assert!(TextTransform::from_css_str(value).is_err(), "{value:?}");
    }
  }
}
