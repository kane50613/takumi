use cosmic_text::{Align, Weight};
use merge::{Merge, option::overwrite_none};
use serde::{Deserialize, Serialize};
use taffy::{
  Dimension, Display, LengthPercentage, LengthPercentageAuto, Rect, Size,
  style::Style as TaffyStyle,
};
use ts_rs::TS;

use crate::{
  color::ColorInput,
  render::{DEFAULT_FONT_SIZE, DEFAULT_LINE_HEIGHT, RenderContext},
};

/// Represents font weight as a numeric value.
///
/// This wraps a u16 value that corresponds to CSS font-weight values
/// (e.g., 100 for thin, 400 for normal, 700 for bold, 900 for black).
#[derive(Debug, Copy, Clone, Deserialize, Serialize, TS)]
pub struct FontWeight(pub u16);

impl Default for FontWeight {
  fn default() -> Self {
    FontWeight(Weight::NORMAL.0)
  }
}

impl From<FontWeight> for Weight {
  fn from(weight: FontWeight) -> Self {
    Weight(weight.0)
  }
}

/// Defines how an image should be resized to fit its container.
///
/// Similar to CSS object-fit property.
#[derive(Debug, Clone, Deserialize, Serialize, Copy, TS)]
#[serde(rename_all = "kebab-case")]
pub enum ObjectFit {
  /// Scale the image to fit within the container while preserving aspect ratio
  Contain,
  /// Scale the image to cover the entire container while preserving aspect ratio
  Cover,
  /// Fill the container with the image, potentially distorting it
  Fill,
  /// Scale the image down to fit within the container while preserving aspect ratio, but never scale up
  ScaleDown,
  /// Don't resize the image, display it at its natural size
  None,
}

impl Default for ObjectFit {
  fn default() -> Self {
    Self::Fill
  }
}

/// Text alignment options for text rendering.
///
/// Corresponds to CSS text-align property values.
#[derive(Debug, Clone, Deserialize, Serialize, Copy, TS)]
#[serde(rename_all = "kebab-case")]
pub enum TextAlign {
  /// Align text to the left edge
  Left,
  /// Align text to the right edge
  Right,
  /// Center align text
  Center,
  /// Justify text to both edges
  Justify,
  /// Align text to the start (language-dependent)
  Start,
  /// Align text to the end (language-dependent)
  End,
}

impl From<TextAlign> for Align {
  fn from(value: TextAlign) -> Self {
    match value {
      TextAlign::Left => Align::Left,
      TextAlign::Right => Align::Right,
      TextAlign::Center => Align::Center,
      TextAlign::Justify => Align::Justified,
      TextAlign::Start => Align::Left, // Start maps to Left for LTR languages
      TextAlign::End => Align::End,
    }
  }
}

impl Default for TextAlign {
  fn default() -> Self {
    Self::Start
  }
}

/// Defines the positioning method for an element.
///
/// This enum determines how an element is positioned within its containing element.
#[derive(Debug, Clone, Deserialize, Serialize, Copy, TS)]
#[serde(rename_all = "kebab-case")]
pub enum Position {
  /// Element is positioned according to the normal flow of the document
  Static,
  /// Element is positioned according to the normal flow of the document
  Relative,
  /// Element is positioned relative to its nearest positioned ancestor
  Absolute,
  /// Element is positioned relative to the initial containing block
  Fixed,
  /// Element is positioned based on the user's scroll position
  Sticky,
}

impl Default for Position {
  fn default() -> Self {
    Self::Static
  }
}

impl From<Position> for taffy::style::Position {
  fn from(value: Position) -> Self {
    match value {
      Position::Static => taffy::style::Position::Relative,
      Position::Relative => taffy::style::Position::Relative,
      Position::Absolute => taffy::style::Position::Absolute,
      Position::Fixed => taffy::style::Position::Absolute,
      Position::Sticky => taffy::style::Position::Relative,
    }
  }
}

/// Defines the direction of flex items within a flex container.
///
/// This enum determines how flex items are laid out along the main axis.
#[derive(Debug, Clone, Deserialize, Serialize, Copy, TS)]
#[serde(rename_all = "kebab-case")]
pub enum FlexDirection {
  /// Items are laid out horizontally from left to right
  Row,
  /// Items are laid out vertically from top to bottom
  Column,
  /// Items are laid out horizontally from right to left
  RowReverse,
  /// Items are laid out vertically from bottom to top
  ColumnReverse,
}

impl Default for FlexDirection {
  fn default() -> Self {
    Self::Row
  }
}

