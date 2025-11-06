use std::{borrow::Cow, marker::PhantomData};

use derive_builder::Builder;
use parley::{FontSettings, FontStack, FontWidth, TextStyle};
use serde::{Deserialize, Serialize};
use smallvec::SmallVec;
use taffy::{Point, Size, prelude::FromLength};
use ts_rs::TS;

use crate::{
  layout::{
    inline::InlineBrush,
    style::{CssOption, CssValue, properties::*},
  },
  rendering::{RenderContext, SizedShadow},
};

/// Helper macro to define the `Style` struct and `InheritedStyle` struct.
macro_rules! define_style {
  ($( $(#[$attr:meta])? $property:ident: $type:ty = $default_global:expr => $initial_value:expr),* $(,)?) => {
    /// Defines the style of an element.
    #[derive(Debug, Clone, Deserialize, Serialize, TS, Builder, PartialEq)]
    #[serde(default, rename_all = "camelCase")]
    #[ts(export, optional_fields)]
    #[builder(default, setter(into))]
    pub struct Style {
      $(
        #[allow(missing_docs)]
        $(#[$attr])?
        pub $property: CssValue<$type>,
      )*
    }

    impl Default for Style {
      fn default() -> Self {
        Self {
          $( $property: $default_global.into(), )*
        }
      }
    }

    impl Style {
      /// Inherits the style from the parent element.
      pub(crate) fn inherit(self, parent: &InheritedStyle) -> InheritedStyle {
        InheritedStyle {
          $( $property: self.$property.inherit_value(&parent.$property, $initial_value), )*
        }
      }
    }

    /// A resolved set of style properties.
    #[derive(Clone, Debug)]
    pub struct InheritedStyle {
      $( pub(crate) $property: $type, )*
    }

    impl Default for InheritedStyle {
      fn default() -> Self {
        Self { $( $property: $initial_value, )* }
      }
    }
  };
}

// property: type = node default value => viewport default value
define_style!(
  // For convenience, we default to border-box
  box_sizing: BoxSizing = CssValue::inherit() => BoxSizing::BorderBox,
  opacity: PercentageNumber = PercentageNumber(1.0) => PercentageNumber(1.0),
  display: Display = Display::Flex => Display::Flex,
  width: LengthUnit = LengthUnit::Auto => LengthUnit::Auto,
  height: LengthUnit = LengthUnit::Auto => LengthUnit::Auto,
  max_width: LengthUnit = LengthUnit::Auto => LengthUnit::Auto,
  max_height: LengthUnit = LengthUnit::Auto => LengthUnit::Auto,
  min_width: LengthUnit = LengthUnit::Auto => LengthUnit::Auto,
  min_height: LengthUnit = LengthUnit::Auto => LengthUnit::Auto,
  aspect_ratio: AspectRatio = AspectRatio::Auto => AspectRatio::Auto,
  padding: Sides<LengthUnit> = Sides::zero() => Sides::zero(),
  padding_inline: CssOption<SpacePair<LengthUnit>> = CssOption::none() => CssOption::none(),
  padding_block: CssOption<SpacePair<LengthUnit>> = CssOption::none() => CssOption::none(),
  padding_top: CssOption<LengthUnit> = CssOption::none() => CssOption::none(),
  padding_right: CssOption<LengthUnit> = CssOption::none() => CssOption::none(),
  padding_bottom: CssOption<LengthUnit> = CssOption::none() => CssOption::none(),
  padding_left: CssOption<LengthUnit> = CssOption::none() => CssOption::none(),
  margin: Sides<LengthUnit> = Sides::zero() => Sides::zero(),
  margin_inline: CssOption<SpacePair<LengthUnit>> = CssOption::none() => CssOption::none(),
  margin_block: CssOption<SpacePair<LengthUnit>> = CssOption::none() => CssOption::none(),
  margin_top: CssOption<LengthUnit> = CssOption::none() => CssOption::none(),
  margin_right: CssOption<LengthUnit> = CssOption::none() => CssOption::none(),
  margin_bottom: CssOption<LengthUnit> = CssOption::none() => CssOption::none(),
  margin_left: CssOption<LengthUnit> = CssOption::none() => CssOption::none(),
  inset: Sides<LengthUnit> = Sides::auto() => Sides::auto(),
  inset_inline: CssOption<SpacePair<LengthUnit>> = CssOption::none() => CssOption::none(),
  inset_block: CssOption<SpacePair<LengthUnit>> = CssOption::none() => CssOption::none(),
  top: CssOption<LengthUnit> = CssOption::none() => CssOption::none(),
  right: CssOption<LengthUnit> = CssOption::none() => CssOption::none(),
  bottom: CssOption<LengthUnit> = CssOption::none() => CssOption::none(),
  left: CssOption<LengthUnit> = CssOption::none() => CssOption::none(),
  flex_direction: FlexDirection = FlexDirection::Row => FlexDirection::Row,
  justify_self: AlignItems = AlignItems::Normal => AlignItems::Normal,
  justify_content: JustifyContent = JustifyContent::Normal => JustifyContent::Normal,
  align_content: JustifyContent = JustifyContent::Normal => JustifyContent::Normal,
  justify_items: AlignItems = AlignItems::Normal => AlignItems::Normal,
  align_items: AlignItems = AlignItems::Normal => AlignItems::Normal,
  align_self: AlignItems = AlignItems::Normal => AlignItems::Normal,
  flex_wrap: FlexWrap = FlexWrap::NoWrap => FlexWrap::NoWrap,
  flex_basis: CssOption<LengthUnit> = CssOption::none() => CssOption::none(),
  position: Position = Position::Relative => Position::Relative,
  rotate: CssOption<Angle> = CssOption::none() => CssOption::none(),
  scale: CssOption<SpacePair<PercentageNumber>> = CssOption::none() => CssOption::none(),
  scale_x: CssOption<PercentageNumber> = CssOption::none() => CssOption::none(),
  scale_y: CssOption<PercentageNumber> = CssOption::none() => CssOption::none(),
  transform: CssOption<Transforms> = CssOption::none() => CssOption::none(),
  transform_origin: CssOption<BackgroundPosition> = CssOption::none() => CssOption::none(),
  translate: CssOption<SpacePair<LengthUnit>> = CssOption::none() => CssOption::none(),
  translate_x: CssOption<LengthUnit> = CssOption::none() => CssOption::none(),
  translate_y: CssOption<LengthUnit> = CssOption::none() => CssOption::none(),
  mask_image: CssOption<BackgroundImages> = CssOption::none() => CssOption::none(),
  mask_size: CssOption<BackgroundSizes> = CssOption::none() => CssOption::none(),
  mask_position: CssOption<BackgroundPositions> = CssOption::none() => CssOption::none(),
  mask_repeat: CssOption<BackgroundRepeats> = CssOption::none() => CssOption::none(),
  gap: SpacePair<LengthUnit, true> = SpacePair::from_single(LengthUnit::Px(0.0)) => SpacePair::from_single(LengthUnit::Px(0.0)),
  column_gap: CssOption<LengthUnit> = CssOption::none() => CssOption::none(),
  row_gap: CssOption<LengthUnit> = CssOption::none() => CssOption::none(),
  flex: CssOption<Flex> = CssOption::none() => CssOption::none(),
  flex_grow: CssOption<FlexGrow> = CssOption::none() => CssOption::none(),
  flex_shrink: CssOption<FlexGrow> = CssOption::none() => CssOption::none(),
  border_radius: Sides<LengthUnit> = Sides::zero() => Sides::zero(),
  border_top_left_radius: CssOption<LengthUnit> = CssOption::none() => CssOption::none(),
  border_top_right_radius: CssOption<LengthUnit> = CssOption::none() => CssOption::none(),
  border_bottom_right_radius: CssOption<LengthUnit> = CssOption::none() => CssOption::none(),
  border_bottom_left_radius: CssOption<LengthUnit> = CssOption::none() => CssOption::none(),
  border_width: CssOption<Sides<LengthUnit>> = CssOption::none() => CssOption::none(),
  border_inline_width: CssOption<SpacePair<LengthUnit>> = CssOption::none() => CssOption::none(),
  border_block_width: CssOption<SpacePair<LengthUnit>> = CssOption::none() => CssOption::none(),
  border_top_width: CssOption<LengthUnit> = CssOption::none() => CssOption::none(),
  border_right_width: CssOption<LengthUnit> = CssOption::none() => CssOption::none(),
  border_bottom_width: CssOption<LengthUnit> = CssOption::none() => CssOption::none(),
  border_left_width: CssOption<LengthUnit> = CssOption::none() => CssOption::none(),
  border: Border = Border::default() => Border::default(),
  object_fit: ObjectFit = CssValue::inherit() => Default::default(),
  overflow: Overflows = Overflows::default() => Overflows::default(),
  overflow_x: CssOption<Overflow> = CssOption::none() => CssOption::none(),
  overflow_y: CssOption<Overflow> = CssOption::none() => CssOption::none(),
  object_position: BackgroundPosition = CssValue::inherit() => BackgroundPosition::default(),
  background_image: CssOption<BackgroundImages> = CssOption::none() => CssOption::none(),
  background_position: CssOption<BackgroundPositions> = CssOption::none() => CssOption::none(),
  background_size: CssOption<BackgroundSizes> = CssOption::none() => CssOption::none(),
  background_repeat: CssOption<BackgroundRepeats> = CssOption::none() => CssOption::none(),
  background_color: ColorInput = ColorInput::Value(Color::transparent()) => ColorInput::Value(Color::transparent()),
  box_shadow: CssOption<BoxShadows> = CssOption::none() => CssOption::none(),
  grid_auto_columns: CssOption<GridTrackSizes> = CssOption::none() => CssOption::none(),
  grid_auto_rows: CssOption<GridTrackSizes> = CssOption::none() => CssOption::none(),
  grid_auto_flow: CssOption<GridAutoFlow> = CssOption::none() => CssOption::none(),
  grid_column: CssOption<GridLine> = CssOption::none() => CssOption::none(),
  grid_row: CssOption<GridLine> = CssOption::none() => CssOption::none(),
  grid_template_columns: CssOption<GridTemplateComponents> = CssOption::none() => CssOption::none(),
  grid_template_rows: CssOption<GridTemplateComponents> = CssOption::none() => CssOption::none(),
  grid_template_areas: CssOption<GridTemplateAreas> = CssOption::none() => CssOption::none(),
  text_overflow: TextOverflow = CssValue::inherit() => Default::default(),
  text_transform: TextTransform = CssValue::inherit() => Default::default(),
  font_style: FontStyle = CssValue::inherit() => Default::default(),
  border_color: CssOption<ColorInput> = CssOption::none() => CssOption::none(),
  color: ColorInput = CssValue::inherit() => ColorInput::CurrentColor,
  filter: CssOption<Filters> = CssOption::none() => CssOption::none(),
  font_size: CssOption<LengthUnit> = CssValue::inherit() => CssOption::none(),
  font_family: CssOption<FontFamily> = CssValue::inherit() => CssOption::none(),
  line_height: LineHeight = CssValue::inherit() => Default::default(),
  font_weight: FontWeight = CssValue::inherit() => Default::default(),
  font_variation_settings: CssOption<FontVariationSettings> = CssValue::inherit() => CssOption::none(),
  font_feature_settings: CssOption<FontFeatureSettings> = CssValue::inherit() => CssOption::none(),
  line_clamp: CssOption<LineClamp> = CssValue::inherit() => CssOption::none(),
  text_align: TextAlign = CssValue::inherit() => Default::default(),
  text_stroke_width: LengthUnit = CssValue::inherit() => LengthUnit::Px(0.0),
  text_stroke_color: CssOption<ColorInput> = CssValue::inherit() => CssOption::none(),
  text_stroke: CssOption<TextStroke> = CssValue::inherit() => CssOption::none(),
  text_shadow: CssOption<TextShadows> = CssValue::inherit() => CssOption::none(),
  text_decoration: TextDecoration = TextDecoration::default() => TextDecoration::default(),
  text_decoration_line: CssOption<TextDecorationLines> = CssValue::inherit() => CssOption::none(),
  text_decoration_color: CssOption<ColorInput> = CssValue::inherit() => CssOption::none(),
  letter_spacing: CssOption<LengthUnit> = CssValue::inherit() => CssOption::none(),
  word_spacing: CssOption<LengthUnit> = CssValue::inherit() => CssOption::none(),
  image_rendering: ImageScalingAlgorithm = CssValue::inherit() => Default::default(),
  overflow_wrap: OverflowWrap = CssValue::inherit() => Default::default(),
  word_break: WordBreak = CssValue::inherit() => Default::default(),
  clip_path: CssOption<BasicShape> = CssOption::none() => CssOption::none(),
  clip_rule: FillRule = CssValue::inherit() => FillRule::NonZero,
  white_space: WhiteSpace = CssValue::inherit() => WhiteSpace::normal(),
  white_space_collapse: CssOption<WhiteSpaceCollapse> = CssValue::inherit() => CssOption::none(),
  text_wrap_mode: CssOption<TextWrapMode> = CssValue::inherit() => CssOption::none(),
  text_wrap: CssOption<TextWrapMode> = CssValue::inherit() => CssOption::none(),
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
      font_variations: style
        .parent
        .font_variation_settings
        .as_ref()
        .map(|var| FontSettings::List(Cow::Borrowed(&var.0)))
        .unwrap_or(FontSettings::List(Cow::Borrowed(&[]))),
      font_features: style
        .parent
        .font_feature_settings
        .as_ref()
        .map(|var| FontSettings::List(Cow::Borrowed(&var.0)))
        .unwrap_or(FontSettings::List(Cow::Borrowed(&[]))),
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

impl<'s> SizedFontStyle<'s> {
  pub(crate) fn ellipsis_char(&self) -> &str {
    const ELLIPSIS_CHAR: &str = "…";

    match &self.parent.text_overflow {
      TextOverflow::Ellipsis => return ELLIPSIS_CHAR,
      TextOverflow::Custom(custom) => return custom.as_str(),
      _ => {}
    }

    if let Some(clamp) = &self
      .parent
      .line_clamp
      .0
      .as_ref()
      .and_then(|clamp| clamp.ellipsis.as_deref())
    {
      return clamp;
    }

    ELLIPSIS_CHAR
  }
}

impl InheritedStyle {
  pub(crate) fn resolve_overflows(&self) -> Overflows {
    Overflows(SpacePair::from_pair(
      self.overflow_x.unwrap_or(self.overflow.0.x),
      self.overflow_y.unwrap_or(self.overflow.0.y),
    ))
  }

  pub(crate) fn white_space(&self) -> WhiteSpace {
    WhiteSpace {
      text_wrap_mode: self
        .text_wrap_mode
        .or(self.text_wrap.0)
        .unwrap_or(self.white_space.text_wrap_mode),
      white_space_collapse: self
        .white_space_collapse
        .unwrap_or(self.white_space.white_space_collapse),
    }
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
    inline: CssOption<SpacePair<LengthUnit>>,
    block: CssOption<SpacePair<LengthUnit>>,
    top: CssOption<LengthUnit>,
    right: CssOption<LengthUnit>,
    bottom: CssOption<LengthUnit>,
    left: CssOption<LengthUnit>,
  ) -> taffy::Rect<LengthUnit> {
    let mut values = base.0;

    if let Some(pair) = *inline {
      values[3] = pair.x; // left
      values[1] = pair.y; // right
    }

    if let Some(pair) = *block {
      values[0] = pair.x; // top
      values[2] = pair.y; // bottom
    }

    if let Some(v) = *top {
      values[0] = v;
    }
    if let Some(v) = *right {
      values[1] = v;
    }
    if let Some(v) = *bottom {
      values[2] = v;
    }
    if let Some(v) = *left {
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
      self.padding_inline,
      self.padding_block,
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
  fn resolved_gap(&self) -> SpacePair<LengthUnit, true> {
    SpacePair::from_pair(
      self.column_gap.unwrap_or(self.gap.y),
      self.row_gap.unwrap_or(self.gap.x),
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
  pub(crate) fn resolved_border_radius(&self) -> taffy::Rect<LengthUnit> {
    Self::resolve_rect_with_longhands(
      self.border_radius,
      CssOption::none(),
      CssOption::none(),
      self.border_top_left_radius,
      self.border_top_right_radius,
      self.border_bottom_right_radius,
      self.border_bottom_left_radius,
    )
  }

  pub(crate) fn to_sized_font_style(&'_ self, context: &RenderContext) -> SizedFontStyle<'_> {
    let line_height = self.line_height.into_parley(context);

    let resolved_stroke_width = self
      .text_stroke
      .map(|stroke| stroke.width)
      .unwrap_or(self.text_stroke_width)
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
          .0
          .iter()
          .map(|shadow| {
            SizedShadow::from_text_shadow(*shadow, context, Size::from_length(context.font_size))
          })
          .collect()
      }),
      color: self.color.resolve(context.current_color, context.opacity),
      text_stroke_color: self
        .text_stroke_color
        .or(self.text_stroke.and_then(|stroke| stroke.color))
        .unwrap_or(self.color)
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

    let overflow = self.resolve_overflows();

    taffy::style::Style {
      box_sizing: self.box_sizing.into(),
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
      aspect_ratio: self.aspect_ratio.into(),
      align_self: self.align_self.into(),
      justify_self: self.justify_self.into(),
      overflow: Point {
        x: overflow.0.x.into(),
        y: overflow.0.y.into(),
      },
      dummy: PhantomData,
      item_is_table: false,
      item_is_replaced: false,
      scrollbar_width: 0.0,
      text_align: taffy::TextAlign::Auto,
    }
  }
}
