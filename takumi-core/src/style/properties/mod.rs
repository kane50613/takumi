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
mod background_size_resolve;
mod blend_mode;
mod border;
mod box_alignment;
mod box_shadow;
mod breaks;
mod clip_path;
mod color;
pub(crate) mod conic_gradient;
mod content;
mod corner_shape;
pub(crate) mod filter;
#[cfg(feature = "svg")]
pub(crate) mod filter_reference;
mod flex;
mod flex_grow;
mod font_family;
mod font_feature_settings;
mod font_kerning;
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
mod length;
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
mod space_pair;
mod tab_size;
mod text_decoration;
mod text_fit;
mod text_indent;
mod text_overflow;
mod text_shadow;
mod text_stroke;
mod text_wrap;
mod traits;
mod transform;
mod vertical_align;
mod white_space;
mod word_break;
mod z_index;

use std::fmt;

pub use animation::*;
pub use aspect_ratio::*;
pub use background::*;
pub use background_image::*;
pub use background_position::*;
pub use background_repeat::{BackgroundRepeat, BackgroundRepeatStyle, BackgroundRepeats};
pub use background_size::*;
pub use background_size_resolve::*;
pub use blend_mode::*;
pub use border::*;
pub use box_alignment::*;
pub use box_shadow::*;
pub use breaks::*;
pub use clip_path::*;
pub use color::*;
pub use conic_gradient::ConicGradient;
pub use content::*;
pub use corner_shape::*;
use cssparser::{Parser, match_ignore_ascii_case};
pub use filter::{
  BlurType, Filter, FilterCategory, Filters, LUMA_WEIGHTS, SEPIA_WEIGHTS, TransferChannel,
  TransferTable,
};
#[cfg(feature = "svg")]
pub use filter_reference::FilterReference;
pub use flex::*;
pub use flex_grow::*;
pub use font_family::*;
pub(crate) use font_feature_settings::FontFeatureSettings;
pub use font_feature_settings::{FontFeature, Tag};
pub use font_kerning::*;
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
pub use length::*;
pub use line_clamp::*;
pub use line_height::*;
pub(crate) use linear_gradient::GradientStops;
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
use parley::Alignment;
pub use percentage_number::*;
pub use radial_gradient::{RadialGradient, RadialShape, RadialSize};
use serde::Deserialize;
pub use sides::*;
pub use space_pair::*;
pub use tab_size::*;
pub use text_decoration::*;
pub use text_fit::*;
pub use text_indent::*;
pub use text_overflow::*;
pub use text_shadow::*;
pub use text_stroke::*;
pub use text_wrap::*;
pub use traits::*;
pub(crate) use traits::{
  declare_box_alignment_enum_impl, declare_enum_from_css_impl, impl_from_taffy_enum,
};
pub use transform::*;
pub use vertical_align::*;
pub use white_space::*;
pub use word_break::*;
pub use z_index::*;

use crate::style::{SizingContext, tw::TailwindPropertyParser};

pub(crate) fn next_is_comma<'i>(input: &mut Parser<'i, '_>) -> bool {
  let state = input.state();
  let is_comma = input.expect_comma().is_ok();
  input.reset(&state);
  is_comma
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

impl<T: Animatable + Copy> Animatable for SpacePair<T> {
  fn interpolate(
    &mut self,
    from: &Self,
    to: &Self,
    progress: f32,
    sizing: &SizingContext,
    current_color: Color,
  ) {
    self
      .x
      .interpolate(&from.x, &to.x, progress, sizing, current_color);
    self
      .y
      .interpolate(&from.y, &to.y, progress, sizing, current_color);
  }
}

impl<T: Animatable + Copy> Animatable for Sides<T> {
  fn interpolate(
    &mut self,
    from: &Self,
    to: &Self,
    progress: f32,
    sizing: &SizingContext,
    current_color: Color,
  ) {
    for (index, value) in self.0.iter_mut().enumerate() {
      value.interpolate(
        &from.0[index],
        &to.0[index],
        progress,
        sizing,
        current_color,
      );
    }
  }
}

macro_rules! unexpected_token {
  ($type:ty, $location:expr, $token:expr $(,)?) => {
    $crate::style::build_unexpected_token(
      $location,
      $token,
      <$type as $crate::style::FromCss>::EXPECT_MESSAGE,
      <$type as $crate::style::FromCss>::VALID_TOKENS,
    )
  };
  ($location:expr, $token:expr $(,)?) => {
    $crate::style::unexpected_token!(Self, $location, $token)
  };
}

pub(crate) use unexpected_token;