impl From<FlexDirection> for taffy::style::FlexDirection {
  fn from(value: FlexDirection) -> Self {
    match value {
      FlexDirection::Row => taffy::style::FlexDirection::Row,
      FlexDirection::Column => taffy::style::FlexDirection::Column,
      FlexDirection::RowReverse => taffy::style::FlexDirection::RowReverse,
      FlexDirection::ColumnReverse => taffy::style::FlexDirection::ColumnReverse,
    }
  }
}

/// Defines a box shadow for an element.
///
/// This struct contains the properties for a box shadow, including color,
/// offset, blur radius, spread radius, and inset flag.
#[derive(Debug, Clone, Deserialize, Serialize, TS)]
pub struct BoxShadow {
  /// Color of the box shadow
  pub color: ColorInput,
  /// Horizontal offset of the box shadow
  pub offset_x: LengthUnit,
  /// Vertical offset of the box shadow
  pub offset_y: LengthUnit,
  /// Blur radius of the box shadow (must be non-negative)
  pub blur_radius: LengthUnit,
  /// Spread radius of the box shadow
  pub spread_radius: LengthUnit,
  /// Whether the shadow is inset (inside the element)
  #[serde(default)]
  pub inset: bool,
}

impl BoxShadow {
  pub(crate) fn resolve(self, context: &RenderContext) -> BoxShadowResolved {
    BoxShadowResolved {
      color: self.color,
      offset_x: self.offset_x.resolve_to_px(context),
      offset_y: self.offset_y.resolve_to_px(context),
      blur_radius: self.blur_radius.resolve_to_px(context),
      spread_radius: self.spread_radius.resolve_to_px(context),
    }
  }
}

pub(crate) struct BoxShadowResolved {
  pub color: ColorInput,
  pub offset_x: f32,
  pub offset_y: f32,
  pub blur_radius: f32,
  pub spread_radius: f32,
}

/// Defines a box shadow for an element.
///
/// This enum allows for flexible specification of box shadows, including
/// a single shadow or multiple shadows.
#[derive(Debug, Clone, Deserialize, Serialize, TS)]
#[serde(untagged)]
pub enum BoxShadowInput {
  /// A single box shadow
  Single(BoxShadow),
  /// Multiple box shadows
  Multiple(Vec<BoxShadow>),
}

/// Defines how flex items are aligned along the main axis.
///
/// This enum determines how space is distributed between and around flex items
/// along the main axis of the flex container.
#[derive(Debug, Clone, Deserialize, Serialize, Copy, TS)]
#[serde(rename_all = "kebab-case")]
pub enum JustifyContent {
  /// Items are packed toward the start of the writing-mode direction
  Start,
  /// Items are packed toward the end of the writing-mode direction
  End,
  /// Items are packed toward the start of the flex-direction
  FlexStart,
  /// Items are packed toward the end of the flex-direction
  FlexEnd,
  /// Items are centered along the main axis
  Center,
  /// Items are evenly distributed with the first item at the start and last at the end
  SpaceBetween,
  /// Items are evenly distributed with equal space around them
  SpaceAround,
  /// Items are evenly distributed with equal space between them
  SpaceEvenly,
}

impl From<JustifyContent> for taffy::style::JustifyContent {
  fn from(value: JustifyContent) -> Self {
    match value {
      JustifyContent::Start => taffy::style::JustifyContent::Start,
      JustifyContent::End => taffy::style::JustifyContent::End,
      JustifyContent::FlexStart => taffy::style::JustifyContent::FlexStart,
      JustifyContent::FlexEnd => taffy::style::JustifyContent::FlexEnd,
      JustifyContent::Center => taffy::style::JustifyContent::Center,
      JustifyContent::SpaceBetween => taffy::style::JustifyContent::SpaceBetween,
      JustifyContent::SpaceAround => taffy::style::JustifyContent::SpaceAround,
      JustifyContent::SpaceEvenly => taffy::style::JustifyContent::SpaceEvenly,
    }
  }
}

/// Defines how flex items are aligned along the cross axis.
///
/// This enum determines how flex items are aligned within the flex container
/// along the cross axis (perpendicular to the main axis).
#[derive(Debug, Clone, Deserialize, Serialize, Copy, TS)]
#[serde(rename_all = "kebab-case")]
pub enum AlignItems {
  /// Items are aligned to the start of the writing-mode direction
  Start,
  /// Items are aligned to the end of the writing-mode direction
  End,
  /// Items are aligned to the start of the cross axis
  FlexStart,
  /// Items are aligned to the end of the cross axis
  FlexEnd,
  /// Items are centered along the cross axis
  Center,
  /// Items are aligned such that their baselines align
  Baseline,
  /// Items are stretched to fill the container along the cross axis
  Stretch,
}

