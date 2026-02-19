//! Style properties and related types for the takumi styling system.
//!
//! This module contains CSS-like properties including layout properties,
//! typography settings, positioning, and visual effects.

mod aspect_ratio;
mod background;
mod background_image;
mod background_position;
mod background_repeat;
mod background_size;
mod blend_mode;
mod border;
mod box_shadow;
mod clip_path;
mod color;
mod conic_gradient;
mod filter;
mod flex;
mod flex_grow;
mod font_feature_settings;
mod font_stretch;
mod font_style;
mod font_synthesis;
mod font_variation_settings;
mod font_weight;
mod gradient_utils;
mod grid;
mod length;
mod line_clamp;
mod line_height;
mod linear_gradient;
mod noise_v1;
mod overflow;
mod overflow_wrap;
mod percentage_number;
mod radial_gradient;
mod sides;
mod space_pair;
mod text_decoration;
mod text_overflow;
mod text_shadow;
mod text_stroke;
mod text_wrap;
mod transform;
mod vertical_align;
mod white_space;
mod word_break;

pub use aspect_ratio::*;
pub use background::*;
pub use background_image::*;
pub use background_position::*;
pub use background_repeat::*;
pub use background_size::*;
pub use blend_mode::*;
pub use border::*;
pub use box_shadow::*;
pub use clip_path::*;
pub use color::*;
pub use conic_gradient::*;
pub use filter::*;
pub use flex::*;
pub use flex_grow::*;
pub use font_feature_settings::*;
pub use font_stretch::*;
pub use font_style::*;
pub use font_synthesis::*;
pub use font_variation_settings::*;
pub use font_weight::*;
pub use grid::*;
pub use length::*;
pub use line_clamp::*;
pub use line_height::*;
pub use linear_gradient::*;
pub use noise_v1::*;
pub use overflow::*;
pub use overflow_wrap::*;
pub use percentage_number::*;
pub use radial_gradient::*;
pub use sides::*;
pub use space_pair::*;
pub use text_decoration::*;
pub use text_overflow::*;
pub use text_shadow::*;
pub use text_stroke::*;
pub use text_wrap::*;
pub use transform::*;
pub use vertical_align::*;
pub use white_space::*;
pub use word_break::*;

use cssparser::{
  ParseError, ParseErrorKind, Parser, ParserInput, SourceLocation, ToCss, Token,
  match_ignore_ascii_case,
};
use fast_image_resize::ResizeAlg;
use image::imageops::FilterType;
use parley::{Alignment, FontStack};
use std::borrow::Cow;
use zeno::Join;

use crate::layout::style::tw::TailwindPropertyParser;
use crate::rendering::Sizing;

/// Parser result type alias for CSS property parsers.
pub type ParseResult<'i, T> = Result<T, ParseError<'i, Cow<'i, str>>>;

/// Enum representing CSS tokens.
pub enum CssToken {
  /// A CSS keyword.
  Keyword(&'static str),
  /// A CSS token without the < and > wrappers.
  Token(&'static str),
}

impl std::fmt::Display for CssToken {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      CssToken::Keyword(keyword) => write!(f, "'{}'", keyword),
      CssToken::Token(token) => write!(f, "<{}>", token),
    }
  }
}

/// Trait for types that can be parsed from CSS.
pub trait FromCss<'i> {
  /// Parses the type from a [`Parser`] instance.
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self>
  where
    Self: Sized;

  /// Helper function to parse the type from a string.
  fn from_str(source: &'i str) -> ParseResult<'i, Self>
  where
    Self: Sized,
  {
    let mut input = ParserInput::new(source);
    let mut parser = Parser::new(&mut input);

    Self::from_css(&mut parser)
  }

  /// Returns the list of valid CSS tokens for this type.
  fn valid_tokens() -> &'static [CssToken];

  /// Returns a message to be used in error messages.
  fn expect_message() -> Cow<'static, str> {
    Cow::Owned(format!(
      "a value of {}",
      merge_enum_values(Self::valid_tokens())
    ))
  }

  /// Creates a parse error for an unexpected token.
  fn unexpected_token_error(
    location: SourceLocation,
    token: &Token,
  ) -> ParseError<'i, Cow<'i, str>> {
    #[cfg(feature = "detailed_css_error")]
    {
      create_unexpected_token_error(location, token, Self::expect_message())
    }
    #[cfg(not(feature = "detailed_css_error"))]
    {
      create_unexpected_token_error(location, token)
    }
  }
}

