use serde::Serialize;

use crate::{
  context::RenderContext,
  font_style::SizedFontStyle,
  style::{BackgroundImage, Color, ColorInput, ComputedStyle, ToCss, ZIndex},
};

/// The resolved paint properties of a measured box, serialized from the same
/// computed style the renderer paints with.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct MeasuredStyle {
  /// Used `color`, with `currentColor` resolved.
  pub color: String,
  /// Computed `font-family` stack.
  pub font_family: String,
  /// Used `font-size` in device pixels.
  pub font_size: f32,
  /// Used numeric `font-weight`.
  pub font_weight: f32,
  /// Computed `font-style`.
  pub font_style: String,
  /// Used `line-height` in device pixels, absent when it resolves against font
  /// metrics rather than a length.
  #[serde(skip_serializing_if = "Option::is_none")]
  pub line_height: Option<f32>,
  /// Used `letter-spacing` in device pixels.
  pub letter_spacing: f32,
  /// Computed `text-align`.
  pub text_align: String,
  /// Computed `text-transform`.
  pub text_transform: String,
  /// Computed `display`, the box type the node generated.
  pub display: String,
  /// Computed `position`.
  pub position: String,
  /// Computed `visibility`.
  pub visibility: String,
  /// Computed `list-style-type`.
  pub list_style_type: String,
  /// Used padding in device pixels: top, right, bottom, left.
  #[serde(skip_serializing_if = "Option::is_none")]
  pub padding: Option<[f32; 4]>,
  /// Used `opacity`.
  pub opacity: f32,
  /// Used `background-color`, omitted when transparent.
  #[serde(skip_serializing_if = "Option::is_none")]
  pub background_color: Option<String>,
  /// Computed `background-image`, omitted when `none`.
  #[serde(skip_serializing_if = "Option::is_none")]
  pub background_image: Option<String>,
  /// Used corner radii in device pixels: top-left, top-right, bottom-right,
  /// bottom-left. An elliptical corner reports its horizontal radius.
  #[serde(skip_serializing_if = "Option::is_none")]
  pub border_radius: Option<[f32; 4]>,
  /// Used border widths in device pixels: top, right, bottom, left.
  #[serde(skip_serializing_if = "Option::is_none")]
  pub border_widths: Option<[f32; 4]>,
  /// Used border colors: top, right, bottom, left.
  #[serde(skip_serializing_if = "Option::is_none")]
  pub border_colors: Option<[String; 4]>,
  /// Computed `box-shadow`, omitted when `none`.
  #[serde(skip_serializing_if = "Option::is_none")]
  pub box_shadow: Option<String>,
  /// Computed `z-index`, omitted when `auto`.
  #[serde(skip_serializing_if = "Option::is_none")]
  pub z_index: Option<i32>,
}

/// The resolved paint properties of a measured text run.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct MeasuredTextRunStyle {
  /// Used `color`, with `currentColor` resolved.
  pub color: String,
  /// Family name of the face the run was shaped with.
  pub font_family: String,
  /// Used `font-size` in device pixels.
  pub font_size: f32,
  /// Used numeric `font-weight`.
  pub font_weight: f32,
  /// Computed `font-style`.
  pub font_style: String,
  /// Used `letter-spacing` in device pixels.
  pub letter_spacing: f32,
  /// Used `opacity` of the inline element the run came from, which generates no box of its own.
  pub opacity: f32,
}

impl MeasuredStyle {
  /// Reads the paint properties off a node's resolved context. `size` is the node's border box,
  /// which a percentage `border-radius` resolves against.
  pub fn from_context(context: &RenderContext, size: (f32, f32)) -> Self {
    let style = &context.style;
    let sizing = &context.sizing;
    let current_color = context.current_color;
    let font_style = SizedFontStyle::from_style(style, context);

    let border_widths = [
      style.border_top_width.to_used_px(sizing),
      style.border_right_width.to_used_px(sizing),
      style.border_bottom_width.to_used_px(sizing),
      style.border_left_width.to_used_px(sizing),
    ];
    let border_radius = [
      style.border_top_left_radius,
      style.border_top_right_radius,
      style.border_bottom_right_radius,
      style.border_bottom_left_radius,
    ]
    .map(|radius| radius.x.to_px(sizing, size.0));
    let has_border = border_widths.iter().any(|width| *width > 0.0);
    let padding = [
      style.padding_top,
      style.padding_right,
      style.padding_bottom,
      style.padding_left,
    ]
    .map(|padding| padding.to_px(sizing, 0.0));

    Self {
      color: style.color.resolve(current_color).to_css_string(),
      font_family: style.font_family.to_css_string(),
      font_size: sizing.font_size,
      font_weight: style.font_weight.value(),
      font_style: style.font_style.to_css_string(),
      line_height: font_style.line_height_px,
      letter_spacing: font_style.letter_spacing,
      text_align: style.text_align.to_css_string(),
      text_transform: style.text_transform.to_css_string(),
      display: style.display.to_css_string(),
      position: style.position.to_css_string(),
      visibility: style.visibility.to_css_string(),
      list_style_type: style.list_style_type.to_css_string(),
      padding: padding.iter().any(|value| *value > 0.0).then_some(padding),
      opacity: style.opacity.0,
      background_color: painted_color(style.background_color, current_color),
      // `none` layers are placeholders that keep the other `background-*` lists aligned,
      // so a list of only those is the same as no background image at all.
      background_image: style
        .background_image
        .as_ref()
        .filter(|images| images.iter().any(BackgroundImage::paints))
        .map(ToCss::to_css_string),
      border_radius: border_radius
        .iter()
        .any(|radius| *radius > 0.0)
        .then_some(border_radius),
      border_widths: has_border.then_some(border_widths),
      border_colors: has_border.then(|| {
        [
          style.border_top_color,
          style.border_right_color,
          style.border_bottom_color,
          style.border_left_color,
        ]
        .map(|color| color.resolve(current_color).to_css_string())
      }),
      box_shadow: style
        .box_shadow
        .as_ref()
        .filter(|shadows| !shadows.is_empty())
        .map(ToCss::to_css_string),
      z_index: match style.z_index {
        ZIndex::Integer(value) => Some(value),
        ZIndex::Auto => None,
      },
    }
  }
}

impl MeasuredTextRunStyle {
  /// Reads the paint properties off the font style a run was shaped with.
  pub(crate) fn from_font_style(
    style: &SizedFontStyle<'_>,
    face_family: Option<String>,
    opacity: f32,
  ) -> Self {
    let parent: &ComputedStyle = style.parent;

    Self {
      color: style.color.to_css_string(),
      // The computed stack stands in when the selected face has no readable family name.
      font_family: face_family.unwrap_or_else(|| parent.font_family.to_css_string()),
      font_size: style.sizing.font_size,
      font_weight: parent.font_weight.value(),
      font_style: parent.font_style.to_css_string(),
      letter_spacing: style.letter_spacing,
      opacity,
    }
  }
}

fn painted_color(input: ColorInput, current_color: Color) -> Option<String> {
  let color = input.resolve(current_color);
  (color.0[3] > 0).then(|| color.to_css_string())
}
