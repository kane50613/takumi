use cosmic_text::{Align, FamilyOwned, Weight};
use merge::{Merge, option::overwrite_none};
use serde::{Deserialize, Serialize};
use taffy::{Layout, Size, Style as TaffyStyle};
use ts_rs::TS;

use crate::{
  layout::{DEFAULT_FONT_SIZE, DEFAULT_LINE_HEIGHT_SCALER, style::properties::*},
  rendering::{BorderRadius, RenderContext},
};

/// Represents the resolved font style for a text node.
#[derive(Debug, Clone)]
pub struct FontStyle {
  /// Font size in pixels for text rendering.
  pub font_size: f32,
  /// Line height as an absolute value in pixels.
  pub line_height: f32,
  /// Font weight for text rendering.
  pub font_weight: Weight,
  /// Font slant style (normal, italic, oblique).
  pub text_style: TextStyle,
  /// Maximum number of lines for text before truncation.
  pub line_clamp: Option<u32>,
  /// Font family for text rendering.
  pub font_family: Option<FamilyOwned>,
  /// Letter spacing for text rendering in em units (relative to font size).
  pub letter_spacing: Option<f32>,
  /// Text alignment within the element.
  pub text_align: Option<Align>,
  /// How text should be overflowed.
  pub text_overflow: TextOverflow,
  /// Text transform behavior (uppercase, lowercase, capitalize, none)
  pub text_transform: TextTransform,
  /// Text color for child text elements.
  pub color: LinearGradientOrColor,
}