impl From<AlignItems> for taffy::style::AlignItems {
  fn from(value: AlignItems) -> Self {
    match value {
      AlignItems::Start => taffy::style::AlignItems::Start,
      AlignItems::End => taffy::style::AlignItems::End,
      AlignItems::FlexStart => taffy::style::AlignItems::FlexStart,
      AlignItems::FlexEnd => taffy::style::AlignItems::FlexEnd,
      AlignItems::Center => taffy::style::AlignItems::Center,
      AlignItems::Baseline => taffy::style::AlignItems::Baseline,
      AlignItems::Stretch => taffy::style::AlignItems::Stretch,
    }
  }
}

/// Defines how flex items should wrap.
///
/// This enum determines how flex items should wrap within the flex container.
#[derive(Debug, Clone, Deserialize, Serialize, Copy, TS)]
#[serde(rename_all = "kebab-case")]
pub enum FlexWrap {
  /// Flex items will not wrap and will shrink to fit within the container
  NoWrap,
  /// Flex items will wrap to the next line when they exceed the container width
  Wrap,
  /// Flex items will wrap to the previous line when they exceed the container width
  WrapReverse,
}

impl From<FlexWrap> for taffy::style::FlexWrap {
  fn from(value: FlexWrap) -> Self {
    match value {
      FlexWrap::NoWrap => taffy::style::FlexWrap::NoWrap,
      FlexWrap::Wrap => taffy::style::FlexWrap::Wrap,
      FlexWrap::WrapReverse => taffy::style::FlexWrap::WrapReverse,
    }
  }
}

/// Represents the resolved font style for a text node.
///
/// This struct contains the resolved font style properties after inheriting
/// from parent elements.
#[derive(Debug, Clone)]
pub struct ResolvedFontStyle {
  /// Font size in pixels for text rendering
  pub font_size: f32,
  /// Line height multiplier for text spacing
  pub line_height: f32,
  /// Font weight for text rendering
  pub font_weight: Weight,
  /// Maximum number of lines for text before truncation
  pub line_clamp: Option<u32>,
  /// Font family name for text rendering
  pub font_family: Option<String>,
  /// Letter spacing for text rendering
  pub letter_spacing: Option<f32>,
  /// Text alignment within the element
  pub text_align: Align,
  /// Text color for child text elements
  pub color: ColorInput,
}

/// Main styling structure that contains all layout and visual properties.
///
/// This structure combines both layout properties (like width, height, padding)
/// and inheritable properties (like font settings, colors) that can be passed
/// down to child elements.
#[derive(Debug, Clone, Deserialize, Serialize, TS)]
#[serde(default)]
#[ts(export, optional_fields)]
pub struct Style {
  /// Width of the element
  pub width: LengthUnit,
  /// Height of the element
  pub height: LengthUnit,
  /// Max width of the element
  pub max_width: LengthUnit,
  /// Max height of the element
  pub max_height: LengthUnit,
  /// Min width of the element
  pub min_width: LengthUnit,
  /// Min height of the element
  pub min_height: LengthUnit,
  /// Aspect ratio of the element
  pub aspect_ratio: Option<f32>,
  /// Internal spacing around the element's content
  pub padding: SidesValue<LengthUnit>,
  /// External spacing around the element
  pub margin: SidesValue<LengthUnit>,
  /// Positioning offset from the element's normal position
  pub inset: SidesValue<LengthUnit>,
  /// Direction of flex layout (row or column)
  pub flex_direction: FlexDirection,
  /// How flex items are aligned along the main axis
  pub justify_content: Option<JustifyContent>,
  /// How flex items are aligned along the cross axis
  pub align_items: Option<AlignItems>,
  /// How flex items should wrap
  pub flex_wrap: FlexWrap,
  /// The initial size of the flex item
  pub flex_basis: LengthUnit,
  /// Positioning method (relative, absolute, etc.)
  pub position: Position,
  /// Spacing between flex items
  pub gap: Gap,
  /// How much the element should grow relative to other flex items
  pub flex_grow: f32,
  /// How much the element should shrink relative to other flex items
  pub flex_shrink: f32,
  /// Width of the element's border
  pub border_width: SidesValue<LengthUnit>,
  /// How images should be fitted within their container
  pub object_fit: ObjectFit,
  /// Element's background color
  pub background_color: Option<ColorInput>,
  /// Box shadow for the element
  pub box_shadow: Option<BoxShadowInput>,
  /// Inheritable style properties
  #[serde(flatten)]
  pub inheritable_style: InheritableStyle,
}

