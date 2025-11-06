//! Style properties and related types for the takumi styling system.
//!
//! This module contains CSS-like properties including layout properties,
//! typography settings, positioning, and visual effects.

mod aspect_ratio;
mod background_image;
mod background_position;
mod background_repeat;
mod background_size;
mod border;
mod box_shadow;
mod clip_path;
mod color;
mod filter;
mod flex;
mod flex_grow;
mod font_feature_settings;
mod font_style;
mod font_variation_settings;
mod font_weight;
mod gradient_utils;
mod grid;
mod length_unit;
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
mod transform;
mod white_space;
mod word_break;

use std::borrow::Cow;

pub use aspect_ratio::*;
pub use background_image::*;
pub use background_position::*;
pub use background_repeat::*;
pub use background_size::*;
pub use border::*;
pub use box_shadow::*;
pub use clip_path::*;
pub use color::*;
pub use filter::*;
pub use flex::*;
pub use flex_grow::*;
pub use font_feature_settings::*;
pub use font_style::*;
pub use font_variation_settings::*;
pub use font_weight::*;
pub use grid::*;
pub use length_unit::*;
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
pub use transform::*;
pub use white_space::*;
pub use word_break::*;

use cssparser::{ParseError, Parser, ParserInput, Token, match_ignore_ascii_case};
use image::imageops::FilterType;
use parley::{Alignment, FontStack};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::layout::style::tw::TailwindPropertyParser;

/// Parser result type alias for CSS property parsers.
pub type ParseResult<'i, T> = Result<T, ParseError<'i, Cow<'i, str>>>;

/// Trait for types that can be deserialized from CSS.
pub trait FromCss<'i> {
  /// Deserializes the type from a [`Parser`] instance.
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self>
  where
    Self: Sized;

  /// Helper function to deserialize the type from a string.
  fn from_str(source: &'i str) -> ParseResult<'i, Self>
  where
    Self: Sized,
  {
    let mut input = ParserInput::new(source);
    let mut parser = Parser::new(&mut input);

    Self::from_css(&mut parser)
  }
}

/// Macro to implement From trait for Taffy enum conversions
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

/// Defines how an image should be resized to fit its container.
///
/// Similar to CSS object-fit property.
#[derive(Debug, Clone, Deserialize, Serialize, Copy, TS, PartialEq, Default)]
#[serde(rename_all = "kebab-case")]
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

impl<'i> FromCss<'i> for ObjectFit {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    let location = input.current_source_location();
    let token = input.expect_ident()?;

    match_ignore_ascii_case! { token,
      "fill" => Ok(ObjectFit::Fill),
      "contain" => Ok(ObjectFit::Contain),
      "cover" => Ok(ObjectFit::Cover),
      "scale-down" => Ok(ObjectFit::ScaleDown),
      "none" => Ok(ObjectFit::None),
      _ => Err(location.new_unexpected_token_error(Token::Ident(token.clone()))),
    }
  }
}

impl TailwindPropertyParser for ObjectFit {
  fn parse_tw(token: &str) -> Option<Self> {
    Self::from_str(token).ok()
  }
}

/// Defines how the width and height of an element are calculated.
///
/// This enum determines whether the width and height properties include padding and border, or just the content area.
#[derive(Default, Debug, Clone, Deserialize, Serialize, Copy, TS, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum BoxSizing {
  /// The width and height properties include padding and border, but not the content area
  ContentBox,
  /// The width and height properties include the content area, but not padding and border
  #[default]
  BorderBox,
}

impl_from_taffy_enum!(BoxSizing, taffy::BoxSizing, ContentBox, BorderBox);

/// Text alignment options for text rendering.
///
/// Corresponds to CSS text-align property values.
#[derive(Default, Debug, Clone, Deserialize, Serialize, Copy, TS, PartialEq)]
#[serde(rename_all = "kebab-case")]
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

impl<'i> FromCss<'i> for TextAlign {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    let location = input.current_source_location();
    let token = input.expect_ident()?;