/// Main styling structure that contains all layout and visual properties.
#[derive(Debug, Clone, Deserialize, Serialize, TS)]
#[serde(default, rename_all = "camelCase")]
#[ts(export, optional_fields)]
pub struct Style {
  /// Display algorithm to use for the element.
  pub display: Display,
  /// Width of the element.
  pub width: LengthUnit,
  /// Height of the element.
  pub height: LengthUnit,
  /// Max width of the element.
  pub max_width: LengthUnit,
  /// Max height of the element.
  pub max_height: LengthUnit,
  /// Min width of the element.
  pub min_width: LengthUnit,
  /// Min height of the element.
  pub min_height: LengthUnit,
  /// Aspect ratio of the element (width/height).
  pub aspect_ratio: Option<f32>,
  /// Internal spacing around the element's content (top, right, bottom, left).
  pub padding: Sides<LengthUnit>,
  /// Longhand: top padding. Overrides `padding` top value.
  pub padding_top: Option<LengthUnit>,
  /// Longhand: right padding. Overrides `padding` right value.
  pub padding_right: Option<LengthUnit>,
  /// Longhand: bottom padding. Overrides `padding` bottom value.
  pub padding_bottom: Option<LengthUnit>,
  /// Longhand: left padding. Overrides `padding` left value.
  pub padding_left: Option<LengthUnit>,
  /// External spacing around the element (top, right, bottom, left).
  pub margin: Sides<LengthUnit>,
  /// Longhand: top margin. Overrides `margin` top value.
  pub margin_top: Option<LengthUnit>,
  /// Longhand: right margin. Overrides `margin` right value.
  pub margin_right: Option<LengthUnit>,
  /// Longhand: bottom margin. Overrides `margin` bottom value.
  pub margin_bottom: Option<LengthUnit>,
  /// Longhand: left margin. Overrides `margin` left value.
  pub margin_left: Option<LengthUnit>,
  /// Positioning offsets (top, right, bottom, left) from the element's normal position.
  pub inset: Sides<LengthUnit>,
  /// Longhand: top offset. Overrides `inset` top value.
  pub top: Option<LengthUnit>,
  /// Longhand: right offset. Overrides `inset` right value.
  pub right: Option<LengthUnit>,
  /// Longhand: bottom offset. Overrides `inset` bottom value.
  pub bottom: Option<LengthUnit>,
  /// Longhand: left offset. Overrides `inset` left value.
  pub left: Option<LengthUnit>,
  /// Direction of flex layout (row or column).
  pub flex_direction: FlexDirection,
  /// How a single grid item is aligned along the inline (row) axis, overriding the container's justify-items value.
  pub justify_self: Option<AlignItems>,
  /// How items are aligned along the main axis.
  pub justify_content: Option<JustifyContent>,
  /// How lines are aligned within the flex container when there's extra space in the cross axis.
  pub align_content: Option<JustifyContent>,
  /// How grid items are aligned along the inline (row) axis within their grid areas.
  pub justify_items: Option<AlignItems>,
  /// How items are aligned along the cross axis.
  pub align_items: Option<AlignItems>,
  /// How a single item is aligned along the cross axis, overriding the container's align-items value.
  pub align_self: Option<AlignItems>,
  /// How flex items should wrap.
  pub flex_wrap: FlexWrap,
  /// The initial main size of the flex item before growing or shrinking.
  pub flex_basis: LengthUnit,
  /// Positioning method (relative, absolute, etc.).
  pub position: Position,
  /// Spacing between rows and columns in flex or grid layouts.
  pub gap: Gap,
  /// How much the flex item should grow relative to other flex items when positive free space is distributed.
  pub flex_grow: f32,
  /// How much the flex item should shrink relative to other flex items when negative free space is distributed.
  pub flex_shrink: f32,
  /// Shorthand border radius (top-left, top-right, bottom-right, bottom-left).
  pub border_radius: Option<Sides<LengthUnit>>,
  /// Longhand: top-left border radius. Overrides `border_radius` top-left value.
  pub border_top_left_radius: Option<LengthUnit>,
  /// Longhand: top-right border radius. Overrides `border_radius` top-right value.
  pub border_top_right_radius: Option<LengthUnit>,
  /// Longhand: bottom-right border radius. Overrides `border_radius` bottom-right value.
  pub border_bottom_right_radius: Option<LengthUnit>,
  /// Longhand: bottom-left border radius. Overrides `border_radius` bottom-left value.
  pub border_bottom_left_radius: Option<LengthUnit>,
  /// Width of the element's border on each side (top, right, bottom, left).
  pub border_width: Sides<LengthUnit>,
  /// Longhand: top border width. Overrides `border_width` top value.
  pub border_top_width: Option<LengthUnit>,
  /// Longhand: right border width. Overrides `border_width` right value.
  pub border_right_width: Option<LengthUnit>,
  /// Longhand: bottom border width. Overrides `border_width` bottom value.
  pub border_bottom_width: Option<LengthUnit>,
  /// Longhand: left border width. Overrides `border_width` left value.
  pub border_left_width: Option<LengthUnit>,
  /// How images should be fitted within their container.
  pub object_fit: ObjectFit,
  /// Background image(s): linear or radial gradients.
  pub background_image: Option<BackgroundImages>,
  /// Background positions per layer.
  pub background_position: Option<BackgroundPositions>,
  /// Background sizes per layer.
  pub background_size: Option<BackgroundSizes>,
  /// Background repeat per layer.
  pub background_repeat: Option<BackgroundRepeats>,
  /// Background color for the element.
  pub background_color: Option<Color>,
  /// Box shadow effect for the element.
  pub box_shadow: Option<BoxShadows>,
  /// Controls the size of implicitly-created grid columns.
  pub grid_auto_columns: Option<GridTrackSizes>,
  /// Controls the size of implicitly-created grid rows.
  pub grid_auto_rows: Option<GridTrackSizes>,
  /// Controls how auto-placed items are inserted in the grid.
  pub grid_auto_flow: Option<GridAutoFlow>,
  /// Specifies a grid item's size and location within the grid column.
  pub grid_column: Option<GridLine>,
  /// Specifies a grid item's size and location within the grid row.
  pub grid_row: Option<GridLine>,
  /// Defines the line names and track sizing functions of the grid columns.
  pub grid_template_columns: Option<GridTemplateComponents>,
  /// Defines the line names and track sizing functions of the grid rows.
  pub grid_template_rows: Option<GridTemplateComponents>,
  /// Defines named grid areas specified via `grid-template-areas`.
  pub grid_template_areas: Option<GridTemplateAreas>,
  /// Transform functions applied to this element.
  pub transform: Option<Transform>,
  /// Transform origin for this element.
  pub transform_origin: Option<TransformOrigin>,
  /// Inheritable style properties that cascade to child elements.
  #[serde(flatten)]
  pub inheritable_style: InheritableStyle,
}