/// Converts a parsed/inherited value into a computed value for the current node context.
pub(crate) trait MakeComputed {
  /// Default no-op for types that do not need computed-value normalization.
  fn make_computed(&mut self, _sizing: &Sizing) {}
}

impl<T: MakeComputed> MakeComputed for Option<T> {
  fn make_computed(&mut self, sizing: &Sizing) {
    if let Some(value) = self.as_mut() {
      value.make_computed(sizing);
    }
  }
}

impl<T: MakeComputed> MakeComputed for Box<[T]> {
  fn make_computed(&mut self, sizing: &Sizing) {
    for value in self.iter_mut() {
      value.make_computed(sizing);
    }
  }
}

impl<T: MakeComputed> MakeComputed for Vec<T> {
  fn make_computed(&mut self, sizing: &Sizing) {
    for value in self.iter_mut() {
      value.make_computed(sizing);
    }
  }
}

fn create_unexpected_token_error<'i>(
  location: SourceLocation,
  token: &Token,
  #[cfg(feature = "detailed_css_error")] expect_message: Cow<'static, str>,
) -> ParseError<'i, Cow<'i, str>> {
  #[cfg(feature = "detailed_css_error")]
  let message = format!(
    "unexpected token: {}, {}.",
    token.to_css_string(),
    expect_message
  );
  #[cfg(not(feature = "detailed_css_error"))]
  let message = format!("unexpected token: {}.", token.to_css_string());

  ParseError {
    location,
    kind: ParseErrorKind::Custom(Cow::Owned(message)),
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

/// Macro to implement From trait for Taffy enum conversions.
macro_rules! impl_from_taffy_enum {
  ($from_ty:ty, $to_ty:ty, $($variant:ident),*) => {
    impl From<$from_ty> for $to_ty {
      fn from(value: $from_ty) -> Self {
        match value {
          $(<$from_ty>::$variant => <$to_ty>::$variant,)*
        }
      }
    }
  };
}

/// Declares a CSS enum parser with automatic value list generation.
macro_rules! declare_enum_from_css_impl {
  (
    $enum_type:ty,
    $($css_value:expr => $variant:expr),* $(,)?
  ) => {
    impl crate::layout::style::MakeComputed for $enum_type {}

    impl<'i> crate::layout::style::FromCss<'i> for $enum_type {
      fn valid_tokens() -> &'static [crate::layout::style::CssToken] {
        &[$(crate::layout::style::CssToken::Keyword($css_value)),*]
      }

      fn from_css(input: &mut cssparser::Parser<'i, '_>) -> crate::layout::style::ParseResult<'i, Self> {
        let location = input.current_source_location();
        let token = input.next()?;

        let cssparser::Token::Ident(ident) = token else {
          return Err(Self::unexpected_token_error(location, &token));
        };

        cssparser::match_ignore_ascii_case! {&ident,
          $(
            $css_value => Ok($variant),
          )*
          _ => Err(Self::unexpected_token_error(location, &token)),
        }
      }
    }
  };
}

pub(crate) use declare_enum_from_css_impl;

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

impl TailwindPropertyParser for ObjectFit {
  fn parse_tw(token: &str) -> Option<Self> {
    Self::from_str(token).ok()
  }
}

/// Defines how the background is clipped.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
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