impl Default for Style {
  fn default() -> Self {
    Self {
      margin: SidesValue::SingleValue(LengthUnit::Px(0.0)),
      width: Default::default(),
      height: Default::default(),
      max_width: Default::default(),
      max_height: Default::default(),
      min_width: Default::default(),
      min_height: Default::default(),
      aspect_ratio: None,
      padding: Default::default(),
      inset: Default::default(),
      flex_direction: Default::default(),
      justify_content: Default::default(),
      align_items: Default::default(),
      position: Default::default(),
      gap: Default::default(),
      flex_grow: 0.0,
      flex_shrink: 1.0,
      flex_basis: Default::default(),
      flex_wrap: FlexWrap::NoWrap,
      border_width: Default::default(),
      object_fit: Default::default(),
      background_color: Default::default(),
      box_shadow: Default::default(),
      inheritable_style: Default::default(),
    }
  }
}

/// Style properties that can be inherited by child elements.
///
/// These properties are typically passed down from parent to child elements
/// in the layout hierarchy, such as font settings and colors.
#[derive(Debug, Clone, Deserialize, Default, Serialize, Merge, TS)]
#[merge(strategy = overwrite_none)]
#[ts(optional_fields, export)]
pub struct InheritableStyle {
  /// Color of the element's border
  pub border_color: Option<ColorInput>,
  /// Text color for child text elements
  pub color: Option<ColorInput>,
  /// Font size in pixels for text rendering
  pub font_size: Option<LengthUnit>,
  /// Font family name for text rendering
  pub font_family: Option<String>,
  /// Line height multiplier for text spacing
  pub line_height: Option<LengthUnit>,
  /// Font weight for text rendering
  pub font_weight: Option<FontWeight>,
  /// Maximum number of lines for text before truncation
  pub line_clamp: Option<u32>,
  /// Corner radius for rounded borders in pixels
  pub border_radius: Option<SidesValue<LengthUnit>>,
  /// Text alignment within the element
  pub text_align: Option<TextAlign>,
  /// Letter spacing for text rendering
  /// Value is measured in EM units
  pub letter_spacing: Option<f32>,
}

/// Represents spacing between flex items.
///
/// Can be either a single value applied to both axes, or separate values
/// for horizontal and vertical spacing.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, TS)]
#[serde(untagged)]
pub enum Gap {
  /// Same gap value for both horizontal and vertical spacing
  SingleValue(LengthUnit),
  /// Separate values for horizontal and vertical spacing (horizontal, vertical)
  Array(LengthUnit, LengthUnit),
}

impl Default for Gap {
  fn default() -> Self {
    Self::SingleValue(LengthUnit::Px(0.0))
  }
}

impl Gap {
  /// Resolves the gap to a size in length percentages.
  ///
  /// This method converts the gap value to a size in length percentages,
  /// which can be used to set the size of flex items in a flex container.
  pub fn resolve_to_size(self, context: &RenderContext) -> Size<LengthPercentage> {
    match self {
      Gap::SingleValue(value) => Size {
        width: value.resolve_to_length_percentage(context),
        height: value.resolve_to_length_percentage(context),
      },
      Gap::Array(horizontal, vertical) => Size {
        width: horizontal.resolve_to_length_percentage(context),
        height: vertical.resolve_to_length_percentage(context),
      },
    }
  }
}

/// Border styling properties.
///
/// Defines the visual appearance of element borders, including color and size.
#[derive(Debug, Clone, Deserialize, Serialize, TS)]
#[ts(optional_fields)]
pub struct Border {
  /// Border color
  pub color: Option<ColorInput>,
  /// Border size for each side
  pub size: SidesValue<LengthUnit>,
}

/// Represents values that can be applied to all sides of an element.
///
/// This enum allows for flexible specification of values like padding, margin,
/// or border sizes using either a single value for all sides, separate values
/// for vertical/horizontal axes, or individual values for each side.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, TS)]
#[serde(untagged)]
pub enum SidesValue<T> {
  /// Same value for all four sides
  SingleValue(T),
  /// Separate values for vertical and horizontal sides (vertical, horizontal)
  AxisSidesArray(T, T),
  /// Individual values for each side (top, right, bottom, left)
  AllSides(T, T, T, T),
}