impl Default for Style {
  fn default() -> Self {
    Self {
      display: Display::Flex,
      margin: Sides([LengthUnit::zero(); 4]),
      margin_top: None,
      margin_right: None,
      margin_bottom: None,
      margin_left: None,
      padding: Sides([LengthUnit::zero(); 4]),
      padding_top: None,
      padding_right: None,
      padding_bottom: None,
      padding_left: None,
      width: Default::default(),
      height: Default::default(),
      max_width: Default::default(),
      max_height: Default::default(),
      min_width: Default::default(),
      min_height: Default::default(),
      aspect_ratio: None,
      inset: Default::default(),
      top: None,
      right: None,
      bottom: None,
      left: None,
      flex_direction: Default::default(),
      justify_content: Default::default(),
      align_content: Default::default(),
      justify_self: Default::default(),
      justify_items: Default::default(),
      align_items: Default::default(),
      align_self: Default::default(),
      position: Default::default(),
      gap: Default::default(),
      flex_grow: 0.0,
      flex_shrink: 1.0,
      flex_basis: Default::default(),
      flex_wrap: FlexWrap::NoWrap,
      border_width: Sides([LengthUnit::zero(); 4]),
      border_top_width: None,
      border_right_width: None,
      border_bottom_width: None,
      border_left_width: None,
      border_radius: None,
      border_top_left_radius: None,
      border_top_right_radius: None,
      border_bottom_right_radius: None,
      border_bottom_left_radius: None,
      object_fit: Default::default(),
      box_shadow: Default::default(),
      background_color: None,
      background_image: None,
      background_position: None,
      background_size: None,
      background_repeat: None,
      grid_auto_columns: None,
      grid_auto_rows: None,
      grid_auto_flow: None,
      grid_column: None,
      grid_row: None,
      grid_template_columns: None,
      grid_template_rows: None,
      grid_template_areas: None,
      transform: None,
      transform_origin: None,
      inheritable_style: Default::default(),
    }
  }
}

/// Style properties that can be inherited by child elements.
#[derive(Debug, Clone, Deserialize, Serialize, Default, TS, Merge)]
#[merge(strategy = overwrite_none)]
#[ts(optional_fields, export)]
#[serde(rename_all = "camelCase")]
pub struct InheritableStyle {
  /// How the width and height of an element are calculated.
  pub box_sizing: Option<BoxSizing>,
  /// How text should be overflowed.
  pub text_overflow: Option<TextOverflow>,
  /// Controls text case transformation when rendering.
  pub text_transform: Option<TextTransform>,
  /// Font slant style (normal, italic, oblique).
  pub text_style: Option<TextStyle>,
  /// Color of the element's border.
  pub border_color: Option<Color>,
  /// Text color for child text elements.
  pub color: Option<LinearGradientOrColor>,
  /// Font size for text rendering.
  pub font_size: Option<LengthUnit>,
  /// Font family name for text rendering.
  pub font_family: Option<FontFamily>,
  /// Line height for text spacing, number is em.
  pub line_height: Option<LineHeight>,
  /// Font weight for text rendering.
  pub font_weight: Option<FontWeight>,
  /// Maximum number of lines for text before truncation.
  pub line_clamp: Option<u32>,
  /// Text alignment within the element.
  pub text_align: Option<TextAlign>,
  /// Additional spacing between characters in text.
  pub letter_spacing: Option<LengthUnit>,
  /// Controls how images are scaled when rendered.
  pub image_rendering: Option<ImageScalingAlgorithm>,
}