/// Represents the CSS `border-radius` property, supporting elliptical corners.
///
/// Each corner has independent horizontal and vertical radii, allowing for both circular and elliptical shapes.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct BorderRadius(pub Sides<SpacePair<Length<false>>>);

impl MakeComputed for BorderRadius {
  fn make_computed(&mut self, sizing: &Sizing) {
    self.0.make_computed(sizing);
  }
}

impl<'i> FromCss<'i> for BorderRadius {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    let widths: Sides<Length<false>> = Sides::from_css(input)?;

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

  fn expect_message() -> Cow<'static, str> {
    "1 to 4 length values for width, optionally followed by '/' and 1 to 4 length values for height"
      .into()
  }

  fn valid_tokens() -> &'static [CssToken] {
    &[CssToken::Token("length")]
  }
}

/// Defines how the width and height of an element are calculated.
///
/// This enum determines whether the width and height properties include padding and border, or just the content area.
#[derive(Default, Debug, Clone, Copy, PartialEq)]
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

impl_from_taffy_enum!(BoxSizing, taffy::BoxSizing, ContentBox, BorderBox);

/// Text alignment options for text rendering.
///
/// Corresponds to CSS text-align property values.
#[derive(Default, Debug, Clone, Copy, PartialEq)]
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

impl TailwindPropertyParser for TextAlign {
  fn parse_tw(token: &str) -> Option<Self> {
    Self::from_str(token).ok()
  }
}

impl_from_taffy_enum!(
  TextAlign, Alignment, Left, Right, Center, Justify, Start, End
);

/// Defines whether an element creates a new stacking context.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
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

impl From<LineJoin> for Join {
  fn from(value: LineJoin) -> Self {
    match value {
      LineJoin::Miter => Join::Miter,
      LineJoin::Round => Join::Round,
      LineJoin::Bevel => Join::Bevel,
    }
  }
}

impl TailwindPropertyParser for LineJoin {
  fn parse_tw(token: &str) -> Option<Self> {
    Self::from_str(token).ok()
  }
}

/// Defines the positioning method for an element.
///
/// This enum determines how an element is positioned within its containing element.
#[derive(Default, Debug, Clone, Copy, PartialEq)]
pub enum Position {
  /// The element is positioned according to the normal flow of the document.
  /// Offsets (top, right, bottom, left) have no effect.
  #[default]
  Relative,
  /// The element is removed from the normal document flow and positioned relative to its nearest positioned ancestor.
  /// Offsets (top, right, bottom, left) specify the distance from the ancestor.
  Absolute,
}

declare_enum_from_css_impl!(
  Position,
  "relative" => Position::Relative,
  "absolute" => Position::Absolute
);

impl_from_taffy_enum!(Position, taffy::Position, Relative, Absolute);

/// Defines the direction of flex items within a flex container.
///
/// This enum determines how flex items are laid out along the main axis.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
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
  taffy::FlexDirection,
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
}

declare_enum_from_css_impl!(
  JustifyContent,
  "normal" => JustifyContent::Normal,
  "start" => JustifyContent::Start,
  "end" => JustifyContent::End,
  "flex-start" => JustifyContent::FlexStart,
  "flex-end" => JustifyContent::FlexEnd,
  "center" => JustifyContent::Center,
  "stretch" => JustifyContent::Stretch,
  "space-between" => JustifyContent::SpaceBetween,
  "space-around" => JustifyContent::SpaceAround,
  "space-evenly" => JustifyContent::SpaceEvenly
);

impl TailwindPropertyParser for JustifyContent {
  fn parse_tw(token: &str) -> Option<Self> {
    match token {
      "between" => Some(JustifyContent::SpaceBetween),
      "around" => Some(JustifyContent::SpaceAround),
      "evenly" => Some(JustifyContent::SpaceEvenly),
      _ => Self::from_str(token).ok(),
    }
  }
}

