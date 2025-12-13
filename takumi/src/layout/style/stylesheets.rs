use std::{borrow::Cow, marker::PhantomData};

use derive_builder::Builder;
use parley::{FontSettings, FontStack, FontWidth, TextStyle};
use serde::Deserialize;
use smallvec::SmallVec;
use taffy::{Point, Size, prelude::FromLength};

use crate::{
  layout::{
    inline::InlineBrush,
    style::{CssValue, properties::*},
  },
  rendering::{RenderContext, SizedShadow},
};

/// Helper macro to define the `Style` struct and `InheritedStyle` struct.
macro_rules! define_style {
  ($( $(#[$attr:meta])? $property:ident: $type:ty $(where inherit = $inherit:expr)? ),* $(,)?) => {
    /// Defines the style of an element.
    #[derive(Debug, Default, Clone, Deserialize, Builder, PartialEq)]
    #[serde(default, rename_all = "camelCase")]
    #[builder(default, setter(into))]
    pub struct Style {
      $(
        $(#[$attr])?
        #[allow(missing_docs)]
        pub $property: CssValue<$type$(, $inherit)?>,
      )*
    }

    impl Style {
      /// Inherits the style from the parent element.
      pub(crate) fn inherit(self, parent: &InheritedStyle) -> InheritedStyle {
        InheritedStyle {
          $( $property: self.$property.inherit_value(&parent.$property), )*
        }
      }
    }

    /// A resolved set of style properties.
    #[derive(Clone, Debug, Default)]
    pub struct InheritedStyle {
      $( pub(crate) $property: $type, )*
    }
  };
}

define_style!(
  // For convenience, we default to border-box
  box_sizing: BoxSizing,
  opacity: PercentageNumber,
  display: Display,
  width: LengthUnit,
  height: LengthUnit,
  max_width: LengthUnit,
  max_height: LengthUnit,
  min_width: LengthUnit,
  min_height: LengthUnit,
  aspect_ratio: AspectRatio,
  padding: Sides<LengthUnit<false>>,
  padding_inline: Option<SpacePair<LengthUnit<false>>>,
  padding_block: Option<SpacePair<LengthUnit<false>>>,
  padding_top: Option<LengthUnit<false>>,
  padding_right: Option<LengthUnit<false>>,
  padding_bottom: Option<LengthUnit<false>>,
  padding_left: Option<LengthUnit<false>>,
  margin: Sides<LengthUnit<false>>,
  margin_inline: Option<SpacePair<LengthUnit<false>>>,
  margin_block: Option<SpacePair<LengthUnit<false>>>,
  margin_top: Option<LengthUnit<false>>,
  margin_right: Option<LengthUnit<false>>,
  margin_bottom: Option<LengthUnit<false>>,
  margin_left: Option<LengthUnit<false>>,
  inset: Sides<LengthUnit>,
  inset_inline: Option<SpacePair<LengthUnit>>,
  inset_block: Option<SpacePair<LengthUnit>>,
  top: Option<LengthUnit>,
  right: Option<LengthUnit>,
  bottom: Option<LengthUnit>,
  left: Option<LengthUnit>,
  flex_direction: FlexDirection,
  justify_self: AlignItems,
  justify_content: JustifyContent,
  align_content: JustifyContent,
  justify_items: AlignItems,
  align_items: AlignItems,
  align_self: AlignItems,
  flex_wrap: FlexWrap,
  flex_basis: Option<LengthUnit>,
  position: Position,
  rotate: Option<Angle>,
  scale: Option<SpacePair<PercentageNumber>>,
  scale_x: Option<PercentageNumber>,
  scale_y: Option<PercentageNumber>,
  transform: Option<Transforms>,
  transform_origin: Option<BackgroundPosition>,
  translate: Option<SpacePair<LengthUnit>>,
  translate_x: Option<LengthUnit>,
  translate_y: Option<LengthUnit>,
  mask: Backgrounds,
  mask_image: Option<BackgroundImages>,
  mask_size: Option<BackgroundSizes>,
  mask_position: Option<BackgroundPositions>,
  mask_repeat: Option<BackgroundRepeats>,
  gap: SpacePair<LengthUnit<false>, true>,
  column_gap: Option<LengthUnit<false>>,
  row_gap: Option<LengthUnit<false>>,
  flex: Option<Flex>,
  flex_grow: Option<FlexGrow>,
  flex_shrink: Option<FlexGrow>,
  border_radius: BorderRadius,
  border_top_left_radius: Option<SpacePair<LengthUnit<false>>>,
  border_top_right_radius: Option<SpacePair<LengthUnit<false>>>,
  border_bottom_right_radius: Option<SpacePair<LengthUnit<false>>>,
  border_bottom_left_radius: Option<SpacePair<LengthUnit<false>>>,
  border_width: Option<Sides<LengthUnit>>,
  border_inline_width: Option<SpacePair<LengthUnit>>,
  border_block_width: Option<SpacePair<LengthUnit>>,
  border_top_width: Option<LengthUnit>,
  border_right_width: Option<LengthUnit>,
  border_bottom_width: Option<LengthUnit>,
  border_left_width: Option<LengthUnit>,
  border: Border,
  object_fit: ObjectFit,
  overflow: SpacePair<Overflow>,
  overflow_x: Option<Overflow>,
  overflow_y: Option<Overflow>,
  object_position: BackgroundPosition where inherit = true,
  background: Backgrounds,
  background_image: Option<BackgroundImages>,
  background_position: Option<BackgroundPositions>,
  background_size: Option<BackgroundSizes>,
  background_repeat: Option<BackgroundRepeats>,
  background_color: ColorInput<false>,
  background_clip: BackgroundClip,
  box_shadow: Option<BoxShadows>,
  grid_auto_columns: Option<GridTrackSizes>,
  grid_auto_rows: Option<GridTrackSizes>,
  grid_auto_flow: Option<GridAutoFlow>,
  grid_column: Option<GridLine>,
  grid_row: Option<GridLine>,
  grid_template_columns: Option<GridTemplateComponents>,
  grid_template_rows: Option<GridTemplateComponents>,
  grid_template_areas: Option<GridTemplateAreas>,
  text_overflow: TextOverflow,
  text_transform: TextTransform where inherit = true,
  font_style: FontStyle where inherit = true,
  border_color: Option<ColorInput>,
  color: ColorInput where inherit = true,
  filter: Option<Filters>,
  font_size: Option<LengthUnit> where inherit = true,
  font_family: Option<FontFamily> where inherit = true,
  line_height: LineHeight where inherit = true,
  font_weight: FontWeight where inherit = true,
  font_variation_settings: Option<FontVariationSettings> where inherit = true,
  font_feature_settings: Option<FontFeatureSettings> where inherit = true,
  line_clamp: Option<LineClamp> where inherit = true,
  text_align: TextAlign where inherit = true,
  #[serde(rename = "webkitTextStroke")]
  webkit_text_stroke: Option<TextStroke> where inherit = true,
  #[serde(rename = "webkitTextStrokeWidth")]
  webkit_text_stroke_width: Option<LengthUnit<false>> where inherit = true,
  #[serde(rename = "webkitTextStrokeColor")]
  webkit_text_stroke_color: Option<ColorInput> where inherit = true,
  text_shadow: Option<TextShadows> where inherit = true,
  text_decoration: TextDecoration,
  text_decoration_line: Option<TextDecorationLines> where inherit = true,
  text_decoration_color: Option<ColorInput> where inherit = true,
  letter_spacing: Option<LengthUnit> where inherit = true,
  word_spacing: Option<LengthUnit> where inherit = true,
  image_rendering: ImageScalingAlgorithm where inherit = true,
  overflow_wrap: OverflowWrap where inherit = true,
  word_break: WordBreak where inherit = true,
  clip_path: Option<BasicShape>,
  clip_rule: FillRule where inherit = true,
  white_space: WhiteSpace where inherit = true,
  white_space_collapse: Option<WhiteSpaceCollapse> where inherit = true,
  text_wrap_mode: Option<TextWrapMode> where inherit = true,
  text_wrap: Option<TextWrapMode> where inherit = true,
);

/// Sized font style with resolved font size and line height.
#[derive(Clone)]
pub(crate) struct SizedFontStyle<'s> {
  pub parent: &'s InheritedStyle,
  pub font_size: f32,
  pub line_height: parley::LineHeight,
  pub stroke_width: f32,
  pub letter_spacing: Option<f32>,
  pub word_spacing: Option<f32>,
  pub text_shadow: Option<SmallVec<[SizedShadow; 4]>>,
  pub color: Color,
  pub text_stroke_color: Color,
  pub text_decoration_color: Color,
}

impl<'s> From<&'s SizedFontStyle<'s>> for TextStyle<'s, InlineBrush> {
  fn from(style: &'s SizedFontStyle<'s>) -> Self {
    TextStyle {
      font_size: style.font_size,
      line_height: style.line_height,
      font_weight: style.parent.font_weight.into(),
      font_style: style.parent.font_style.into(),
      font_variations: FontSettings::List(Cow::Borrowed(
        style
          .parent
          .font_variation_settings
          .as_deref()
          .unwrap_or(&[]),
      )),
      font_features: FontSettings::List(Cow::Borrowed(
        style.parent.font_feature_settings.as_deref().unwrap_or(&[]),
      )),
      font_stack: style
        .parent
        .font_family
        .as_ref()
        .map(Into::into)
        .unwrap_or(FontStack::Source(Cow::Borrowed("sans-serif"))),
      letter_spacing: style.letter_spacing.unwrap_or_default(),
      word_spacing: style.word_spacing.unwrap_or_default(),
      word_break: style.parent.word_break.into(),
      overflow_wrap: if style.parent.word_break == WordBreak::BreakWord {
        // When word-break is break-word, ignore the overflow-wrap property's value.
        // https://developer.mozilla.org/en-US/docs/Web/CSS/word-break#break-word
        parley::OverflowWrap::Anywhere
      } else {
        style.parent.overflow_wrap.into()
      },
      brush: InlineBrush {
        color: style.color,
        decoration_color: style.text_decoration_color,
        stroke_color: style.text_stroke_color,
      },
      text_wrap_mode: style.parent.text_wrap_mode_and_line_clamp().0.into(),

      font_width: FontWidth::NORMAL,
      locale: None,
      has_underline: false,
      underline_offset: None,
      underline_size: None,
      underline_brush: None,
      has_strikethrough: false,
      strikethrough_offset: None,
      strikethrough_size: None,
      strikethrough_brush: None,
    }
  }
}

impl InheritedStyle {
  pub(crate) fn resolve_overflows(&self) -> SpacePair<Overflow> {
    SpacePair::from_pair(
      self.overflow_x.unwrap_or(self.overflow.x),
      self.overflow_y.unwrap_or(self.overflow.y),
    )
  }

  pub(crate) fn resolve_translate(&self) -> SpacePair<LengthUnit> {
    SpacePair::from_pair(
      self
        .translate_x
        .unwrap_or(self.translate.unwrap_or_default().x),
      self
        .translate_y
        .unwrap_or(self.translate.unwrap_or_default().y),
    )
  }

  pub(crate) fn resolve_scale(&self) -> SpacePair<PercentageNumber> {
    SpacePair::from_pair(
      self.scale_x.unwrap_or(self.scale.unwrap_or_default().x),
      self.scale_y.unwrap_or(self.scale.unwrap_or_default().y),
    )
  }

  pub(crate) fn ellipsis_char(&self) -> &str {
    const ELLIPSIS_CHAR: &str = "…";

    match &self.text_overflow {
      TextOverflow::Ellipsis => return ELLIPSIS_CHAR,
      TextOverflow::Custom(custom) => return custom.as_str(),
      _ => {}
    }

    if let Some(clamp) = &self
      .line_clamp
      .as_ref()
      .and_then(|clamp| clamp.ellipsis.as_deref())
    {
      return clamp;
    }

    ELLIPSIS_CHAR
  }

  pub(crate) fn should_handle_ellipsis(&self) -> bool {
    self.text_overflow == TextOverflow::Ellipsis || self.line_clamp.is_some()
  }

  pub(crate) fn white_space_collapse(&self) -> WhiteSpaceCollapse {
    self
      .white_space_collapse
      .unwrap_or(self.white_space.white_space_collapse)
  }

  pub(crate) fn text_wrap_mode_and_line_clamp(&self) -> (TextWrapMode, Option<Cow<'_, LineClamp>>) {
    let mut text_wrap_mode = self
      .text_wrap_mode
      .or(self.text_wrap)
      .unwrap_or(self.white_space.text_wrap_mode);

    let mut line_clamp = self.line_clamp.as_ref().map(Cow::Borrowed);

    // Special case: when nowrap + ellipsis, parley will layout all the text even when it overflows.
    // So we need to use a fixed line clamp of 1 instead.
    if text_wrap_mode == TextWrapMode::NoWrap && self.text_overflow == TextOverflow::Ellipsis {
      line_clamp = Some(Cow::Owned(LineClamp {
        count: 1,
        ellipsis: Some(self.ellipsis_char().to_string()),
      }));

      text_wrap_mode = TextWrapMode::Wrap;
    }

    (text_wrap_mode, line_clamp)
  }

  #[inline]
  fn convert_template_components(
    components: &Option<GridTemplateComponents>,
    context: &RenderContext,
  ) -> (Vec<taffy::GridTemplateComponent<String>>, Vec<Vec<String>>) {
    let mut track_components: Vec<taffy::GridTemplateComponent<String>> = Vec::new();
    let mut line_name_sets: Vec<Vec<String>> = Vec::new();
    let mut pending_line_names: Vec<String> = Vec::new();

    if let Some(list) = components {
      for comp in list.iter() {
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
  fn resolve_rect_with_longhands<T: Copy>(
    base: Sides<T>,
    inline: Option<SpacePair<T>>,
    block: Option<SpacePair<T>>,
    top: Option<T>,
    right: Option<T>,
    bottom: Option<T>,
    left: Option<T>,
  ) -> taffy::Rect<T> {
    let mut values = base.0;

    if let Some(pair) = inline {
      values[3] = pair.x; // left
      values[1] = pair.y; // right
    }

    if let Some(pair) = block {
      values[0] = pair.x; // top
      values[2] = pair.y; // bottom
    }

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
  fn resolved_padding(&self) -> taffy::Rect<LengthUnit<false>> {
    Self::resolve_rect_with_longhands(
      self.padding,
      self.padding_inline,
      self.padding_block,
      self.padding_top,
      self.padding_right,
      self.padding_bottom,
      self.padding_left,
    )
  }

  #[inline]
  fn resolved_margin(&self) -> taffy::Rect<LengthUnit<false>> {
    Self::resolve_rect_with_longhands(
      self.margin,
      self.margin_inline,
      self.margin_block,
      self.margin_top,
      self.margin_right,
      self.margin_bottom,
      self.margin_left,
    )
  }

  #[inline]
  fn resolved_inset(&self) -> taffy::Rect<LengthUnit> {
    Self::resolve_rect_with_longhands(
      self.inset,
      self.inset_inline,
      self.inset_block,
      self.top,
      self.right,
      self.bottom,
      self.left,
    )
  }

  #[inline]
  fn resolved_gap(&self) -> SpacePair<LengthUnit<false>> {
    SpacePair::from_pair(
      self.row_gap.unwrap_or(self.gap.x),
      self.column_gap.unwrap_or(self.gap.y),
    )
  }

  #[inline]
  fn resolved_border_width(&self) -> taffy::Rect<LengthUnit> {
    Self::resolve_rect_with_longhands(
      self
        .border_width
        .or_else(|| self.border.width.map(Into::into))
        .unwrap_or(Sides::zero()),
      self.border_inline_width,
      self.border_block_width,
      self.border_top_width,
      self.border_right_width,
      self.border_bottom_width,
      self.border_left_width,
    )
  }

  #[inline]
  pub(crate) fn resolved_border_radius(&self) -> taffy::Rect<SpacePair<LengthUnit<false>>> {
    Self::resolve_rect_with_longhands(
      self.border_radius.0,
      None,
      None,
      self.border_top_left_radius,
      self.border_top_right_radius,
      self.border_bottom_right_radius,
      self.border_bottom_left_radius,
    )
  }

  pub(crate) fn to_sized_font_style(&'_ self, context: &RenderContext) -> SizedFontStyle<'_> {
    let line_height = self.line_height.into_parley(context);

    let resolved_stroke_width = self
      .webkit_text_stroke_width
      .or(self.webkit_text_stroke.map(|stroke| stroke.width))
      .unwrap_or_default()
      .resolve_to_px(context, context.font_size);

    SizedFontStyle {
      parent: self,
      font_size: context.font_size,
      line_height,
      stroke_width: resolved_stroke_width,
      letter_spacing: self
        .letter_spacing
        .map(|spacing| spacing.resolve_to_px(context, context.font_size)),
      word_spacing: self
        .word_spacing
        .map(|spacing| spacing.resolve_to_px(context, context.font_size)),
      text_shadow: self.text_shadow.as_ref().map(|shadows| {
        shadows
          .iter()
          .map(|shadow| {
            SizedShadow::from_text_shadow(*shadow, context, Size::from_length(context.font_size))
          })
          .collect()
      }),
      color: self.color.resolve(context.current_color, context.opacity),
      text_stroke_color: self
        .webkit_text_stroke_color
        .or(self.webkit_text_stroke.and_then(|stroke| stroke.color))
        .unwrap_or_default()
        .resolve(context.current_color, context.opacity),
      text_decoration_color: self
        .text_decoration_color
        .or(self.text_decoration.color)
        .unwrap_or(ColorInput::CurrentColor)
        .resolve(context.current_color, context.opacity),
    }
  }

  pub(crate) fn to_taffy_style(&self, context: &RenderContext) -> taffy::style::Style {
    // Convert grid templates and associated line names
    let (grid_template_columns, grid_template_column_names) =
      Self::convert_template_components(&self.grid_template_columns, context);
    let (grid_template_rows, grid_template_row_names) =
      Self::convert_template_components(&self.grid_template_rows, context);

    taffy::style::Style {
      box_sizing: self.box_sizing.into(),
      size: Size {
        width: self.width.resolve_to_dimension(context),
        height: self.height.resolve_to_dimension(context),
      },
      border: self
        .resolved_border_width()
        .map(|border| border.resolve_to_length_percentage(context)),
      padding: self
        .resolved_padding()
        .map(|padding| padding.resolve_to_length_percentage(context)),
      inset: self
        .resolved_inset()
        .map(|inset| inset.resolve_to_length_percentage_auto(context)),
      margin: self
        .resolved_margin()
        .map(|margin| margin.resolve_to_length_percentage_auto(context)),
      display: self.display.into(),
      flex_direction: self.flex_direction.into(),
      position: self.position.into(),
      justify_content: self.justify_content.into(),
      align_content: self.align_content.into(),
      justify_items: self.justify_items.into(),
      flex_grow: self
        .flex_grow
        .map(|grow| grow.0)
        .or_else(|| self.flex.map(|flex| flex.grow))
        .unwrap_or(0.0),
      align_items: self.align_items.into(),
      gap: self.resolved_gap().resolve_to_size(context),
      flex_basis: self
        .flex_basis
        .or_else(|| self.flex.map(|flex| flex.basis))
        .unwrap_or(LengthUnit::Auto)
        .resolve_to_dimension(context),
      flex_shrink: self
        .flex_shrink
        .map(|shrink| shrink.0)
        .or_else(|| self.flex.map(|flex| flex.shrink))
        .unwrap_or(1.0),
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
        v.iter().map(|s| s.to_min_max(context)).collect()
      }),
      grid_auto_rows: self.grid_auto_rows.as_ref().map_or_else(Vec::new, |v| {
        v.iter().map(|s| s.to_min_max(context)).collect()
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
      aspect_ratio: self.aspect_ratio.into(),
      align_self: self.align_self.into(),
      justify_self: self.justify_self.into(),
      overflow: Point::from(self.resolve_overflows()).map(Into::into),
      dummy: PhantomData,
      item_is_table: false,
      item_is_replaced: false,
      scrollbar_width: 0.0,
      text_align: taffy::TextAlign::Auto,
    }
  }
}