    match_ignore_ascii_case! { token,
      "left" => Ok(TextAlign::Left),
      "right" => Ok(TextAlign::Right),
      "center" => Ok(TextAlign::Center),
      "justify" => Ok(TextAlign::Justify),
      "start" => Ok(TextAlign::Start),
      "end" => Ok(TextAlign::End),
      _ => Err(location.new_unexpected_token_error(Token::Ident(token.clone()))),
    }
  }
}

impl TailwindPropertyParser for TextAlign {
  fn parse_tw(token: &str) -> Option<Self> {
    Self::from_str(token).ok()
  }
}

impl_from_taffy_enum!(
  TextAlign, Alignment, Left, Right, Center, Justify, Start, End
);

/// Defines the positioning method for an element.
///
/// This enum determines how an element is positioned within its containing element.
#[derive(Default, Debug, Clone, Deserialize, Serialize, Copy, TS, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum Position {
  /// The element is positioned according to the normal flow of the document.
  /// Offsets (top, right, bottom, left) have no effect.
  #[default]
  Relative,
  /// The element is removed from the normal document flow and positioned relative to its nearest positioned ancestor.
  /// Offsets (top, right, bottom, left) specify the distance from the ancestor.
  Absolute,
}

impl_from_taffy_enum!(Position, taffy::Position, Relative, Absolute);

/// Defines the direction of flex items within a flex container.
///
/// This enum determines how flex items are laid out along the main axis.
#[derive(Default, Debug, Clone, Deserialize, Serialize, Copy, TS, PartialEq)]
#[serde(rename_all = "kebab-case")]
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
#[derive(Debug, Default, Clone, Deserialize, Serialize, Copy, TS, PartialEq)]
#[serde(rename_all = "kebab-case")]
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

impl<'i> FromCss<'i> for JustifyContent {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    let location = input.current_source_location();
    let token = input.expect_ident()?;

    match_ignore_ascii_case! { token,
      "normal" => Ok(JustifyContent::Normal),
      "start" => Ok(JustifyContent::Start),
      "end" => Ok(JustifyContent::End),
      "flex-start" => Ok(JustifyContent::FlexStart),
      "flex-end" => Ok(JustifyContent::FlexEnd),
      "center" => Ok(JustifyContent::Center),
      "stretch" => Ok(JustifyContent::Stretch),
      "space-between" => Ok(JustifyContent::SpaceBetween),
      "space-around" => Ok(JustifyContent::SpaceAround),
      "space-evenly" => Ok(JustifyContent::SpaceEvenly),
      _ => Err(location.new_unexpected_token_error(Token::Ident(token.clone()))),
    }
  }
}

impl TailwindPropertyParser for JustifyContent {
  fn parse_tw(token: &str) -> Option<Self> {
    Self::from_str(token).ok()
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
#[derive(Debug, Clone, Deserialize, Serialize, Copy, TS, PartialEq, Default)]
#[serde(rename_all = "kebab-case")]
pub enum Display {
  /// The element is not displayed
  None,
  /// The element generates a flex container and its children follow the flexbox layout algorithm
  #[default]
  Flex,
  /// The element generates a grid container and its children follow the CSS Grid layout algorithm
  Grid,
  /// The element generates an inline container and its children follow the inline layout algorithm
  Inline,
  /// The element creates a block container and its children follow the block layout algorithm
  Block,
}

impl Display {
  /// Returns true if the display is inline.
  pub fn is_inline(&self) -> bool {
    *self == Display::Inline
  }

  /// Returns true if the display makes the children blockified (e.g., flex or grid).
  pub fn should_blockify_children(&self) -> bool {
    matches!(self, Display::Flex | Display::Grid)
  }

  /// Cast the display to block level.
  pub fn as_blockified(self) -> Self {
    match self {
      Display::Inline => Display::Block,
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
      Display::Grid => taffy::Display::Grid,
      Display::Block => taffy::Display::Block,
      Display::None => taffy::Display::None,
      Display::Inline => unreachable!("Inline node should not be inserted into taffy context"),
    }
  }
}

/// Defines how flex items are aligned along the cross axis.
///
/// This enum determines how items are aligned within the flex container
/// along the cross axis (perpendicular to the main axis).
#[derive(Debug, Clone, Default, Deserialize, Serialize, Copy, TS, PartialEq)]
#[serde(rename_all = "kebab-case")]
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

impl<'i> FromCss<'i> for AlignItems {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    let location = input.current_source_location();
    let token = input.expect_ident()?;

    match_ignore_ascii_case! {token,
      "normal" => Ok(AlignItems::Normal),
      "start" => Ok(AlignItems::Start),
      "end" => Ok(AlignItems::End),
      "flex-start" => Ok(AlignItems::FlexStart),
      "flex-end" => Ok(AlignItems::FlexEnd),
      "center" => Ok(AlignItems::Center),
      "baseline" => Ok(AlignItems::Baseline),
      "stretch" => Ok(AlignItems::Stretch),
      _ => Err(location.new_unexpected_token_error(Token::Ident(token.clone()))),
    }
  }
}

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
#[derive(Debug, Clone, Deserialize, Serialize, Copy, TS, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum FlexWrap {
  /// Flex items will all be displayed in a single line, shrinking as needed
  #[serde(rename = "nowrap")]
  NoWrap,
  /// Flex items will wrap onto multiple lines, with new lines stacking in the flex direction
  Wrap,
  /// Flex items will wrap onto multiple lines, with new lines stacking in the reverse flex direction
  WrapReverse,
}