impl From<JustifyContent> for Option<taffy::JustifyContent> {
  fn from(value: JustifyContent) -> Self {
    match value {
      JustifyContent::Normal => None,
      JustifyContent::Start => Some(taffy::JustifyContent::Start),
      JustifyContent::End => Some(taffy::JustifyContent::End),
      JustifyContent::FlexStart => Some(taffy::JustifyContent::FlexStart),
      JustifyContent::FlexEnd => Some(taffy::JustifyContent::FlexEnd),
      JustifyContent::Center => Some(taffy::JustifyContent::Center),
      JustifyContent::Stretch => Some(taffy::JustifyContent::Stretch),
      JustifyContent::SpaceBetween => Some(taffy::JustifyContent::SpaceBetween),
      JustifyContent::SpaceAround => Some(taffy::JustifyContent::SpaceAround),
      JustifyContent::SpaceEvenly => Some(taffy::JustifyContent::SpaceEvenly),
    }
  }
}

/// This enum determines the layout algorithm used for the children of a node.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum Display {
  /// The element is not displayed
  None,
  /// The element generates a flex container and its children follow the flexbox layout algorithm
  #[default]
  Flex,
  /// The element generates an inline-level flex container
  InlineFlex,
  /// The element generates a grid container and its children follow the CSS Grid layout algorithm
  Grid,
  /// The element generates an inline-level grid container
  InlineGrid,
  /// The element generates an inline container and its children follow the inline layout algorithm
  Inline,
  /// The element creates a block container and its children follow the block layout algorithm
  Block,
  /// The element generates an inline-level block container
  InlineBlock,
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
  "inline-block" => Display::InlineBlock
);

impl Display {
  /// Returns true if the display creates an inline formatting context.
  pub fn is_inline(&self) -> bool {
    *self == Display::Inline
  }

  /// Returns true if the display participates in the inline flow as an atomic box.
  pub fn is_inline_level(&self) -> bool {
    matches!(
      self,
      Display::Inline | Display::InlineBlock | Display::InlineFlex | Display::InlineGrid
    )
  }

  /// Returns true if the display makes the children blockified (e.g., flex or grid).
  pub fn should_blockify_children(&self) -> bool {
    matches!(
      self,
      Display::Flex | Display::InlineFlex | Display::Grid | Display::InlineGrid
    )
  }

  /// Cast the display to block level.
  pub fn as_blockified(self) -> Self {
    match self {
      Display::Inline => Display::Block,
      Display::InlineBlock => Display::Block,
      Display::InlineFlex => Display::Flex,
      Display::InlineGrid => Display::Grid,
      _ => self,
    }
  }

  /// Mutate the display to be block level.
  pub fn blockify(&mut self) {
    *self = self.as_blockified();
  }
}

impl From<Display> for taffy::Display {
  fn from(value: Display) -> Self {
    match value {
      Display::Flex => taffy::Display::Flex,
      Display::InlineFlex => taffy::Display::Flex,
      Display::Grid => taffy::Display::Grid,
      Display::InlineGrid => taffy::Display::Grid,
      Display::Block => taffy::Display::Block,
      Display::InlineBlock => taffy::Display::Block,
      Display::None => taffy::Display::None,
      Display::Inline => unreachable!("Inline node should not be inserted into taffy context"),
    }
  }
}

/// Defines how flex items are aligned along the cross axis.
///
/// This enum determines how items are aligned within the flex container
/// along the cross axis (perpendicular to the main axis).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
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
}

declare_enum_from_css_impl!(
  AlignItems,
  "normal" => AlignItems::Normal,
  "start" => AlignItems::Start,
  "end" => AlignItems::End,
  "flex-start" => AlignItems::FlexStart,
  "flex-end" => AlignItems::FlexEnd,
  "center" => AlignItems::Center,
  "baseline" => AlignItems::Baseline,
  "stretch" => AlignItems::Stretch
);

impl TailwindPropertyParser for AlignItems {
  fn parse_tw(token: &str) -> Option<Self> {
    Self::from_str(token).ok()
  }
}