/// Builds the `ParseError` for an unexpected token, out-of-line and `#[cold]` to keep the many `FromCss` call sites tiny.
#[cold]
#[inline(never)]
pub(crate) fn build_unexpected_token<'i>(
  location: cssparser::SourceLocation,
  token: &cssparser::Token<'_>,
  expect: CssExpectedMessage,
  valid_tokens: &'static [CssToken],
) -> cssparser::ParseError<'i, std::borrow::Cow<'i, str>> {
  let token = cssparser::ToCss::to_css_string(token);
  let message = expect.build_message(&token, merge_enum_values(valid_tokens));

  cssparser::ParseError {
    location,
    kind: cssparser::ParseErrorKind::Custom(std::borrow::Cow::Owned(message)),
  }
}

/// Helper function to merge enum values into a human-readable format.
/// - `["fill"]` → `"'fill'"`
/// - `["fill", "contain"]` → `"'fill' or 'contain'"`
/// - `["fill", "contain", "cover"]` → `"'fill', 'contain' or 'cover'"`
pub(crate) fn merge_enum_values(values: &[CssToken]) -> String {
  match values.len() {
    0 => String::new(),
    1 => values[0].to_string(),
    2 => format!("{} or {}", values[0], values[1]),
    _ => {
      let all_but_last = values[..values.len() - 1]
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(", ");
      format!("{} or {}", all_but_last, values[values.len() - 1])
    }
  }
}