impl Style {
  #[inline]
  fn convert_template_components(
    components: &Option<GridTemplateComponents>,
    context: &RenderContext,
  ) -> (Vec<taffy::GridTemplateComponent<String>>, Vec<Vec<String>>) {
    let mut track_components: Vec<taffy::GridTemplateComponent<String>> = Vec::new();
    let mut line_name_sets: Vec<Vec<String>> = Vec::new();
    let mut pending_line_names: Vec<String> = Vec::new();

    if let Some(list) = components {
      for comp in list.0.iter() {
        match comp {
          GridTemplateComponent::LineNames(names) => {
            if !names.is_empty() {
              pending_line_names.extend_from_slice(&names[..]);
            }
          }
          GridTemplateComponent::Single(track_size) => {
            // Push names for the line preceding this track
            line_name_sets.push(std::mem::take(&mut pending_line_names));
            // Push the track component
            track_components.push(taffy::GridTemplateComponent::Single(
              track_size.to_min_max(context),
            ));
          }
          GridTemplateComponent::Repeat(repetition, tracks) => {
            // Push names for the line preceding this repeat fragment
            line_name_sets.push(std::mem::take(&mut pending_line_names));

            // Build repetition
            let track_sizes: Vec<taffy::TrackSizingFunction> =
              tracks.iter().map(|t| t.size.to_min_max(context)).collect();

            // Build inner line names: one per line inside the repeat, including a trailing set
            let mut inner_line_names: Vec<Vec<String>> =
              tracks.iter().map(|t| t.names.clone()).collect();
            if let Some(last) = tracks.last() {
              if let Some(end) = &last.end_names {
                inner_line_names.push(end.clone());
              } else {
                inner_line_names.push(Vec::new());
              }
            } else {
              inner_line_names.push(Vec::new());
            }

            track_components.push(taffy::GridTemplateComponent::Repeat(
              taffy::GridTemplateRepetition {
                count: (*repetition).into(),
                tracks: track_sizes,
                line_names: inner_line_names,
              },
            ));
          }
        }
      }
    }

    // Trailing names after the last track
    line_name_sets.push(pending_line_names);

    (track_components, line_name_sets)
  }

  #[inline]
  fn resolve_rect_with_longhands(
    base: Sides<LengthUnit>,
    top: Option<LengthUnit>,
    right: Option<LengthUnit>,
    bottom: Option<LengthUnit>,
    left: Option<LengthUnit>,
  ) -> taffy::Rect<LengthUnit> {
    let mut values = base.0;
    if let Some(v) = top {
      values[0] = v;
    }
    if let Some(v) = right {
      values[1] = v;
    }
    if let Some(v) = bottom {
      values[2] = v;
    }
    if let Some(v) = left {
      values[3] = v;
    }
    taffy::Rect {
      top: values[0],
      right: values[1],
      bottom: values[2],
      left: values[3],
    }
  }

  #[inline]
  fn resolved_padding(&self) -> taffy::Rect<LengthUnit> {
    Self::resolve_rect_with_longhands(
      self.padding,
      self.padding_top,
      self.padding_right,
      self.padding_bottom,
      self.padding_left,
    )
  }

  #[inline]
  fn resolved_margin(&self) -> taffy::Rect<LengthUnit> {
    Self::resolve_rect_with_longhands(
      self.margin,
      self.margin_top,
      self.margin_right,
      self.margin_bottom,
      self.margin_left,
    )
  }

  #[inline]
  fn resolved_inset(&self) -> taffy::Rect<LengthUnit> {
    Self::resolve_rect_with_longhands(self.inset, self.top, self.right, self.bottom, self.left)
  }

  #[inline]
  fn resolved_border_width(&self) -> taffy::Rect<LengthUnit> {
    Self::resolve_rect_with_longhands(
      self.border_width,
      self.border_top_width,
      self.border_right_width,
      self.border_bottom_width,
      self.border_left_width,
    )
  }

  #[inline]
  fn resolved_border_radius(&self) -> taffy::Rect<LengthUnit> {
    Self::resolve_rect_with_longhands(
      self.border_radius.unwrap_or_default(),
      self.border_top_left_radius,
      self.border_top_right_radius,
      self.border_bottom_right_radius,
      self.border_bottom_left_radius,
    )
  }

  /// Creates a `BorderRadius` from the style's border radius properties.
  #[inline]
  pub fn create_border_radius(&self, layout: &Layout, context: &RenderContext) -> BorderRadius {
    BorderRadius::from_layout(context, layout, self.resolved_border_radius())
  }