impl From<AlignItems> for Option<taffy::AlignItems> {
  fn from(value: AlignItems) -> Self {
    match value {
      AlignItems::Normal => None,
      AlignItems::Start => Some(taffy::AlignItems::Start),
      AlignItems::End => Some(taffy::AlignItems::End),
      AlignItems::FlexStart => Some(taffy::AlignItems::FlexStart),
      AlignItems::FlexEnd => Some(taffy::AlignItems::FlexEnd),
      AlignItems::Center => Some(taffy::AlignItems::Center),
      AlignItems::Baseline => Some(taffy::AlignItems::Baseline),
      AlignItems::Stretch => Some(taffy::AlignItems::Stretch),
    }
  }
}

/// Defines how flex items should wrap.
///
/// This enum determines how flex items should wrap within the flex container.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
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

impl_from_taffy_enum!(FlexWrap, taffy::FlexWrap, NoWrap, Wrap, WrapReverse);

/// Controls text case transformation when rendering.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
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

/// Represents a font family for text rendering.
/// Multi value fallback is supported.
#[derive(Debug, Clone, PartialEq)]
pub struct FontFamily(String);

impl MakeComputed for FontFamily {}

impl<'i> FromCss<'i> for FontFamily {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    Ok(FontFamily(input.current_line().to_string()))
  }

  fn from_str(source: &'i str) -> ParseResult<'i, Self> {
    Ok(FontFamily(source.to_string()))
  }

  fn valid_tokens() -> &'static [CssToken] {
    &[
      CssToken::Token("family-name"),
      CssToken::Token("generic-name"),
    ]
  }
}

impl TailwindPropertyParser for FontFamily {
  fn parse_tw(token: &str) -> Option<Self> {
    match_ignore_ascii_case! {token,
      "sans" => Some(FontFamily("sans-serif".to_string())),
      "serif" => Some(FontFamily("serif".to_string())),
      "mono" => Some(FontFamily("monospace".to_string())),
      _ => None,
    }
  }
}

impl Default for FontFamily {
  fn default() -> Self {
    Self("sans-serif".to_string())
  }
}

impl<'a> From<FontFamily> for FontStack<'a> {
  fn from(family: FontFamily) -> Self {
    FontStack::Source(family.0.into())
  }
}

impl<'a> From<&'a FontFamily> for FontStack<'a> {
  fn from(family: &'a FontFamily) -> Self {
    FontStack::Source(family.0.as_str().into())
  }
}

impl From<&str> for FontFamily {
  fn from(family: &str) -> Self {
    FontFamily(family.to_string())
  }
}

/// Controls how whitespace should be collapsed.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
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
  /// Solid border style.
  Solid,
}

declare_enum_from_css_impl!(
  BorderStyle,
  "none" => BorderStyle::None,
  "solid" => BorderStyle::Solid,
);

impl TailwindPropertyParser for BorderStyle {
  fn parse_tw(token: &str) -> Option<Self> {
    Self::from_str(token).ok()
  }
}

impl From<ImageScalingAlgorithm> for FilterType {
  fn from(algorithm: ImageScalingAlgorithm) -> Self {
    match algorithm {
      ImageScalingAlgorithm::Auto => FilterType::CatmullRom,
      ImageScalingAlgorithm::Smooth => FilterType::Lanczos3,
      ImageScalingAlgorithm::Pixelated => FilterType::Nearest,
    }
  }
}

impl From<ImageScalingAlgorithm> for ResizeAlg {
  fn from(algorithm: ImageScalingAlgorithm) -> Self {
    match algorithm {
      ImageScalingAlgorithm::Auto => {
        ResizeAlg::Convolution(fast_image_resize::FilterType::CatmullRom)
      }
      ImageScalingAlgorithm::Smooth => {
        ResizeAlg::Convolution(fast_image_resize::FilterType::Lanczos3)
      }
      ImageScalingAlgorithm::Pixelated => ResizeAlg::Nearest,
    }
  }
}