/// Write a CSS quoted string, escaping backslashes and double quotes.
pub(crate) fn write_css_string<W: fmt::Write>(dest: &mut W, s: &str) -> fmt::Result {
  dest.write_char('"')?;
  for ch in s.chars() {
    match ch {
      '\\' => dest.write_str("\\\\")?,
      '"' => dest.write_str("\\\"")?,
      c => dest.write_char(c)?,
    }
  }
  dest.write_char('"')
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

declare_enum_from_css_impl!(
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

declare_enum_from_css_impl!(
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

declare_enum_from_css_impl!(
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
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
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

  const VALID_TOKENS: &'static [CssToken] = &[CssToken::Syntax(CssSyntaxKind::Length)];
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

declare_enum_from_css_impl!(
  BoxSizing,
  "content-box" => BoxSizing::ContentBox,
  "border-box" => BoxSizing::BorderBox
);

impl_from_taffy_enum!(BoxSizing, into_taffy -> taffy::BoxSizing, ContentBox, BorderBox);

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

declare_enum_from_css_impl!(
  TextAlign,
  "left" => TextAlign::Left,
  "right" => TextAlign::Right,
  "center" => TextAlign::Center,
  "justify" => TextAlign::Justify,
  "start" => TextAlign::Start,
  "end" => TextAlign::End
);

impl_from_taffy_enum!(
  TextAlign, into_parley -> Alignment, Left, Right, Center, Justify, Start, End
);

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

declare_enum_from_css_impl!(
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
}

declare_enum_from_css_impl!(
  Visibility,
  "visible" => Visibility::Visible,
  "hidden" => Visibility::Hidden
);

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

declare_enum_from_css_impl!(
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

declare_enum_from_css_impl!(
  Position,
  "relative" => Position::Relative,
  "absolute" => Position::Absolute,
  "static" => Position::Static,
  "fixed" => Position::Fixed
);

impl Position {
  pub(crate) fn into_taffy(self) -> taffy::Position {
    match self {
      Position::Relative | Position::Static => taffy::Position::Relative,
      Position::Absolute | Position::Fixed => taffy::Position::Absolute,
    }
  }
}

impl Position {
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

declare_enum_from_css_impl!(
  Direction,
  "ltr" => Direction::Ltr,
  "rtl" => Direction::Rtl
);

impl_from_taffy_enum!(Direction, into_taffy -> taffy::Direction, Ltr, Rtl);

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

declare_enum_from_css_impl!(
  Float,
  "none" => Float::None,
  "left" => Float::Left,
  "right" => Float::Right,
  "inline-start" => Float::InlineStart,
  "inline-end" => Float::InlineEnd,
);

impl Float {
  /// Resolves the floating direction based on the layout direction.
  pub(crate) fn resolve(self, direction: Direction) -> taffy::Float {
    match self {
      Self::None => taffy::Float::None,
      Self::Left => taffy::Float::Left,
      Self::Right => taffy::Float::Right,
      Self::InlineStart => {
        if direction == Direction::Rtl {
          taffy::Float::Right
        } else {
          taffy::Float::Left
        }
      }
      Self::InlineEnd => {
        if direction == Direction::Rtl {
          taffy::Float::Left
        } else {
          taffy::Float::Right
        }
      }
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

declare_enum_from_css_impl!(
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
  pub(crate) fn resolve(self, direction: Direction) -> taffy::Clear {
    match self {
      Self::None => taffy::Clear::None,
      Self::Left => taffy::Clear::Left,
      Self::Right => taffy::Clear::Right,
      Self::Both => taffy::Clear::Both,
      Self::InlineStart => {
        if direction == Direction::Rtl {
          taffy::Clear::Right
        } else {
          taffy::Clear::Left
        }
      }
      Self::InlineEnd => {
        if direction == Direction::Rtl {
          taffy::Clear::Left
        } else {
          taffy::Clear::Right
        }
      }
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

declare_enum_from_css_impl!(
  FlexDirection,
  "row" => FlexDirection::Row,
  "column" => FlexDirection::Column,
  "row-reverse" => FlexDirection::RowReverse,
  "column-reverse" => FlexDirection::ColumnReverse
);

impl_from_taffy_enum!(
  FlexDirection,
  into_taffy -> taffy::FlexDirection,
  Row,
  Column,
  RowReverse,
  ColumnReverse
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
}

declare_box_alignment_enum_impl!(
  JustifyContent,
  safe {
    "start" => Start / SafeStart,
    "end" => End / SafeEnd,
    "flex-start" => FlexStart / SafeFlexStart,
    "flex-end" => FlexEnd / SafeFlexEnd,
    "center" => Center / SafeCenter,
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

impl JustifyContent {
  pub(crate) fn into_taffy(self) -> Option<taffy::JustifyContent> {
    match self {
      JustifyContent::Normal => None,
      JustifyContent::Start => Some(taffy::JustifyContent::START),
      JustifyContent::End => Some(taffy::JustifyContent::END),
      JustifyContent::FlexStart => Some(taffy::JustifyContent::FLEX_START),
      JustifyContent::FlexEnd => Some(taffy::JustifyContent::FLEX_END),
      JustifyContent::Center => Some(taffy::JustifyContent::CENTER),
      JustifyContent::Stretch => Some(taffy::JustifyContent::STRETCH),
      JustifyContent::SpaceBetween => Some(taffy::JustifyContent::SPACE_BETWEEN),
      JustifyContent::SpaceAround => Some(taffy::JustifyContent::SPACE_AROUND),
      JustifyContent::SpaceEvenly => Some(taffy::JustifyContent::SPACE_EVENLY),
      JustifyContent::SafeStart => Some(taffy::JustifyContent::SAFE_START),
      JustifyContent::SafeEnd => Some(taffy::JustifyContent::SAFE_END),
      JustifyContent::SafeFlexStart => Some(taffy::JustifyContent::SAFE_FLEX_START),
      JustifyContent::SafeFlexEnd => Some(taffy::JustifyContent::SAFE_FLEX_END),
      JustifyContent::SafeCenter => Some(taffy::JustifyContent::SAFE_CENTER),
    }
  }
}

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
  /// The element creates a block container that also generates a list marker
  ListItem,
  /// The element generates a table wrapper box
  Table,
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
}

declare_enum_from_css_impl!(
  Display,
  "none" => Display::None,
  "flex" => Display::Flex,
  "inline-flex" => Display::InlineFlex,
  "grid" => Display::Grid,
  "inline-grid" => Display::InlineGrid,
  "inline" => Display::Inline,
  "block" => Display::Block,
  "inline-block" => Display::InlineBlock,
  "list-item" => Display::ListItem,
  "table" => Display::Table,
  "table-header-group" => Display::TableHeaderGroup,
  "table-row-group" => Display::TableRowGroup,
  "table-footer-group" => Display::TableFooterGroup,
  "table-row" => Display::TableRow,
  "table-cell" => Display::TableCell,
  "table-caption" => Display::TableCaption
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
      Display::Inline | Display::InlineBlock | Display::InlineFlex | Display::InlineGrid
    )
  }

  /// Returns true if the display makes the children blockified (e.g., flex or grid).
  pub(crate) fn should_blockify_children(&self) -> bool {
    matches!(
      self,
      Display::Flex | Display::InlineFlex | Display::Grid | Display::InlineGrid
    )
  }

  /// Cast the display to block level.
  pub(crate) fn as_blockified(self) -> Self {
    match self {
      Display::Inline => Display::Block,
      Display::InlineBlock => Display::Block,
      Display::InlineFlex => Display::Flex,
      Display::InlineGrid => Display::Grid,
      _ => self,
    }
  }

  /// Mutate the display to be block level.
  pub(crate) fn blockify(&mut self) {
    *self = self.as_blockified();
  }
}

impl Display {
  pub(crate) fn into_taffy(self) -> taffy::Display {
    match self {
      Display::Flex | Display::InlineFlex => taffy::Display::Flex,
      Display::Grid | Display::InlineGrid => taffy::Display::Grid,
      Display::Block | Display::InlineBlock | Display::Inline | Display::ListItem => {
        taffy::Display::Block
      }
      // Lowering replaces every table box that sits in a table, so what is left
      // here is a table part outside one. Blink wraps those in anonymous table
      // boxes; block is the approximation.
      Display::Table
      | Display::TableHeaderGroup
      | Display::TableRowGroup
      | Display::TableFooterGroup
      | Display::TableRow
      | Display::TableCell
      | Display::TableCaption => taffy::Display::Block,
      Display::None => taffy::Display::None,
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
    "center" => Center / SafeCenter,
  },
  plain {
    "normal" => Normal,
    "baseline" => Baseline,
    "stretch" => Stretch,
  }
);

impl AlignItems {
  pub(crate) fn into_taffy(self) -> Option<taffy::AlignItems> {
    match self {
      AlignItems::Normal => None,
      AlignItems::Start => Some(taffy::AlignItems::START),
      AlignItems::End => Some(taffy::AlignItems::END),
      AlignItems::FlexStart => Some(taffy::AlignItems::FLEX_START),
      AlignItems::FlexEnd => Some(taffy::AlignItems::FLEX_END),
      AlignItems::Center => Some(taffy::AlignItems::CENTER),
      AlignItems::Baseline => Some(taffy::AlignItems::BASELINE),
      AlignItems::Stretch => Some(taffy::AlignItems::STRETCH),
      AlignItems::SafeStart => Some(taffy::AlignItems::SAFE_START),
      AlignItems::SafeEnd => Some(taffy::AlignItems::SAFE_END),
      AlignItems::SafeFlexStart => Some(taffy::AlignItems::SAFE_FLEX_START),
      AlignItems::SafeFlexEnd => Some(taffy::AlignItems::SAFE_FLEX_END),
      AlignItems::SafeCenter => Some(taffy::AlignItems::SAFE_CENTER),
    }
  }
}

/// Defines how flex items should wrap.
///
/// This enum determines how flex items should wrap within the flex container.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
#[non_exhaustive]
pub enum FlexWrap {
  /// Flex items will all be displayed in a single line, shrinking as needed
  #[default]
  NoWrap,
  /// Flex items will wrap onto multiple lines, with new lines stacking in the flex direction
  Wrap,
  /// Flex items will wrap onto multiple lines, with new lines stacking in the reverse flex direction
  WrapReverse,
}

declare_enum_from_css_impl!(
  FlexWrap,
  "nowrap" => FlexWrap::NoWrap,
  "wrap" => FlexWrap::Wrap,
  "wrap-reverse" => FlexWrap::WrapReverse
);

impl_from_taffy_enum!(FlexWrap, into_taffy -> taffy::FlexWrap, NoWrap, Wrap, WrapReverse);

/// Controls text case transformation when rendering.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
#[non_exhaustive]
pub enum TextTransform {
  /// Do not transform text
  #[default]
  None,
  /// Transform all characters to uppercase
  Uppercase,
  /// Transform all characters to lowercase
  Lowercase,
  /// Uppercase the first letter of each word
  Capitalize,
}

declare_enum_from_css_impl!(
  TextTransform,
  "none" => TextTransform::None,
  "uppercase" => TextTransform::Uppercase,
  "lowercase" => TextTransform::Lowercase,
  "capitalize" => TextTransform::Capitalize
);

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

declare_enum_from_css_impl!(
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
}

declare_enum_from_css_impl!(
  WhiteSpaceCollapse,
  "preserve" => WhiteSpaceCollapse::Preserve,
  "collapse" => WhiteSpaceCollapse::Collapse,
  "preserve-spaces" => WhiteSpaceCollapse::PreserveSpaces,
  "preserve-breaks" => WhiteSpaceCollapse::PreserveBreaks,
);

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

declare_enum_from_css_impl!(
  ImageScalingAlgorithm,
  "auto" => ImageScalingAlgorithm::Auto,
  "smooth" => ImageScalingAlgorithm::Smooth,
  "pixelated" => ImageScalingAlgorithm::Pixelated
);

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

declare_enum_from_css_impl!(
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