impl<T: Default> Default for SidesValue<T> {
  fn default() -> Self {
    Self::SingleValue(T::default())
  }
}

impl<T: Copy, F: Copy + Default + Into<T>> From<SidesValue<F>> for Rect<T> {
  fn from(value: SidesValue<F>) -> Self {
    match value {
      SidesValue::AllSides(top, right, bottom, left) => Rect {
        left: left.into(),
        right: right.into(),
        top: top.into(),
        bottom: bottom.into(),
      },
      SidesValue::AxisSidesArray(vertical, horizontal) => Rect {
        left: horizontal.into(),
        right: horizontal.into(),
        top: vertical.into(),
        bottom: vertical.into(),
      },
      SidesValue::SingleValue(value) => Rect {
        left: value.into(),
        right: value.into(),
        top: value.into(),
        bottom: value.into(),
      },
    }
  }
}

fn resolve_length_unit_rect_to_length_percentage(
  context: &RenderContext,
  value: Rect<LengthUnit>,
) -> Rect<LengthPercentage> {
  Rect {
    left: value.left.resolve_to_length_percentage(context),
    right: value.right.resolve_to_length_percentage(context),
    top: value.top.resolve_to_length_percentage(context),
    bottom: value.bottom.resolve_to_length_percentage(context),
  }
}

fn resolve_length_unit_rect_to_length_percentage_auto(
  context: &RenderContext,
  value: Rect<LengthUnit>,
) -> Rect<LengthPercentageAuto> {
  Rect {
    left: value.left.resolve_to_length_percentage_auto(context),
    right: value.right.resolve_to_length_percentage_auto(context),
    top: value.top.resolve_to_length_percentage_auto(context),
    bottom: value.bottom.resolve_to_length_percentage_auto(context),
  }
}

/// Represents a value that can be a specific length, percentage, or automatic.
///
/// This corresponds to CSS values that can be specified as pixels, percentages,
/// or the 'auto' keyword for automatic sizing.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Copy, TS)]
#[serde(rename_all = "camelCase")]
pub enum LengthUnit {
  /// Automatic sizing based on content
  Auto,
  /// Percentage value relative to parent container (0-100)
  Percentage(f32),
  /// Rem value relative to the root font size
  Rem(f32),
  /// Em value relative to the font size
  Em(f32),
  /// Vh value relative to the viewport height (0-100)
  Vh(f32),
  /// Vw value relative to the viewport width (0-100)
  Vw(f32),
  /// Specific pixel value
  #[serde(untagged)]
  Px(f32),
}

impl From<f32> for LengthUnit {
  fn from(value: f32) -> Self {
    Self::Px(value)
  }
}

impl Default for LengthUnit {
  fn default() -> Self {
    Self::Auto
  }
}

impl LengthUnit {
  /// Resolves the length unit to a `LengthPercentage`.
  pub fn resolve_to_length_percentage(self, context: &RenderContext) -> LengthPercentage {
    match self {
      LengthUnit::Auto => LengthPercentage::length(0.0),
      LengthUnit::Px(value) => LengthPercentage::length(value),
      LengthUnit::Percentage(value) => LengthPercentage::percent(value / 100.0),
      LengthUnit::Rem(value) => LengthPercentage::length(value * context.viewport.font_size),
      LengthUnit::Em(value) => LengthPercentage::length(value * context.parent_font_size),
      LengthUnit::Vh(value) => {
        LengthPercentage::length(context.viewport.height as f32 * value / 100.0)
      }
      LengthUnit::Vw(value) => {
        LengthPercentage::length(context.viewport.width as f32 * value / 100.0)
      }
    }
  }

  /// Resolves the length unit to a pixel value.
  pub fn resolve_to_px(self, context: &RenderContext) -> f32 {
    match self {
      LengthUnit::Auto => 0.0,
      LengthUnit::Px(value) => value,
      LengthUnit::Percentage(value) => value * context.parent_font_size / 100.0,
      LengthUnit::Rem(value) => value * context.viewport.font_size,
      LengthUnit::Em(value) => value * context.parent_font_size,
      LengthUnit::Vh(value) => value * context.viewport.height as f32 / 100.0,
      LengthUnit::Vw(value) => value * context.viewport.width as f32 / 100.0,
    }
  }