  /// Converts this style to a Taffy-compatible style for layout calculations.
  pub fn resolve_to_taffy_style(&self, context: &RenderContext) -> TaffyStyle {
    // Convert grid templates and associated line names
    let (grid_template_columns, grid_template_column_names) =
      Self::convert_template_components(&self.grid_template_columns, context);
    let (grid_template_rows, grid_template_row_names) =
      Self::convert_template_components(&self.grid_template_rows, context);

    TaffyStyle {
      box_sizing: self.inheritable_style.box_sizing.unwrap_or_default().into(),
      size: Size {
        width: self.width.resolve_to_dimension(context),
        height: self.height.resolve_to_dimension(context),
      },
      border: resolve_length_unit_rect_to_length_percentage(context, self.resolved_border_width()),
      padding: resolve_length_unit_rect_to_length_percentage(context, self.resolved_padding()),
      inset: resolve_length_unit_rect_to_length_percentage_auto(context, self.resolved_inset()),
      margin: resolve_length_unit_rect_to_length_percentage_auto(context, self.resolved_margin()),
      display: self.display.into(),
      flex_direction: self.flex_direction.into(),
      position: self.position.into(),
      justify_content: self.justify_content.map(Into::into),
      align_content: self.align_content.map(Into::into),
      justify_items: self.justify_items.map(Into::into),
      flex_grow: self.flex_grow,
      align_items: self.align_items.map(Into::into),
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
      grid_auto_columns: self.grid_auto_columns.as_ref().map_or_else(Vec::new, |v| {
        v.0.iter().map(|s| s.to_min_max(context)).collect()
      }),
      grid_auto_rows: self.grid_auto_rows.as_ref().map_or_else(Vec::new, |v| {
        v.0.iter().map(|s| s.to_min_max(context)).collect()
      }),
      grid_auto_flow: self.grid_auto_flow.unwrap_or_default().into(),
      grid_column: self
        .grid_column
        .as_ref()
        .map_or_else(Default::default, |line| line.clone().into()),
      grid_row: self
        .grid_row
        .as_ref()
        .map_or_else(Default::default, |line| line.clone().into()),
      grid_template_columns,
      grid_template_rows,
      grid_template_column_names,
      grid_template_row_names,
      grid_template_areas: self
        .grid_template_areas
        .as_ref()
        .cloned()
        .unwrap_or_default()
        .into(),
      aspect_ratio: self.aspect_ratio,
      align_self: self.align_self.map(Into::into),
      justify_self: self.justify_self.map(Into::into),
      ..Default::default()
    }
  }

  /// Resolves inheritable style properties to concrete values for text rendering.
  pub fn resolve_to_font_style(&self, context: &RenderContext) -> FontStyle {
    let font_size = self
      .inheritable_style
      .font_size
      .map(|f| f.resolve_to_px(context))
      .unwrap_or(DEFAULT_FONT_SIZE);

    let line_height = self
      .inheritable_style
      .line_height
      .map(|f| f.0.resolve_to_px(context))
      .unwrap_or_else(|| font_size * DEFAULT_LINE_HEIGHT_SCALER);

    FontStyle {
      color: self
        .inheritable_style
        .color
        .clone()
        .unwrap_or(LinearGradientOrColor::Color(Color([0, 0, 0, 255]))),
      font_size,
      line_height,
      font_weight: self
        .inheritable_style
        .font_weight
        .unwrap_or_default()
        .into(),
      text_style: self.inheritable_style.text_style.unwrap_or_default(),
      line_clamp: self.inheritable_style.line_clamp,
      font_family: self.inheritable_style.font_family.clone().map(Into::into),
      letter_spacing: self
        .inheritable_style
        .letter_spacing
        .map(|spacing| spacing.resolve_to_px(context) / context.parent_font_size),
      text_align: self.inheritable_style.text_align.and_then(Into::into),
      text_overflow: self
        .inheritable_style
        .text_overflow
        .unwrap_or(TextOverflow::Clip),
      text_transform: self.inheritable_style.text_transform.unwrap_or_default(),
    }
  }
}