impl_from_taffy_enum!(FlexWrap, taffy::FlexWrap, NoWrap, Wrap, WrapReverse);

/// Controls text case transformation when rendering.
#[derive(Default, Debug, Clone, Deserialize, Serialize, Copy, TS, PartialEq)]
#[serde(rename_all = "kebab-case")]
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

/// Represents a font family for text rendering.
/// Multi value fallback is supported.
#[derive(Debug, Clone, Deserialize, Serialize, TS, PartialEq)]
#[serde(transparent)]
pub struct FontFamily(String);

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

impl<'i> FromCss<'i> for FontFamily {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    Ok(FontFamily(input.current_line().to_string()))
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

/// Controls whether text should be wrapped.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, TS, PartialEq, Default)]
#[serde(rename_all = "lowercase")]
pub enum TextWrapMode {
  /// Text is wrapped across lines at appropriate characters to minimize overflow.
  #[default]
  Wrap,
  /// Text does not wrap across lines. It will overflow its containing element rather than breaking onto a new line.
  NoWrap,
}

impl<'i> FromCss<'i> for TextWrapMode {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    let ident = input.expect_ident()?;
    match_ignore_ascii_case! {ident,
      "wrap" => Ok(TextWrapMode::Wrap),
      "nowrap" => Ok(TextWrapMode::NoWrap),
      _ => {
        let token = Token::Ident(ident.clone());
        Err(input.new_basic_unexpected_token_error(token).into())
      }
    }
  }
}

/// Controls how whitespace should be collapsed.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, TS, PartialEq, Default)]
#[serde(rename_all = "kebab-case")]
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

impl<'i> FromCss<'i> for WhiteSpaceCollapse {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    let ident = input.expect_ident()?;
    match_ignore_ascii_case! {&ident,
      "preserve" => Ok(WhiteSpaceCollapse::Preserve),
      "collapse" => Ok(WhiteSpaceCollapse::Collapse),
      "preserve-spaces" => Ok(WhiteSpaceCollapse::PreserveSpaces),
      "preserve-breaks" => Ok(WhiteSpaceCollapse::PreserveBreaks),
      _ => {
        let token = Token::Ident(ident.clone());
        Err(input.new_basic_unexpected_token_error(token).into())
      }
    }
  }
}

/// Defines how images should be scaled when rendered.
#[derive(Default, Debug, Clone, Copy, Deserialize, Serialize, TS, PartialEq)]
#[serde(rename_all = "kebab-case")]
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

impl From<ImageScalingAlgorithm> for FilterType {
  fn from(algorithm: ImageScalingAlgorithm) -> Self {
    match algorithm {
      ImageScalingAlgorithm::Auto => FilterType::CatmullRom,
      ImageScalingAlgorithm::Smooth => FilterType::Lanczos3,
      ImageScalingAlgorithm::Pixelated => FilterType::Nearest,
    }
  }
}