  /// Resolves the length unit to a `LengthPercentageAuto`.
  pub fn resolve_to_length_percentage_auto(self, context: &RenderContext) -> LengthPercentageAuto {
    match self {
      LengthUnit::Auto => LengthPercentageAuto::auto(),
      LengthUnit::Px(value) => LengthPercentageAuto::length(value),
      LengthUnit::Percentage(value) => LengthPercentageAuto::percent(value / 100.0),
      LengthUnit::Rem(value) => LengthPercentageAuto::length(value * context.viewport.font_size),
      LengthUnit::Em(value) => LengthPercentageAuto::length(value * context.parent_font_size),
      LengthUnit::Vh(value) => {
        LengthPercentageAuto::length(context.viewport.height as f32 * value / 100.0)
      }
      LengthUnit::Vw(value) => {
        LengthPercentageAuto::length(context.viewport.width as f32 * value / 100.0)
      }
    }
  }

  /// Resolves the length unit to a `Dimension`.
  pub fn resolve_to_dimension(self, context: &RenderContext) -> Dimension {
    match self {
      LengthUnit::Auto => Dimension::auto(),
      LengthUnit::Px(value) => Dimension::length(value),
      LengthUnit::Percentage(value) => Dimension::percent(value / 100.0),
      LengthUnit::Rem(value) => Dimension::length(value * context.viewport.font_size),
      LengthUnit::Em(value) => Dimension::length(value * context.parent_font_size),
      LengthUnit::Vh(value) => Dimension::length(context.viewport.height as f32 * value / 100.0),
      LengthUnit::Vw(value) => Dimension::length(context.viewport.width as f32 * value / 100.0),
    }
  }
}

/// Represents values for horizontal and vertical axes.
///
/// Used for properties that can have different values for horizontal
/// and vertical directions, such as padding or margin.
#[derive(Debug, Clone, Deserialize, Serialize, TS)]
pub struct AxisSides<T> {
  /// Horizontal axis value
  #[serde(default)]
  pub horizontal: T,
  /// Vertical axis value
  #[serde(default)]
  pub vertical: T,
}

impl Style {
  /// Resolves the style to a `TaffyStyle`.
  pub fn resolve_to_taffy_style(&self, context: &RenderContext) -> TaffyStyle {
    TaffyStyle {
      size: Size {
        width: self.width.resolve_to_dimension(context),
        height: self.height.resolve_to_dimension(context),
      },
      border: resolve_length_unit_rect_to_length_percentage(context, self.border_width.into()),
      padding: resolve_length_unit_rect_to_length_percentage(context, self.padding.into()),
      inset: resolve_length_unit_rect_to_length_percentage_auto(context, self.inset.into()),
      margin: resolve_length_unit_rect_to_length_percentage_auto(context, self.margin.into()),
      display: Display::Flex,
      flex_direction: self.flex_direction.into(),
      position: self.position.into(),
      justify_content: self.justify_content.map(|j| j.into()),
      flex_grow: self.flex_grow,
      align_items: self.align_items.map(|a| a.into()),
      gap: self.gap.resolve_to_size(context),
      flex_basis: self.flex_basis.resolve_to_dimension(context),
      flex_shrink: self.flex_shrink,
      flex_wrap: self.flex_wrap.into(),
      min_size: Size {
        width: self.min_width.resolve_to_dimension(context),
        height: self.min_height.resolve_to_dimension(context),
      },
      max_size: Size {
        width: self.max_width.resolve_to_dimension(context),
        height: self.max_height.resolve_to_dimension(context),
      },
      aspect_ratio: self.aspect_ratio,
      ..Default::default()
    }
  }

  /// Resolves the style to a `ResolvedFontStyle`.
  pub fn resolve_to_font_style(&self, context: &RenderContext) -> ResolvedFontStyle {
    ResolvedFontStyle {
      color: self.inheritable_style.color.clone().unwrap_or_default(),
      font_size: self
        .inheritable_style
        .font_size
        .map(|f| f.resolve_to_px(context))
        .unwrap_or(DEFAULT_FONT_SIZE),
      line_height: self
        .inheritable_style
        .line_height
        .map(|f| f.resolve_to_px(context))
        .unwrap_or(DEFAULT_LINE_HEIGHT),
      font_weight: self
        .inheritable_style
        .font_weight
        .unwrap_or_default()
        .into(),
      line_clamp: self.inheritable_style.line_clamp,
      font_family: self.inheritable_style.font_family.clone(),
      letter_spacing: self.inheritable_style.letter_spacing,
      text_align: self.inheritable_style.text_align.unwrap_or_default().into(),
    }
  }
}
