use std::{borrow::Cow, marker::PhantomData};

use derive_builder::Builder;
use parley::{FontSettings, FontStack, TextStyle};
use serde::Deserialize;
use smallvec::SmallVec;
use taffy::{Point, Rect, Size, prelude::FromLength};

use crate::{
  layout::{
    inline::InlineBrush,
    style::{CssValue, properties::*},
  },
  rendering::{RenderContext, SizedShadow, Sizing},
};

/// Helper macro to define the `Style` struct and `InheritedStyle` struct.
macro_rules! define_style_apply_clears {
  ($self:ident, $other:ident, $trigger:ident, [$($clear:ident),* $(,)?]) => {
    if !matches!(&$other.$trigger, CssValue::Unset) {
      $(
        if matches!(&$other.$clear, CssValue::Unset) {
          $self.$clear = CssValue::Unset;
        }
      )*
    }
  };
  ($self:ident, $other:ident, $trigger:ident) => {};
}

macro_rules! define_style {
  ($(
    $(#[$attr:meta])*
    $property:ident: $type:ty
      $(where inherit = $inherit:expr)?
      $(=> [$($merge_clear:ident),* $(,)?])?,
  )*) => {
    /// Defines the style of an element.
    #[derive(Debug, Default, Clone, Deserialize, Builder, PartialEq)]
    #[serde(default, rename_all = "camelCase")]
    #[builder(default, setter(into))]
    pub struct Style {
      $(
        $(#[$attr])*
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

      /// Merges styles from another Style, where the other Style's non-Unset values take precedence.
      /// This is used to overlay higher-priority styles (e.g., inline styles) over lower-priority ones (e.g., Tailwind).
      pub(crate) fn merge_from(&mut self, other: Self) {
        $(
          define_style_apply_clears!(self, other, $property $(, [$($merge_clear),*])?);
        )*

        $(
          self.$property = other.$property.or(std::mem::take(&mut self.$property));
        )*
      }
    }

    /// A resolved set of style properties.
    #[derive(Clone, Debug, Default)]
    pub struct InheritedStyle {
      $( pub(crate) $property: $type, )*
    }

    impl InheritedStyle {
      pub(crate) fn make_computed_values(&mut self, sizing: &Sizing) {
        $(
          self.$property.make_computed(sizing);
        )*
      }
    }
  };
}

define_style!(
  // For convenience, we default to border-box
  box_sizing: BoxSizing,
  opacity: PercentageNumber,
  display: Display,
  width: Length,
  height: Length,
  max_width: Length,
  max_height: Length,
  min_width: Length,
  min_height: Length,
  aspect_ratio: AspectRatio,
  padding: Sides<Length<false>> => [
    padding_inline,
    padding_block,
    padding_top,
    padding_right,
    padding_bottom,
    padding_left,
  ],
  padding_inline: Option<SpacePair<Length<false>>> => [padding_left, padding_right],
  padding_block: Option<SpacePair<Length<false>>> => [padding_top, padding_bottom],
  padding_top: Option<Length<false>>,
  padding_right: Option<Length<false>>,
  padding_bottom: Option<Length<false>>,
  padding_left: Option<Length<false>>,
  margin: Sides<Length<false>> => [
    margin_inline,
    margin_block,
    margin_top,
    margin_right,
    margin_bottom,
    margin_left,
  ],
  margin_inline: Option<SpacePair<Length<false>>> => [margin_left, margin_right],
  margin_block: Option<SpacePair<Length<false>>> => [margin_top, margin_bottom],
  margin_top: Option<Length<false>>,
  margin_right: Option<Length<false>>,
  margin_bottom: Option<Length<false>>,
  margin_left: Option<Length<false>>,
  inset: Sides<Length> => [inset_inline, inset_block, top, right, bottom, left],
  inset_inline: Option<SpacePair<Length>> => [left, right],
  inset_block: Option<SpacePair<Length>> => [top, bottom],
  top: Option<Length>,
  right: Option<Length>,
  bottom: Option<Length>,
  left: Option<Length>,
  flex_direction: FlexDirection,
  justify_self: AlignItems,
  justify_content: JustifyContent,
  align_content: JustifyContent,
  justify_items: AlignItems,
  align_items: AlignItems,
  align_self: AlignItems,
  flex_wrap: FlexWrap,
  flex_basis: Option<Length>,
  position: Position,
  rotate: Option<Angle>,
  scale: Option<SpacePair<PercentageNumber>> => [scale_x, scale_y],
  scale_x: Option<PercentageNumber>,
  scale_y: Option<PercentageNumber>,
  transform: Option<Transforms> => [translate, rotate, scale, translate_x, translate_y, scale_x, scale_y],
  transform_origin: Option<BackgroundPosition>,
  translate: Option<SpacePair<Length>> => [translate_x, translate_y],
  translate_x: Option<Length>,
  translate_y: Option<Length>,
  mask: Backgrounds => [mask_image, mask_size, mask_position, mask_repeat],
  mask_image: Option<BackgroundImages>,
  mask_size: Option<BackgroundSizes>,
  mask_position: Option<BackgroundPositions>,
  mask_repeat: Option<BackgroundRepeats>,
  gap: Gap => [column_gap, row_gap],
  column_gap: Option<Length<false>>,
  row_gap: Option<Length<false>>,
  flex: Option<Flex> => [flex_basis, flex_grow, flex_shrink],
  flex_grow: Option<FlexGrow>,
  flex_shrink: Option<FlexGrow>,
  border_radius: BorderRadius => [
    border_top_left_radius,
    border_top_right_radius,
    border_bottom_right_radius,
    border_bottom_left_radius,
  ],
  border_top_left_radius: Option<SpacePair<Length<false>>>,
  border_top_right_radius: Option<SpacePair<Length<false>>>,
  border_bottom_right_radius: Option<SpacePair<Length<false>>>,
  border_bottom_left_radius: Option<SpacePair<Length<false>>>,
  border_width: Option<Sides<Length>> => [
    border_inline_width,
    border_block_width,
    border_top_width,
    border_right_width,
    border_bottom_width,
    border_left_width,
  ],
  border_inline_width: Option<SpacePair<Length>> => [border_left_width, border_right_width],
  border_block_width: Option<SpacePair<Length>> => [border_top_width, border_bottom_width],
  border_top_width: Option<Length>,
  border_right_width: Option<Length>,
  border_bottom_width: Option<Length>,
  border_left_width: Option<Length>,
  border_style: Option<BorderStyle>,
  border_color: Option<ColorInput>,
  border: Border => [
    border_width,
    border_inline_width,
    border_block_width,
    border_top_width,
    border_right_width,
    border_bottom_width,
    border_left_width,
    border_style,
    border_color,
  ],
  outline: Border => [outline_width, outline_style, outline_color, outline_offset],
  outline_width: Option<Length>,
  outline_style: Option<BorderStyle>,
  outline_color: Option<ColorInput>,
  outline_offset: Option<Length>,
  object_fit: ObjectFit,
  overflow: SpacePair<Overflow> => [overflow_x, overflow_y],
  overflow_x: Option<Overflow>,
  overflow_y: Option<Overflow>,
  object_position: BackgroundPosition where inherit = true,
  background: Backgrounds => [
    background_image,
    background_position,
    background_size,
    background_repeat,
    background_blend_mode,
    background_color,
    background_clip,
  ],
  background_image: Option<BackgroundImages>,
  background_position: Option<BackgroundPositions>,
  background_size: Option<BackgroundSizes>,
  background_repeat: Option<BackgroundRepeats>,
  background_blend_mode: Option<BlendModes>,
  background_color: Option<ColorInput<false>>,
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
  font_stretch: FontStretch where inherit = true,
  color: ColorInput where inherit = true,
  filter: Filters,
  backdrop_filter: Filters,
  font_size: Option<Length> where inherit = true,
  font_family: Option<FontFamily> where inherit = true,
  line_height: LineHeight where inherit = true,
  font_weight: FontWeight where inherit = true,
  font_variation_settings: Option<FontVariationSettings> where inherit = true,
  font_feature_settings: Option<FontFeatureSettings> where inherit = true,
  font_synthesis: FontSynthesis where inherit = true => [font_synthesis_weight, font_synthesis_style],
  font_synthesis_weight: Option<FontSynthesic> where inherit = true,
  font_synthesis_style: Option<FontSynthesic> where inherit = true,
  line_clamp: Option<LineClamp> where inherit = true,
  text_align: TextAlign where inherit = true,
  #[serde(rename = "WebkitTextStroke", alias = "textStroke")]
  webkit_text_stroke: Option<TextStroke> where inherit = true => [
    webkit_text_stroke_width,
    webkit_text_stroke_color,
    webkit_text_fill_color,
  ],
  #[serde(rename = "WebkitTextStrokeWidth", alias = "textStrokeWidth")]
  webkit_text_stroke_width: Option<Length<false>> where inherit = true,
  #[serde(rename = "WebkitTextStrokeColor", alias = "textStrokeColor")]
  webkit_text_stroke_color: Option<ColorInput> where inherit = true,
  #[serde(rename = "WebkitTextFillColor", alias = "textFillColor")]
  webkit_text_fill_color: Option<ColorInput> where inherit = true,
  stroke_linejoin: LineJoin where inherit = true,
  text_shadow: Option<TextShadows> where inherit = true,
  text_decoration: TextDecoration => [text_decoration_line, text_decoration_color],
  text_decoration_line: Option<TextDecorationLines> where inherit = true,
  text_decoration_color: Option<ColorInput> where inherit = true,
  text_decoration_skip_ink: TextDecorationSkipInk where inherit = true,
  letter_spacing: Option<Length> where inherit = true,
  word_spacing: Option<Length> where inherit = true,
  image_rendering: ImageScalingAlgorithm where inherit = true,
  overflow_wrap: OverflowWrap where inherit = true,
  word_break: WordBreak where inherit = true,
  clip_path: Option<BasicShape>,
  clip_rule: FillRule where inherit = true,
  white_space: WhiteSpace where inherit = true,
  white_space_collapse: Option<WhiteSpaceCollapse> where inherit = true,
  text_wrap_mode: Option<TextWrapMode> where inherit = true,
  text_wrap_style: Option<TextWrapStyle> where inherit = true,
  text_wrap: TextWrap where inherit = true => [text_wrap_mode, text_wrap_style],
  isolation: Isolation,
  mix_blend_mode: BlendMode,
  visibility: Visibility,
  vertical_align: VerticalAlign,
);

/// Sized font style with resolved font size and line height.
#[derive(Clone)]
pub(crate) struct SizedFontStyle<'s> {
  pub parent: &'s InheritedStyle,
  pub line_height: parley::LineHeight,
  pub stroke_width: f32,
  pub letter_spacing: Option<f32>,
  pub word_spacing: Option<f32>,
  pub text_shadow: Option<SmallVec<[SizedShadow; 4]>>,
  pub color: Color,
  pub text_stroke_color: Color,
  pub text_decoration_color: Color,
  pub sizing: Sizing,
}

impl<'s> From<&'s SizedFontStyle<'s>> for TextStyle<'s, InlineBrush> {
  fn from(style: &'s SizedFontStyle<'s>) -> Self {
    TextStyle {
      font_size: style.sizing.font_size,
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
        font_synthesis: FontSynthesis {
          weight: style
            .parent
            .font_synthesis_weight
            .unwrap_or(style.parent.font_synthesis.weight),
          style: style
            .parent
            .font_synthesis_style
            .unwrap_or(style.parent.font_synthesis.style),
        },
        vertical_align: style.parent.vertical_align,
      },
      text_wrap_mode: style.parent.text_wrap_mode_and_line_clamp().0.into(),
      font_width: style.parent.font_stretch.into(),

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
  /// Normalize inheritable text-related values to computed values for this node.
  pub(crate) fn make_computed(&mut self, sizing: &Sizing) {
    // `font-size` computed value is already resolved in `sizing.font_size`.
    // Keep it as css-px in style to avoid re-resolving descendant inheritance.
    let dpr = sizing.viewport.device_pixel_ratio;
    self.font_size = Some(if dpr > 0.0 {
      Length::Px(sizing.font_size / dpr)
    } else {
      Length::Px(sizing.font_size)
    });

    self.make_computed_values(sizing);
  }

  pub(crate) fn is_invisible(&self) -> bool {
    self.opacity.0 == 0.0 || self.display == Display::None || self.visibility == Visibility::Hidden
  }

  // https://developer.mozilla.org/en-US/docs/Web/CSS/Guides/Positioned_layout/Stacking_context#features_creating_stacking_contexts
  pub(crate) fn is_isolated(&self) -> bool {
    self.isolation == Isolation::Isolate
      || *self.opacity < 1.0
      || !self.filter.is_empty()
      || !self.backdrop_filter.is_empty()
      || self.mix_blend_mode != BlendMode::Normal
      || self.clip_path.is_some()
      || self
        .mask
        .iter()
        .any(|mask| !matches!(mask.image, BackgroundImage::None))
      || self.mask_image.as_ref().is_some_and(|images| {
        images
          .iter()
          .any(|image| !matches!(image, BackgroundImage::None))
      })
  }

  pub(crate) fn has_non_identity_transform(&self, border_box: Size<f32>, sizing: &Sizing) -> bool {
    let transform_origin = self.transform_origin.unwrap_or_default();
    let origin = transform_origin.to_point(sizing, border_box);

    let mut local = Affine::translation(origin.x, origin.y);

    let translate = self.translate();
    if translate != SpacePair::default() {
      local *= Affine::translation(
        translate.x.to_px(sizing, border_box.width),
        translate.y.to_px(sizing, border_box.height),
      );
    }

    if let Some(rotate) = self.rotate {
      local *= Affine::rotation(rotate);
    }

    let scale = self.scale();
    if scale != SpacePair::default() {
      local *= Affine::scale(scale.x.0, scale.y.0);
    }

    if let Some(node_transform) = &self.transform {
      local *= Affine::from_transforms(node_transform.iter(), sizing, border_box);
    }

    local *= Affine::translation(-origin.x, -origin.y);

    !local.is_identity()
  }

  pub(crate) fn resolve_overflows(&self) -> SpacePair<Overflow> {
    SpacePair::from_pair(
      self.overflow_x.unwrap_or(self.overflow.x),
      self.overflow_y.unwrap_or(self.overflow.y),
    )
  }

  pub(crate) fn translate(&self) -> SpacePair<Length> {
    SpacePair::from_pair(
      self
        .translate_x
        .unwrap_or(self.translate.unwrap_or_default().x),
      self
        .translate_y
        .unwrap_or(self.translate.unwrap_or_default().y),
    )
  }

  pub(crate) fn scale(&self) -> SpacePair<PercentageNumber> {
    SpacePair::from_pair(
      self.scale_x.unwrap_or(self.scale.unwrap_or_default().x),
      self.scale_y.unwrap_or(self.scale.unwrap_or_default().y),
    )
  }

  pub(crate) fn background_color(&self) -> ColorInput<false> {
    if let Some(color) = self.background_color {
      return color;
    }

    self
      .background
      .iter()
      .filter_map(|bg| bg.color)
      .next_back()
      .unwrap_or_default()
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

  pub(crate) fn white_space_collapse(&self) -> WhiteSpaceCollapse {
    self
      .white_space_collapse
      .unwrap_or(self.white_space.white_space_collapse)
  }

  pub(crate) fn text_wrap_mode_and_line_clamp(&self) -> (TextWrapMode, Option<Cow<'_, LineClamp>>) {
    let mut text_wrap_mode = self
      .text_wrap_mode
      .or(self.text_wrap.mode)
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
    sizing: &Sizing,
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
              track_size.to_min_max(sizing),
            ));
          }
          GridTemplateComponent::Repeat(repetition, tracks) => {
            // Push names for the line preceding this repeat fragment
            line_name_sets.push(std::mem::take(&mut pending_line_names));

            // Build repetition
            let track_sizes: Vec<taffy::TrackSizingFunction> =
              tracks.iter().map(|t| t.size.to_min_max(sizing)).collect();

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
  pub(crate) fn resolved_padding(&self) -> taffy::Rect<Length<false>> {
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
  pub(crate) fn resolved_margin(&self) -> taffy::Rect<Length<false>> {
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
  fn resolved_inset(&self) -> taffy::Rect<Length> {
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
  fn resolved_gap(&self) -> SpacePair<Length<false>> {
    SpacePair::from_pair(
      self.row_gap.unwrap_or(self.gap.x),
      self.column_gap.unwrap_or(self.gap.y),
    )
  }

  #[inline]
  pub(crate) fn resolved_border_width(&self) -> taffy::Rect<Length> {
    Self::resolve_rect_with_longhands(
      self
        .border_width
        .unwrap_or_else(|| self.border.width.into()),
      self.border_inline_width,
      self.border_block_width,
      self.border_top_width,
      self.border_right_width,
      self.border_bottom_width,
      self.border_left_width,
    )
  }

  #[inline]
  pub(crate) fn resolved_border_radius(&self) -> taffy::Rect<SpacePair<Length<false>>> {
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
    let line_height = self.line_height.into_parley(&context.sizing);

    let resolved_stroke_width = self
      .webkit_text_stroke_width
      .or(self.webkit_text_stroke.map(|stroke| stroke.width))
      .unwrap_or_default()
      .to_px(&context.sizing, context.sizing.font_size);

    SizedFontStyle {
      sizing: context.sizing.clone(),
      parent: self,
      line_height,
      stroke_width: resolved_stroke_width,
      letter_spacing: self
        .letter_spacing
        .map(|spacing| spacing.to_px(&context.sizing, context.sizing.font_size)),
      word_spacing: self
        .word_spacing
        .map(|spacing| spacing.to_px(&context.sizing, context.sizing.font_size)),
      text_shadow: self.text_shadow.as_ref().map(|shadows| {
        shadows
          .iter()
          .map(|shadow| {
            SizedShadow::from_text_shadow(
              *shadow,
              &context.sizing,
              context.current_color,
              Size::from_length(context.sizing.font_size),
            )
          })
          .collect()
      }),
      color: self
        .webkit_text_fill_color
        .unwrap_or(self.color)
        .resolve(context.current_color),
      text_stroke_color: self
        .webkit_text_stroke_color
        .or(self.webkit_text_stroke.and_then(|stroke| stroke.color))
        .unwrap_or_default()
        .resolve(context.current_color),
      text_decoration_color: self
        .text_decoration_color
        .or(self.text_decoration.color)
        .unwrap_or(ColorInput::CurrentColor)
        .resolve(context.current_color),
    }
  }

  pub(crate) fn to_taffy_style(&self, context: &RenderContext) -> taffy::Style {
    // Convert grid templates and associated line names
    let (grid_template_columns, grid_template_column_names) =
      Self::convert_template_components(&self.grid_template_columns, &context.sizing);
    let (grid_template_rows, grid_template_row_names) =
      Self::convert_template_components(&self.grid_template_rows, &context.sizing);

    let border_style = self.border_style.unwrap_or(self.border.style);

    taffy::Style {
      box_sizing: self.box_sizing.into(),
      size: Size {
        width: self.width.resolve_to_dimension(&context.sizing),
        height: self.height.resolve_to_dimension(&context.sizing),
      },
      border: if border_style == BorderStyle::None {
        Rect::zero()
      } else {
        self
          .resolved_border_width()
          .map(|border| border.resolve_to_length_percentage(&context.sizing))
      },
      padding: self
        .resolved_padding()
        .map(|padding| padding.resolve_to_length_percentage(&context.sizing)),
      inset: self
        .resolved_inset()
        .map(|inset| inset.resolve_to_length_percentage_auto(&context.sizing)),
      margin: self
        .resolved_margin()
        .map(|margin| margin.resolve_to_length_percentage_auto(&context.sizing)),
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
      gap: self.resolved_gap().resolve_to_size(&context.sizing),
      flex_basis: self
        .flex_basis
        .or_else(|| self.flex.map(|flex| flex.basis))
        .unwrap_or(Length::Auto)
        .resolve_to_dimension(&context.sizing),
      flex_shrink: self
        .flex_shrink
        .map(|shrink| shrink.0)
        .or_else(|| self.flex.map(|flex| flex.shrink))
        .unwrap_or(1.0),
      flex_wrap: self.flex_wrap.into(),
      min_size: Size {
        width: self.min_width.resolve_to_dimension(&context.sizing),
        height: self.min_height.resolve_to_dimension(&context.sizing),
      },
      max_size: Size {
        width: self.max_width.resolve_to_dimension(&context.sizing),
        height: self.max_height.resolve_to_dimension(&context.sizing),
      },
      grid_auto_columns: self.grid_auto_columns.as_ref().map_or_else(Vec::new, |v| {
        v.iter().map(|s| s.to_min_max(&context.sizing)).collect()
      }),
      grid_auto_rows: self.grid_auto_rows.as_ref().map_or_else(Vec::new, |v| {
        v.iter().map(|s| s.to_min_max(&context.sizing)).collect()
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

#[cfg(test)]
mod tests {
  use std::sync::Arc;

  use taffy::Size;

  use crate::{
    layout::{
      Viewport,
      style::{CssValue, InheritedStyle, Style, properties::*},
    },
    rendering::Sizing,
  };

  #[test]
  fn test_merge_from_inline_over_tailwind() {
    // Tailwind style (lower priority)
    let mut tw_style = Style {
      width: CssValue::Value(Length::Rem(10.0)),
      height: CssValue::Value(Length::Rem(20.0)),
      color: CssValue::Value(ColorInput::Value(Color([255, 0, 0, 255]))), // red
      ..Default::default()
    };

    // Inline style (higher priority) - only sets width
    // height is Unset
    // color is Unset
    let inline_style = Style {
      width: CssValue::Value(Length::Px(100.0)),
      ..Default::default()
    };

    // Merge: inline_style should override tw_style's width, but keep height and color
    tw_style.merge_from(inline_style);

    // Check results
    assert_eq!(tw_style.width, CssValue::Value(Length::Px(100.0))); // from inline
    assert_eq!(tw_style.height, CssValue::Value(Length::Rem(20.0))); // from tw
    assert_eq!(
      tw_style.color,
      CssValue::Value(ColorInput::Value(Color([255, 0, 0, 255])))
    ); // from tw
  }

  #[test]
  fn test_merge_from_margin_shorthand_clears_lower_priority_longhands() {
    let mut preset_style = Style {
      margin_top: Some(Length::Em(0.67)).into(),
      margin_bottom: Some(Length::Em(0.67)).into(),
      margin_left: Some(Length::Px(0.0)).into(),
      margin_right: Some(Length::Px(0.0)).into(),
      ..Default::default()
    };

    let inline_style = Style {
      margin: Sides([Length::Px(0.0); 4]).into(),
      ..Default::default()
    };

    preset_style.merge_from(inline_style);

    let inherited = preset_style.inherit(&InheritedStyle::default());
    let resolved = inherited.resolved_margin();
    assert_eq!(resolved.top, Length::Px(0.0));
    assert_eq!(resolved.right, Length::Px(0.0));
    assert_eq!(resolved.bottom, Length::Px(0.0));
    assert_eq!(resolved.left, Length::Px(0.0));
  }

  #[test]
  fn test_merge_from_margin_longhand_still_overrides_shorthand_in_same_layer() {
    let mut preset_style = Style {
      margin_top: Some(Length::Em(0.67)).into(),
      margin_bottom: Some(Length::Em(0.67)).into(),
      ..Default::default()
    };

    let inline_style = Style {
      margin: Sides([Length::Px(0.0); 4]).into(),
      margin_top: Some(Length::Px(8.0)).into(),
      ..Default::default()
    };

    preset_style.merge_from(inline_style);

    let inherited = preset_style.inherit(&InheritedStyle::default());
    let resolved = inherited.resolved_margin();
    assert_eq!(resolved.top, Length::Px(8.0));
    assert_eq!(resolved.right, Length::Px(0.0));
    assert_eq!(resolved.bottom, Length::Px(0.0));
    assert_eq!(resolved.left, Length::Px(0.0));
  }

  #[test]
  fn test_merge_from_text_decoration_shorthand_clears_lower_priority_color() {
    let mut preset_style = Style {
      text_decoration_color: Some(ColorInput::Value(Color([255, 0, 0, 255]))).into(),
      ..Default::default()
    };

    let inline_style = Style {
      text_decoration: TextDecoration {
        line: TextDecorationLines::UNDERLINE,
        style: None,
        color: None,
      }
      .into(),
      ..Default::default()
    };

    preset_style.merge_from(inline_style);

    let inherited = preset_style.inherit(&InheritedStyle::default());
    assert_eq!(inherited.text_decoration_color, None);
    assert_eq!(
      inherited.text_decoration.line,
      TextDecorationLines::UNDERLINE
    );
  }

  #[test]
  fn test_merge_from_border_shorthand_clears_lower_priority_border_width_longhands() {
    let mut preset_style = Style {
      border_top_width: Some(Length::Px(8.0)).into(),
      border_bottom_width: Some(Length::Px(8.0)).into(),
      ..Default::default()
    };

    let inline_style = Style {
      border: Border {
        width: Length::Px(2.0),
        style: BorderStyle::Solid,
        color: ColorInput::CurrentColor,
      }
      .into(),
      ..Default::default()
    };

    preset_style.merge_from(inline_style);

    let inherited = preset_style.inherit(&InheritedStyle::default());
    let resolved = inherited.resolved_border_width();
    assert_eq!(resolved.top, Length::Px(2.0));
    assert_eq!(resolved.right, Length::Px(2.0));
    assert_eq!(resolved.bottom, Length::Px(2.0));
    assert_eq!(resolved.left, Length::Px(2.0));
  }

  #[test]
  fn test_merge_from_background_shorthand_clears_lower_priority_background_color() {
    let mut preset_style = Style {
      background_color: Some(ColorInput::Value(Color([255, 0, 0, 255]))).into(),
      ..Default::default()
    };

    let inline_style = Style {
      background: [Background::default()].into(),
      ..Default::default()
    };

    preset_style.merge_from(inline_style);

    let inherited = preset_style.inherit(&InheritedStyle::default());
    assert_eq!(inherited.background_color, None);
  }

  #[test]
  fn test_unset_follows_default_inherit_flag() {
    // Non-inheriting property (DEFAULT_INHERIT = false)
    let unset_width: CssValue<Length, false> = CssValue::Unset;
    let result = unset_width.inherit_value(&Length::Px(100.0));
    assert_eq!(result, Length::Auto); // Should use default (Auto), not inherit

    // Inheriting property (DEFAULT_INHERIT = true)
    let unset_color: CssValue<ColorInput, true> = CssValue::Unset;
    let parent_color = ColorInput::Value(Color([255, 0, 0, 255]));
    let result = unset_color.inherit_value(&parent_color);
    assert_eq!(result, parent_color); // Should inherit from parent
  }

  #[test]
  fn test_or_method() {
    let high_priority = CssValue::Value(Length::Px(100.0));
    let low_priority = CssValue::Value(Length::Rem(10.0));
    let unset: CssValue<Length> = CssValue::Unset;

    // High priority value should be kept
    assert_eq!(high_priority.or(low_priority), high_priority);

    // Unset should fallback to low priority
    assert_eq!(unset.or(low_priority), low_priority);

    // Initial/Inherit should be kept even when or-ing with Value
    let initial: CssValue<Length> = CssValue::Initial;
    assert_eq!(initial.or(low_priority), initial);

    let inherit: CssValue<Length> = CssValue::Inherit;
    assert_eq!(inherit.or(low_priority), inherit);
  }

  #[test]
  fn test_resolve_padding_precedence() {
    let inherited = Style {
      padding: Sides([
        Length::Px(1.0),
        Length::Px(2.0),
        Length::Px(3.0),
        Length::Px(4.0),
      ])
      .into(),
      padding_inline: Some(SpacePair::from_pair(Length::Px(10.0), Length::Px(20.0))).into(),
      padding_block: Some(SpacePair::from_pair(Length::Px(30.0), Length::Px(40.0))).into(),
      padding_left: Some(Length::Px(50.0)).into(),
      ..Default::default()
    }
    .inherit(&InheritedStyle::default());

    let resolved = inherited.resolved_padding();

    assert_eq!(resolved.top, Length::Px(30.0));
    assert_eq!(resolved.right, Length::Px(20.0));
    assert_eq!(resolved.bottom, Length::Px(40.0));
    assert_eq!(resolved.left, Length::Px(50.0));
  }

  #[test]
  fn test_resolve_border_width_precedence() {
    let inherited = Style {
      border: Border {
        width: Length::Px(1.0),
        style: BorderStyle::None,
        color: ColorInput::CurrentColor,
      }
      .into(),
      border_inline_width: Some(SpacePair::from_pair(Length::Px(2.0), Length::Px(3.0))).into(),
      border_top_width: Some(Length::Px(4.0)).into(),
      ..Default::default()
    }
    .inherit(&InheritedStyle::default());

    let resolved = inherited.resolved_border_width();

    assert_eq!(resolved.top, Length::Px(4.0));
    assert_eq!(resolved.right, Length::Px(3.0));
    assert_eq!(resolved.bottom, Length::Px(1.0));
    assert_eq!(resolved.left, Length::Px(2.0));
  }

  #[test]
  fn test_isolated_for_clip_path_and_mask_image() {
    let mut style = InheritedStyle::default();
    assert!(!style.is_isolated());

    style.clip_path = BasicShape::from_str("inset(10px)").ok();
    assert!(style.is_isolated());

    style.clip_path = None;
    style.mask_image =
      Some(vec![BackgroundImage::Url("https://example.com/mask.png".into())].into_boxed_slice());
    assert!(style.is_isolated());
  }

  #[test]
  fn test_non_identity_transform_detection() {
    let mut style = InheritedStyle::default();
    let sizing = Sizing {
      viewport: Viewport::new(Some(1200), Some(630)),
      font_size: 16.0,
      calc_arena: Arc::new(CalcArena::default()),
    };
    let border_box = Size {
      width: 200.0,
      height: 100.0,
    };

    assert!(!style.has_non_identity_transform(border_box, &sizing));

    style.transform = Some(vec![Transform::Rotate(Angle::new(0.0))].into_boxed_slice());
    assert!(!style.has_non_identity_transform(border_box, &sizing));

    style.transform = Some(vec![Transform::Rotate(Angle::new(10.0))].into_boxed_slice());
    assert!(style.has_non_identity_transform(border_box, &sizing));
  }

  #[test]
  fn test_text_overflow_ellipsis_forces_single_line_clamp_on_nowrap() {
    let style = InheritedStyle {
      text_wrap_mode: Some(TextWrapMode::NoWrap),
      text_overflow: TextOverflow::Ellipsis,
      ..Default::default()
    };

    let (text_wrap_mode, line_clamp) = style.text_wrap_mode_and_line_clamp();

    assert_eq!(text_wrap_mode, TextWrapMode::Wrap);
    assert_eq!(
      line_clamp,
      Some(std::borrow::Cow::Owned(LineClamp {
        count: 1,
        ellipsis: Some("…".to_string()),
      }))
    );
  }

  #[test]
  fn test_inherited_em_text_lengths_are_computed_once() {
    let mut parent = Style {
      font_size: Some(Length::Em(2.0)).into(),
      letter_spacing: Some(Length::Em(1.0)).into(),
      line_height: LineHeight::Length(Length::Em(1.5)).into(),
      ..Default::default()
    }
    .inherit(&InheritedStyle::default());
    parent.make_computed(&Sizing {
      viewport: Viewport::new(Some(1200), Some(630)),
      font_size: 32.0,
      calc_arena: Arc::new(CalcArena::default()),
    });

    let inherited_child = Style::default().inherit(&parent);
    let inherited_child_sizing = Sizing {
      viewport: Viewport::new(Some(1200), Some(630)),
      font_size: 32.0,
      calc_arena: Arc::new(CalcArena::default()),
    };
    let inherited_font_size = inherited_child
      .font_size
      .map(|size| size.to_px(&inherited_child_sizing, inherited_child_sizing.font_size))
      .unwrap_or_default();
    assert_eq!(inherited_font_size, 32.0);

    let child_with_own_font_size = Style {
      font_size: Some(Length::Px(10.0)).into(),
      ..Default::default()
    }
    .inherit(&parent);
    let child_sizing = Sizing {
      viewport: Viewport::new(Some(1200), Some(630)),
      font_size: 10.0,
      calc_arena: Arc::new(CalcArena::default()),
    };

    let inherited_letter_spacing = child_with_own_font_size
      .letter_spacing
      .map(|v| v.to_px(&child_sizing, child_sizing.font_size))
      .unwrap_or_default();
    assert_eq!(inherited_letter_spacing, 32.0);

    let inherited_line_height = match child_with_own_font_size.line_height {
      LineHeight::Length(length) => length.to_px(&child_sizing, child_sizing.font_size),
      _ => 0.0,
    };
    assert_eq!(inherited_line_height, 48.0);
  }
}
