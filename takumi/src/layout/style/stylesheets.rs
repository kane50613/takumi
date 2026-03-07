use std::{borrow::Cow, collections::BTreeSet, marker::PhantomData};

use parley::{FontSettings, FontStack, TextStyle};
use paste::paste;
use serde::de::IgnoredAny;
use smallvec::SmallVec;
use taffy::{Point, Rect, Size, prelude::FromLength};

use crate::{
  layout::{
    inline::InlineBrush,
    style::{RawCssInput, RawCssValueSeed, properties::*},
  },
  rendering::{RenderContext, SizedShadow, Sizing},
};

macro_rules! define_inherited_default {
  ($parent:expr, $inherit:tt) => {
    $parent.clone()
  };
  ($parent:expr) => {
    Default::default()
  };
}

enum ParsedRawStyleValue<T> {
  Keyword(CssWideKeyword),
  Value(T),
}

fn parse_css_wide_keyword(value: &str) -> Option<CssWideKeyword> {
  if value.eq_ignore_ascii_case("initial") {
    Some(CssWideKeyword::Initial)
  } else if value.eq_ignore_ascii_case("inherit") {
    Some(CssWideKeyword::Inherit)
  } else if value.eq_ignore_ascii_case("unset") {
    Some(CssWideKeyword::Unset)
  } else {
    None
  }
}

fn parse_raw_style_value<'de, T, E>(
  raw_value: RawCssInput<'de>,
) -> Result<ParsedRawStyleValue<T>, E>
where
  T: for<'i> FromCss<'i>,
  E: serde::de::Error,
{
  match raw_value {
    RawCssInput::Str(value) => {
      if let Some(keyword) = parse_css_wide_keyword(value.as_ref()) {
        return Ok(ParsedRawStyleValue::Keyword(keyword));
      }

      T::from_str(value.as_ref())
        .map(ParsedRawStyleValue::Value)
        .map_err(|_| {
          E::invalid_value(
            serde::de::Unexpected::Str(value.as_ref()),
            &super::css_expected_message::<T>(),
          )
        })
    }
    RawCssInput::Number(number) => {
      let source = number.to_string();
      T::from_str(&source)
        .map(ParsedRawStyleValue::Value)
        .map_err(|_| E::invalid_type(number.unexpected(), &super::css_expected_message::<T>()))
    }
    RawCssInput::Unexpected(unexpected) => {
      unexpected.as_invalid_type::<T, E, ParsedRawStyleValue<T>>()
    }
  }
}

macro_rules! push_expanded_declarations {
  ($target:expr, $important:expr; $($declaration:expr),+ $(,)?) => {{
    let _ = $important;
    $(
      $target.push($declaration);
    )+
  }};
}

macro_rules! define_style {
  (
    longhands {
      $(
        $longhand:ident: $longhand_ty:ty
          $(where inherit = $longhand_inherit:expr)?,
      )*
    }
    shorthands {
      $(
        $shorthand:ident: $shorthand_ty:ty
          $(where inherit = $shorthand_inherit:expr)?
          => [$($target:ident),+ $(,)?]
          |$value:ident, $target_var:ident, $important_var:ident|
          $expand:block,
      )*
    }
  ) => {
    paste! {
      #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
      pub(crate) enum LonghandId {
        $([<$longhand:camel>],)*
      }

      #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
      pub(crate) enum ShorthandId {
        $([<$shorthand:camel>],)*
      }

      #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
      pub(crate) enum PropertyId {
        Ignored,
        Longhand(LonghandId),
        Shorthand(ShorthandId),
      }

      impl LonghandId {
        fn from_normalized_name(name: &str) -> Option<Self> {
          match name {
            $(stringify!($longhand) => Some(Self::[<$longhand:camel>]),)*
            _ => None,
          }
        }
      }

      impl ShorthandId {
        fn from_normalized_name(name: &str) -> Option<Self> {
          match name {
            $(stringify!($shorthand) => Some(Self::[<$shorthand:camel>]),)*
            _ => None,
          }
        }
      }

      impl PropertyId {
        fn from_alias(name: &str) -> Option<Self> {
          match name {
            "-webkit-text-stroke" => Some(Self::Shorthand(ShorthandId::WebkitTextStroke)),
            "-webkit-text-stroke-width" => {
              Some(Self::Longhand(LonghandId::WebkitTextStrokeWidth))
            }
            "-webkit-text-stroke-color" => {
              Some(Self::Longhand(LonghandId::WebkitTextStrokeColor))
            }
            "-webkit-text-fill-color" => Some(Self::Longhand(LonghandId::WebkitTextFillColor)),
            _ => None,
          }
        }

        fn from_normalized_name(name: &str) -> Self {
          if let Some(property) = LonghandId::from_normalized_name(name) {
            return Self::Longhand(property);
          }
          if let Some(property) = ShorthandId::from_normalized_name(name) {
            return Self::Shorthand(property);
          }
          Self::Ignored
        }

        fn from_kebab_case(name: &str) -> Self {
          if name.starts_with("--") {
            return Self::Ignored;
          }
          if let Some(property) = Self::from_alias(name) {
            return property;
          }

          let normalized = name
            .chars()
            .map(|ch| match ch {
              '-' => '_',
              _ => ch.to_ascii_lowercase(),
            })
            .collect::<String>();
          Self::from_normalized_name(&normalized)
        }

        #[allow(dead_code)]
        pub(crate) fn from_camel_case(name: &str) -> Self {
          match name {
            "textStroke" | "WebkitTextStroke" => {
              return Self::Shorthand(ShorthandId::WebkitTextStroke);
            }
            "textStrokeWidth" | "WebkitTextStrokeWidth" => {
              return Self::Longhand(LonghandId::WebkitTextStrokeWidth);
            }
            "textStrokeColor" | "WebkitTextStrokeColor" => {
              return Self::Longhand(LonghandId::WebkitTextStrokeColor);
            }
            "textFillColor" | "WebkitTextFillColor" => {
              return Self::Longhand(LonghandId::WebkitTextFillColor);
            }
            _ => {}
          }

          let mut normalized = String::with_capacity(name.len() + 4);
          for ch in name.chars() {
            if ch.is_ascii_uppercase() {
              normalized.push('_');
              normalized.push(ch.to_ascii_lowercase());
            } else {
              normalized.push(ch);
            }
          }

          Self::from_normalized_name(normalized.trim_start_matches('_'))
        }
      }

      fn parse_style_declaration<'i>(
        name: &str,
        input: &mut cssparser::Parser<'i, '_>,
      ) -> Result<StyleDeclarationBlock, cssparser::ParseError<'i, Cow<'i, str>>> {
        let property = PropertyId::from_kebab_case(name);
        let mut declarations = StyleDeclarationBlock::default();

        match property {
          PropertyId::Ignored => {
            while input.next_including_whitespace_and_comments().is_ok() {}
            Ok(declarations)
          }
          PropertyId::Shorthand(property) => {
            for declaration in parse_shorthand_declarations(property, input)? {
              declarations.push(declaration, false);
            }
            Ok(declarations)
          }
          PropertyId::Longhand(property) => match property {
            $(
              LonghandId::[<$longhand:camel>] => {
                let state = input.state();
                let keyword = input
                  .try_parse(cssparser::Parser::expect_ident_cloned)
                  .ok()
                  .and_then(|ident| parse_css_wide_keyword(ident.as_ref()));
                if let Some(keyword) = keyword {
                  declarations.push(
                    StyleDeclaration::CssWideKeyword(LonghandId::[<$longhand:camel>], keyword),
                    false,
                  );
                } else {
                  input.reset(&state);
                  declarations.push(
                    StyleDeclaration::[<$longhand:camel>](<$longhand_ty as FromCss>::from_css(input)?),
                    false,
                  );
                }
                Ok(declarations)
              }
            )*
          },
        }
      }

      /// Defines the style of an element.
      #[derive(Debug, Default, Clone, PartialEq)]
      pub struct Style {
        pub(crate) declarations: StyleDeclarationBlock,
      }

      impl<'de> serde::Deserialize<'de> for Style {
        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
          D: serde::Deserializer<'de>,
        {
          struct StyleVisitor;

          impl<'de> serde::de::Visitor<'de> for StyleVisitor {
            type Value = Style;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
              formatter.write_str("a style object")
            }

            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where
              A: serde::de::MapAccess<'de>,
            {
              let mut style = Style::default();

              while let Some(key) = map.next_key::<Cow<'de, str>>()? {
                let property = PropertyId::from_camel_case(&key);
                if matches!(property, PropertyId::Ignored) {
                  map.next_value::<IgnoredAny>()?;
                  continue;
                }

                let raw_value = map.next_value_seed(RawCssValueSeed)?;
                style.push_property_from_raw::<A::Error>(property, raw_value, false)?;
              }

              Ok(style)
            }
          }

          deserializer.deserialize_map(StyleVisitor)
        }
      }

      impl Style {
        fn with_importance(mut self, declaration: StyleDeclaration, important: bool) -> Self {
          self.push(declaration, important);
          self
        }

        fn with_shorthand_importance(
          mut self,
          declarations: Vec<StyleDeclaration>,
          important: bool,
        ) -> Self {
          for declaration in declarations {
            self.push(declaration, important);
          }
          self
        }

        /// Returns a new style with one declaration appended in source order.
        pub fn with(self, declaration: StyleDeclaration) -> Self {
          self.with_importance(declaration, false)
        }

        $(
          /// Returns a new style with this shorthand expanded and appended in source order.
          pub fn [<with_ $shorthand>](self, value: $shorthand_ty) -> Self {
            let mut declarations = Vec::new();
            let $target_var = &mut declarations;
            let $important_var = false;
            let $value = value;
            $expand
            self.with_shorthand_importance(declarations, false)
          }
        )*

        /// Returns a new style with one `!important` declaration appended in source order.
        pub fn with_important(self, declaration: StyleDeclaration) -> Self {
          self.with_importance(declaration, true)
        }

        pub(crate) fn push(&mut self, declaration: StyleDeclaration, important: bool) {
          declaration.append_to_block(&mut self.declarations, important);
        }

        pub(crate) fn append_block(&mut self, declarations: StyleDeclarationBlock) {
          self.declarations.append(declarations);
        }

        pub(crate) fn iter(&self) -> std::slice::Iter<'_, StyleDeclaration> {
          self.declarations.iter()
        }

        pub(crate) fn inherit(self, parent: &ResolvedStyle) -> ResolvedStyle {
          let mut style = ResolvedStyle::from_parent(parent);
          for declaration in self.declarations.iter() {
            declaration.apply_to_computed_with_parent(&mut style, parent);
          }
          style
        }

        pub(crate) fn merge_from(&mut self, other: Self) {
          self.append_block(other.declarations);
        }

        #[inline(never)]
        fn push_property_from_raw<'de, E>(
          &mut self,
          property: PropertyId,
          raw_value: RawCssInput<'de>,
          important: bool,
        ) -> Result<(), E>
        where
          E: serde::de::Error,
        {
          match property {
            PropertyId::Ignored => Ok(()),
            PropertyId::Shorthand(property) => {
              for declaration in parse_shorthand_declarations_from_raw(property, raw_value)? {
                self.push(declaration, important);
              }
              Ok(())
            }
            PropertyId::Longhand(property) => match property {
              $(
                LonghandId::[<$longhand:camel>] => {
                  match parse_raw_style_value::<$longhand_ty, E>(raw_value)? {
                    ParsedRawStyleValue::Keyword(keyword) => {
                      self.push(
                        StyleDeclaration::CssWideKeyword(LonghandId::[<$longhand:camel>], keyword),
                        important,
                      );
                    }
                    ParsedRawStyleValue::Value(value) => {
                      self.push(StyleDeclaration::[<$longhand:camel>](value), important);
                    }
                  }
                  Ok(())
                }
              )*
            },
          }
        }
      }

      impl From<StyleDeclarationBlock> for Style {
        fn from(declarations: StyleDeclarationBlock) -> Self {
          Self { declarations }
        }
      }

      /// A resolved set of style properties.
      #[derive(Clone, Debug, Default)]
      pub struct ComputedStyle {
        $(pub(crate) $longhand: $longhand_ty,)*
      }

      /// A single specified declaration stored in a declaration block.
      #[allow(private_interfaces)]
      #[derive(Debug, Clone, PartialEq)]
      pub enum StyleDeclaration {
        $(
          /// An explicit specified value for a non-shorthand property.
          [<$longhand:camel>]($longhand_ty),
        )*
        /// A CSS-wide keyword targeting a longhand property.
        CssWideKeyword(LonghandId, CssWideKeyword),
      }

      impl ComputedStyle {
        pub(crate) fn from_parent(parent: &Self) -> Self {
          Self {
            $($longhand: define_inherited_default!(parent.$longhand $(, $longhand_inherit)?),)*
          }
        }

        pub(crate) fn make_computed_values(&mut self, sizing: &Sizing) {
          $(self.$longhand.make_computed(sizing);)*
        }

        #[cfg(feature = "css_stylesheet_parsing")]
        pub(crate) fn apply_interpolated_properties(
          &mut self,
          from: &Self,
          to: &Self,
          animated_properties: &PropertyMask,
          progress: f32,
          sizing: &Sizing,
          current_color: Color,
        ) {
          $(
            if animated_properties.contains(&LonghandId::[<$longhand:camel>]) {
              self.$longhand.interpolate(
                &from.$longhand,
                &to.$longhand,
                progress,
                sizing,
                current_color,
              );
            }
          )*
        }
      }

      impl StyleDeclaration {
        $(
          /// Returns a declaration for this property.
          pub fn $longhand(value: $longhand_ty) -> Self {
            Self::[<$longhand:camel>](value)
          }
        )*

        fn append_to_block(&self, block: &mut StyleDeclarationBlock, important: bool) {
          block.push(self.clone(), important);
        }

        pub(crate) fn longhand_id(&self) -> LonghandId {
          match self {
            $(Self::[<$longhand:camel>](..) => LonghandId::[<$longhand:camel>],)*
            Self::CssWideKeyword(id, _) => *id,
          }
        }

        #[inline(never)]
        pub(crate) fn apply_to_computed(&self, style: &mut ComputedStyle) {
          match self {
            Self::CssWideKeyword(property, keyword) => match keyword {
              CssWideKeyword::Initial => apply_initial_longhand(style, *property),
              CssWideKeyword::Inherit | CssWideKeyword::Unset => {}
            },
            $(Self::[<$longhand:camel>](value) => style.$longhand = value.clone(),)*
          }
        }

        #[inline(never)]
        pub(crate) fn apply_to_computed_with_parent(
          &self,
          style: &mut ComputedStyle,
          parent: &ComputedStyle,
        ) {
          match self {
            Self::CssWideKeyword(property, keyword) => {
              apply_css_wide_keyword(style, parent, *property, *keyword)
            }
            $(Self::[<$longhand:camel>](value) => style.$longhand = value.clone(),)*
          }
        }

        pub(crate) fn apply_to_resolved(&self, style: &mut ComputedStyle) {
          self.apply_to_computed(style);
        }

        pub(crate) fn merge_into(&self, style: &mut Style) {
          style.push(self.clone(), false);
        }
      }

      #[inline(never)]
      fn apply_initial_longhand(style: &mut ComputedStyle, property: LonghandId) {
        match property {
          $(
            LonghandId::[<$longhand:camel>] => {
              style.$longhand = Default::default();
            }
          )*
        }
      }

      #[inline(never)]
      fn apply_css_wide_keyword(
        style: &mut ComputedStyle,
        parent: &ComputedStyle,
        property: LonghandId,
        keyword: CssWideKeyword,
      ) {
        match property {
          $(
            LonghandId::[<$longhand:camel>] => {
              style.$longhand = match keyword {
                CssWideKeyword::Initial => Default::default(),
                CssWideKeyword::Inherit => parent.$longhand.clone(),
                CssWideKeyword::Unset => define_inherited_default!(parent.$longhand $(, $longhand_inherit)?),
              };
            }
          )*
        }
      }

      fn parse_shorthand_declarations<'i>(
        property: ShorthandId,
        input: &mut cssparser::Parser<'i, '_>,
      ) -> Result<Vec<StyleDeclaration>, cssparser::ParseError<'i, Cow<'i, str>>> {
        match property {
          $(
            ShorthandId::[<$shorthand:camel>] => {
              let mut declarations = Vec::new();
              let $target_var = &mut declarations;
              let $important_var = false;
              let $value = <$shorthand_ty as FromCss>::from_css(input)?;
              $expand
              Ok(declarations)
            }
          )*
        }
      }

      fn parse_shorthand_declarations_from_raw<'de, E>(
        property: ShorthandId,
        raw_value: RawCssInput<'de>,
      ) -> Result<Vec<StyleDeclaration>, E>
      where
        E: serde::de::Error,
      {
        match property {
          $(
            ShorthandId::[<$shorthand:camel>] => {
              match parse_raw_style_value::<$shorthand_ty, E>(raw_value)? {
                ParsedRawStyleValue::Keyword(keyword) => {
                  Ok(vec![
                    $(StyleDeclaration::CssWideKeyword(LonghandId::$target, keyword)),+
                  ])
                }
                ParsedRawStyleValue::Value(value) => {
                  let mut declarations = Vec::new();
                  let $target_var = &mut declarations;
                  let $important_var = false;
                  let $value = value;
                  $expand
                  Ok(declarations)
                }
              }
            }
          )*
        }
      }
    }
  };
}

define_style! {
  longhands {
    box_sizing: BoxSizing,
    opacity: PercentageNumber,
    animation_name: AnimationNames,
    animation_duration: AnimationDurations,
    animation_delay: AnimationDurations,
    animation_timing_function: AnimationTimingFunctions,
    animation_iteration_count: AnimationIterationCounts,
    animation_direction: AnimationDirections,
    animation_fill_mode: AnimationFillModes,
    animation_play_state: AnimationPlayStates,
    display: Display,
    width: Length,
    height: Length,
    max_width: Length,
    max_height: Length,
    min_width: Length,
    min_height: Length,
    aspect_ratio: AspectRatio,
    padding_top: Length<false>,
    padding_right: Length<false>,
    padding_bottom: Length<false>,
    padding_left: Length<false>,
    margin_top: Length<false>,
    margin_right: Length<false>,
    margin_bottom: Length<false>,
    margin_left: Length<false>,
    top: Length,
    right: Length,
    bottom: Length,
    left: Length,
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
    scale: SpacePair<PercentageNumber>,
    translate: SpacePair<Length>,
    transform: Option<Transforms>,
    transform_origin: TransformOrigin,
    mask_image: Option<BackgroundImages>,
    mask_size: BackgroundSizes,
    mask_position: BackgroundPositions,
    mask_repeat: BackgroundRepeats,
    column_gap: Length<false>,
    row_gap: Length<false>,
    flex_grow: Option<FlexGrow>,
    flex_shrink: Option<FlexGrow>,
    border_top_left_radius: SpacePair<Length<false>>,
    border_top_right_radius: SpacePair<Length<false>>,
    border_bottom_right_radius: SpacePair<Length<false>>,
    border_bottom_left_radius: SpacePair<Length<false>>,
    border_top_width: Length,
    border_right_width: Length,
    border_bottom_width: Length,
    border_left_width: Length,
    border_style: BorderStyle,
    border_color: ColorInput,
    outline_width: Length,
    outline_style: BorderStyle,
    outline_color: ColorInput,
    outline_offset: Length,
    object_fit: ObjectFit,
    overflow_x: Overflow,
    overflow_y: Overflow,
    object_position: ObjectPosition where inherit = true,
    background_image: Option<BackgroundImages>,
    background_position: BackgroundPositions,
    background_size: BackgroundSizes,
    background_repeat: BackgroundRepeats,
    background_blend_mode: BlendModes,
    background_color: ColorInput<false>,
    background_clip: BackgroundClip,
    box_shadow: Option<BoxShadows>,
    grid_auto_columns: Option<GridTrackSizes>,
    grid_auto_rows: Option<GridTrackSizes>,
    grid_auto_flow: GridAutoFlow,
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
    font_size: FontSize where inherit = true,
    font_family: Option<FontFamily> where inherit = true,
    line_height: LineHeight where inherit = true,
    font_weight: FontWeight where inherit = true,
    font_variation_settings: FontVariationSettings where inherit = true,
    font_feature_settings: FontFeatureSettings where inherit = true,
    font_synthesis_weight: FontSynthesic where inherit = true,
    font_synthesis_style: FontSynthesic where inherit = true,
    line_clamp: Option<LineClamp> where inherit = true,
    text_align: TextAlign where inherit = true,
    webkit_text_stroke_width: Option<Length<false>> where inherit = true,
    webkit_text_stroke_color: Option<ColorInput> where inherit = true,
    webkit_text_fill_color: Option<ColorInput> where inherit = true,
    stroke_linejoin: LineJoin where inherit = true,
    text_shadow: Option<TextShadows> where inherit = true,
    text_decoration_line: Option<TextDecorationLines>,
    text_decoration_style: TextDecorationStyle,
    text_decoration_color: ColorInput,
    text_decoration_thickness: TextDecorationThickness,
    text_decoration_skip_ink: TextDecorationSkipInk where inherit = true,
    letter_spacing: Length where inherit = true,
    word_spacing: Length where inherit = true,
    image_rendering: ImageScalingAlgorithm where inherit = true,
    overflow_wrap: OverflowWrap where inherit = true,
    word_break: WordBreak where inherit = true,
    clip_path: Option<BasicShape>,
    clip_rule: FillRule where inherit = true,
    white_space_collapse: WhiteSpaceCollapse where inherit = true,
    text_wrap_mode: TextWrapMode where inherit = true,
    text_wrap_style: TextWrapStyle where inherit = true,
    isolation: Isolation,
    mix_blend_mode: BlendMode,
    visibility: Visibility,
    vertical_align: VerticalAlign,
  }
  shorthands {
    padding: Sides<Length<false>> => [PaddingTop, PaddingRight, PaddingBottom, PaddingLeft] |value, target, important| {
      let values = value.0;
      push_expanded_declarations!(
        target,
        important;
        StyleDeclaration::padding_top(values[0]),
        StyleDeclaration::padding_right(values[1]),
        StyleDeclaration::padding_bottom(values[2]),
        StyleDeclaration::padding_left(values[3]),
      );
    },
    padding_inline: SpacePair<Length<false>> => [PaddingLeft, PaddingRight] |value, target, important| {
      push_expanded_declarations!(
        target,
        important;
        StyleDeclaration::padding_left(value.x),
        StyleDeclaration::padding_right(value.y),
      );
    },
    padding_block: SpacePair<Length<false>> => [PaddingTop, PaddingBottom] |value, target, important| {
      push_expanded_declarations!(
        target,
        important;
        StyleDeclaration::padding_top(value.x),
        StyleDeclaration::padding_bottom(value.y),
      );
    },
    margin: Sides<Length<false>> => [MarginTop, MarginRight, MarginBottom, MarginLeft] |value, target, important| {
      let values = value.0;
      push_expanded_declarations!(
        target,
        important;
        StyleDeclaration::margin_top(values[0]),
        StyleDeclaration::margin_right(values[1]),
        StyleDeclaration::margin_bottom(values[2]),
        StyleDeclaration::margin_left(values[3]),
      );
    },
    margin_inline: SpacePair<Length<false>> => [MarginLeft, MarginRight] |value, target, important| {
      push_expanded_declarations!(
        target,
        important;
        StyleDeclaration::margin_left(value.x),
        StyleDeclaration::margin_right(value.y),
      );
    },
    margin_block: SpacePair<Length<false>> => [MarginTop, MarginBottom] |value, target, important| {
      push_expanded_declarations!(
        target,
        important;
        StyleDeclaration::margin_top(value.x),
        StyleDeclaration::margin_bottom(value.y),
      );
    },
    inset: Sides<Length> => [Top, Right, Bottom, Left] |value, target, important| {
      let values = value.0;
      push_expanded_declarations!(
        target,
        important;
        StyleDeclaration::top(values[0]),
        StyleDeclaration::right(values[1]),
        StyleDeclaration::bottom(values[2]),
        StyleDeclaration::left(values[3]),
      );
    },
    inset_inline: SpacePair<Length> => [Left, Right] |value, target, important| {
      push_expanded_declarations!(
        target,
        important;
        StyleDeclaration::left(value.x),
        StyleDeclaration::right(value.y),
      );
    },
    inset_block: SpacePair<Length> => [Top, Bottom] |value, target, important| {
      push_expanded_declarations!(
        target,
        important;
        StyleDeclaration::top(value.x),
        StyleDeclaration::bottom(value.y),
      );
    },
    mask: Backgrounds => [MaskImage, MaskPosition, MaskSize, MaskRepeat] |value, target, important| {
      push_expanded_declarations!(
        target,
        important;
        StyleDeclaration::mask_position(value.iter().map(|background| background.position).collect()),
        StyleDeclaration::mask_size(value.iter().map(|background| background.size).collect()),
        StyleDeclaration::mask_repeat(value.iter().map(|background| background.repeat).collect()),
        StyleDeclaration::mask_image(Some(value.into_iter().map(|background| background.image).collect())),
      );
    },
    gap: Gap => [RowGap, ColumnGap] |value, target, important| {
      push_expanded_declarations!(
        target,
        important;
        StyleDeclaration::row_gap(value.x),
        StyleDeclaration::column_gap(value.y),
      );
    },
    flex: Option<Flex> => [FlexGrow, FlexShrink, FlexBasis] |value, target, important| {
      push_expanded_declarations!(
        target,
        important;
        StyleDeclaration::flex_grow(value.map(|value| FlexGrow(value.grow))),
        StyleDeclaration::flex_shrink(value.map(|value| FlexGrow(value.shrink))),
        StyleDeclaration::flex_basis(value.map(|value| value.basis)),
      );
    },
    border_radius: Box<BorderRadius> => [BorderTopLeftRadius, BorderTopRightRadius, BorderBottomRightRadius, BorderBottomLeftRadius] |value, target, important| {
      let values = value.0.0;
      push_expanded_declarations!(
        target,
        important;
        StyleDeclaration::border_top_left_radius(values[0]),
        StyleDeclaration::border_top_right_radius(values[1]),
        StyleDeclaration::border_bottom_right_radius(values[2]),
        StyleDeclaration::border_bottom_left_radius(values[3]),
      );
    },
    border_width: Sides<Length> => [BorderTopWidth, BorderRightWidth, BorderBottomWidth, BorderLeftWidth] |value, target, important| {
      let values = value.0;
      push_expanded_declarations!(
        target,
        important;
        StyleDeclaration::border_top_width(values[0]),
        StyleDeclaration::border_right_width(values[1]),
        StyleDeclaration::border_bottom_width(values[2]),
        StyleDeclaration::border_left_width(values[3]),
      );
    },
    border_inline_width: Option<SpacePair<Length>> => [BorderLeftWidth, BorderRightWidth] |value, target, important| {
      let value = value.unwrap_or_default();
      push_expanded_declarations!(
        target,
        important;
        StyleDeclaration::border_left_width(value.x),
        StyleDeclaration::border_right_width(value.y),
      );
    },
    border_block_width: Option<SpacePair<Length>> => [BorderTopWidth, BorderBottomWidth] |value, target, important| {
      let value = value.unwrap_or_default();
      push_expanded_declarations!(
        target,
        important;
        StyleDeclaration::border_top_width(value.x),
        StyleDeclaration::border_bottom_width(value.y),
      );
    },
    border: Border => [BorderTopWidth, BorderRightWidth, BorderBottomWidth, BorderLeftWidth, BorderStyle, BorderColor] |value, target, important| {
      push_expanded_declarations!(
        target,
        important;
        StyleDeclaration::border_top_width(value.width),
        StyleDeclaration::border_right_width(value.width),
        StyleDeclaration::border_bottom_width(value.width),
        StyleDeclaration::border_left_width(value.width),
        StyleDeclaration::border_style(value.style),
        StyleDeclaration::border_color(value.color),
      );
    },
    outline: Border => [OutlineWidth, OutlineStyle, OutlineColor] |value, target, important| {
      push_expanded_declarations!(
        target,
        important;
        StyleDeclaration::outline_width(value.width),
        StyleDeclaration::outline_style(value.style),
        StyleDeclaration::outline_color(value.color),
      );
    },
    overflow: SpacePair<Overflow> => [OverflowX, OverflowY] |value, target, important| {
      push_expanded_declarations!(
        target,
        important;
        StyleDeclaration::overflow_x(value.x),
        StyleDeclaration::overflow_y(value.y),
      );
    },
    background: Backgrounds => [BackgroundImage, BackgroundPosition, BackgroundSize, BackgroundRepeat, BackgroundBlendMode, BackgroundColor, BackgroundClip] |value, target, important| {
      push_expanded_declarations!(
        target,
        important;
        StyleDeclaration::background_position(value.iter().map(|background| background.position).collect()),
        StyleDeclaration::background_size(value.iter().map(|background| background.size).collect()),
        StyleDeclaration::background_repeat(value.iter().map(|background| background.repeat).collect()),
        StyleDeclaration::background_blend_mode(value.iter().map(|background| background.blend_mode).collect()),
        StyleDeclaration::background_color(value.iter().filter_map(|background| background.color).next_back().unwrap_or_default()),
        StyleDeclaration::background_clip(value.last().map(|background| background.clip).unwrap_or_default()),
        StyleDeclaration::background_image(Some(value.into_iter().map(|background| background.image).collect())),
      );
    },
    font_synthesis: FontSynthesis where inherit = true => [FontSynthesisWeight, FontSynthesisStyle] |value, target, important| {
      push_expanded_declarations!(
        target,
        important;
        StyleDeclaration::font_synthesis_weight(value.weight),
        StyleDeclaration::font_synthesis_style(value.style),
      );
    },
    webkit_text_stroke: Option<TextStroke> where inherit = true => [WebkitTextStrokeWidth, WebkitTextStrokeColor] |value, target, important| {
      push_expanded_declarations!(
        target,
        important;
        StyleDeclaration::webkit_text_stroke_width(value.map(|value| value.width)),
        StyleDeclaration::webkit_text_stroke_color(value.and_then(|value| value.color)),
      );
    },
    text_decoration: TextDecoration => [TextDecorationLine, TextDecorationStyle, TextDecorationColor, TextDecorationThickness] |value, target, important| {
      push_expanded_declarations!(
        target,
        important;
        StyleDeclaration::text_decoration_line(Some(value.line)),
        StyleDeclaration::text_decoration_style(value.style.unwrap_or_default()),
        StyleDeclaration::text_decoration_color(value.color.unwrap_or_default()),
        StyleDeclaration::text_decoration_thickness(value.thickness.unwrap_or_default()),
      );
    },
    white_space: WhiteSpace where inherit = true => [TextWrapMode, WhiteSpaceCollapse] |value, target, important| {
      push_expanded_declarations!(
        target,
        important;
        StyleDeclaration::text_wrap_mode(value.text_wrap_mode),
        StyleDeclaration::white_space_collapse(value.white_space_collapse),
      );
    },
    text_wrap: TextWrap where inherit = true => [TextWrapMode, TextWrapStyle] |value, target, important| {
      push_expanded_declarations!(
        target,
        important;
        StyleDeclaration::text_wrap_mode(value.mode.unwrap_or_default()),
        StyleDeclaration::text_wrap_style(value.style),
      );
    },
  }
}

/// CSS-wide keywords that can target any longhand declaration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CssWideKeyword {
  /// Reset the targeted longhand to its initial value.
  Initial,
  /// Inherit the targeted longhand from the parent computed style.
  Inherit,
  /// Apply CSS `unset` semantics to the targeted longhand.
  Unset,
}

#[cfg(feature = "css_stylesheet_parsing")]
pub(crate) type PropertyMask = BTreeSet<LonghandId>;

/// Ordered specified declarations plus the set of important longhands.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct StyleDeclarationBlock {
  /// Ordered declarations in source order.
  pub(crate) declarations: SmallVec<[StyleDeclaration; 8]>,
  /// Longhands that were marked with `!important`.
  pub(crate) importance_set: BTreeSet<LonghandId>,
}

impl StyleDeclarationBlock {
  /// Appends a declaration and records whether it was important.
  pub(crate) fn push(&mut self, declaration: StyleDeclaration, important: bool) {
    if important {
      self.importance_set.insert(declaration.longhand_id());
    }
    self.declarations.push(declaration);
  }

  pub(crate) fn append(&mut self, mut other: Self) {
    self.importance_set.append(&mut other.importance_set);
    self.declarations.extend(other.declarations);
  }

  /// Iterates over the declarations in source order.
  pub(crate) fn iter(&self) -> std::slice::Iter<'_, StyleDeclaration> {
    self.declarations.iter()
  }

  pub(crate) fn parse<'i>(
    name: &str,
    input: &mut cssparser::Parser<'i, '_>,
  ) -> Result<Self, cssparser::ParseError<'i, Cow<'i, str>>> {
    parse_style_declaration(name, input)
  }
}

/// Backward-compatible alias for the computed style snapshot.
pub(crate) type ResolvedStyle = ComputedStyle;

/// Sized font style with resolved font size and line height.
#[derive(Clone)]
pub(crate) struct SizedFontStyle<'s> {
  pub parent: &'s ResolvedStyle,
  pub line_height: parley::LineHeight,
  pub stroke_width: f32,
  pub letter_spacing: f32,
  pub word_spacing: f32,
  pub text_shadow: SmallVec<[SizedShadow; 4]>,
  pub color: Color,
  pub text_stroke_color: Color,
  pub text_decoration_color: Color,
  pub text_decoration_thickness: SizedTextDecorationThickness,
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
        style.parent.font_variation_settings.as_ref(),
      )),
      font_features: FontSettings::List(Cow::Borrowed(style.parent.font_feature_settings.as_ref())),
      font_stack: style
        .parent
        .font_family
        .as_ref()
        .map(Into::into)
        .unwrap_or(FontStack::Source(Cow::Borrowed("sans-serif"))),
      letter_spacing: style.letter_spacing,
      word_spacing: style.word_spacing,
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
        decoration_thickness: style.text_decoration_thickness,
        decoration_line: style.parent.text_decoration_line.unwrap_or_default(),
        decoration_skip_ink: style.parent.text_decoration_skip_ink,
        stroke_color: style.text_stroke_color,
        font_synthesis: FontSynthesis {
          weight: style.parent.font_synthesis_weight,
          style: style.parent.font_synthesis_style,
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

impl ResolvedStyle {
  /// Normalize inheritable text-related values to computed values for this node.
  pub(crate) fn make_computed(&mut self, sizing: &Sizing) {
    // `font-size` computed value is already resolved in `sizing.font_size`.
    // Keep it as css-px in style to avoid re-resolving descendant inheritance.
    let dpr = sizing.viewport.device_pixel_ratio;
    self.font_size = if dpr > 0.0 {
      FontSize::Length(Length::Px(sizing.font_size / dpr))
    } else {
      FontSize::Length(Length::Px(sizing.font_size))
    };

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
      || self.mask_image.as_ref().is_some_and(|images| {
        images
          .iter()
          .any(|image| !matches!(image, BackgroundImage::None))
      })
  }

  pub(crate) fn has_non_identity_transform(&self, border_box: Size<f32>, sizing: &Sizing) -> bool {
    let transform_origin = self.transform_origin;
    let origin = transform_origin.to_point(sizing, border_box);

    let mut local = Affine::translation(origin.x, origin.y);

    let translate = self.translate;
    if translate != SpacePair::default() {
      local *= Affine::translation(
        translate.x.to_px(sizing, border_box.width),
        translate.y.to_px(sizing, border_box.height),
      );
    }

    if let Some(rotate) = self.rotate {
      local *= Affine::rotation(rotate);
    }

    let scale = self.scale;
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
    SpacePair::from_pair(self.overflow_x, self.overflow_y)
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

  pub(crate) fn text_wrap_mode_and_line_clamp(&self) -> (TextWrapMode, Option<Cow<'_, LineClamp>>) {
    let mut text_wrap_mode = self.text_wrap_mode;

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
  fn resolved_gap(&self) -> SpacePair<Length<false>> {
    SpacePair::from_pair(self.row_gap, self.column_gap)
  }

  pub(crate) fn to_sized_font_style(&'_ self, context: &RenderContext) -> SizedFontStyle<'_> {
    let line_height = self.line_height.into_parley(&context.sizing);

    let resolved_stroke_width = self
      .webkit_text_stroke_width
      .unwrap_or_default()
      .to_px(&context.sizing, context.sizing.font_size);

    SizedFontStyle {
      sizing: context.sizing.clone(),
      parent: self,
      line_height,
      stroke_width: resolved_stroke_width,
      letter_spacing: self
        .letter_spacing
        .to_px(&context.sizing, context.sizing.font_size),
      word_spacing: self
        .word_spacing
        .to_px(&context.sizing, context.sizing.font_size),
      text_shadow: self
        .text_shadow
        .as_ref()
        .map_or_else(SmallVec::new, |shadows| {
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
        .unwrap_or_default()
        .resolve(context.current_color),
      text_decoration_color: self.text_decoration_color.resolve(context.current_color),
      text_decoration_thickness: match self.text_decoration_thickness {
        TextDecorationThickness::Length(Length::Auto) | TextDecorationThickness::FromFont => {
          SizedTextDecorationThickness::FromFont
        }
        TextDecorationThickness::Length(thickness) => SizedTextDecorationThickness::Value(
          thickness.to_px(&context.sizing, context.sizing.font_size),
        ),
      },
    }
  }

  pub(crate) fn to_taffy_style(&self, sizing: &Sizing) -> taffy::Style {
    // Convert grid templates and associated line names
    let (grid_template_columns, grid_template_column_names) =
      Self::convert_template_components(&self.grid_template_columns, sizing);
    let (grid_template_rows, grid_template_row_names) =
      Self::convert_template_components(&self.grid_template_rows, sizing);

    taffy::Style {
      box_sizing: self.box_sizing.into(),
      size: Size {
        width: self.width.resolve_to_dimension(sizing),
        height: self.height.resolve_to_dimension(sizing),
      },
      border: if self.border_style == BorderStyle::None {
        Rect::zero()
      } else {
        Rect {
          top: self.border_top_width,
          right: self.border_right_width,
          bottom: self.border_bottom_width,
          left: self.border_left_width,
        }
        .map(|border| border.resolve_to_length_percentage(sizing))
      },
      padding: Rect {
        top: self.padding_top,
        right: self.padding_right,
        bottom: self.padding_bottom,
        left: self.padding_left,
      }
      .map(|padding| padding.resolve_to_length_percentage(sizing)),
      inset: Rect {
        top: self.top,
        right: self.right,
        bottom: self.bottom,
        left: self.left,
      }
      .map(|inset| inset.resolve_to_length_percentage_auto(sizing)),
      margin: Rect {
        top: self.margin_top,
        right: self.margin_right,
        bottom: self.margin_bottom,
        left: self.margin_left,
      }
      .map(|margin| margin.resolve_to_length_percentage_auto(sizing)),
      display: self.display.into(),
      flex_direction: self.flex_direction.into(),
      position: self.position.into(),
      justify_content: self.justify_content.into(),
      align_content: self.align_content.into(),
      justify_items: self.justify_items.into(),
      flex_grow: self.flex_grow.map(|grow| grow.0).unwrap_or(0.0),
      align_items: self.align_items.into(),
      gap: self.resolved_gap().resolve_to_size(sizing),
      flex_basis: self
        .flex_basis
        .unwrap_or(Length::Auto)
        .resolve_to_dimension(sizing),
      flex_shrink: self.flex_shrink.map(|shrink| shrink.0).unwrap_or(1.0),
      flex_wrap: self.flex_wrap.into(),
      min_size: Size {
        width: self.min_width.resolve_to_dimension(sizing),
        height: self.min_height.resolve_to_dimension(sizing),
      },
      max_size: Size {
        width: self.max_width.resolve_to_dimension(sizing),
        height: self.max_height.resolve_to_dimension(sizing),
      },
      grid_auto_columns: self.grid_auto_columns.as_ref().map_or_else(Vec::new, |v| {
        v.iter().map(|s| s.to_min_max(sizing)).collect()
      }),
      grid_auto_rows: self.grid_auto_rows.as_ref().map_or_else(Vec::new, |v| {
        v.iter().map(|s| s.to_min_max(sizing)).collect()
      }),
      grid_auto_flow: self.grid_auto_flow.into(),
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
  use std::rc::Rc;

  use cssparser::{Parser, ParserInput};
  use taffy::Size;

  use super::{CssWideKeyword, LonghandId, PropertyId, StyleDeclarationBlock};
  use crate::{
    layout::{
      Viewport,
      style::{ResolvedStyle, Style, StyleDeclaration, properties::*},
    },
    rendering::Sizing,
  };

  fn style_with(declarations: impl IntoIterator<Item = StyleDeclaration>) -> Style {
    let mut style = Style::default();
    for declaration in declarations {
      style.push(declaration, false);
    }
    style
  }

  fn parse_declarations(name: &str, css: &str) -> StyleDeclarationBlock {
    let mut input = ParserInput::new(css);
    let mut parser = Parser::new(&mut input);
    let declarations_result = StyleDeclarationBlock::parse(name, &mut parser);
    assert!(declarations_result.is_ok());
    let Ok(declarations) = declarations_result else {
      unreachable!()
    };
    declarations
  }

  #[test]
  fn test_merge_from_inline_over_tailwind() {
    let mut tw_style = style_with([
      StyleDeclaration::width(Length::Rem(10.0)),
      StyleDeclaration::height(Length::Rem(20.0)),
      StyleDeclaration::color(ColorInput::Value(Color([255, 0, 0, 255]))),
    ]);
    let inline_style = style_with([StyleDeclaration::width(Length::Px(100.0))]);

    tw_style.merge_from(inline_style);

    let resolved = tw_style.inherit(&ResolvedStyle::default());
    assert_eq!(resolved.width, Length::Px(100.0));
    assert_eq!(resolved.height, Length::Rem(20.0));
    assert_eq!(resolved.color, ColorInput::Value(Color([255, 0, 0, 255])));
  }

  #[test]
  fn property_id_accepts_kebab_and_camel_case() {
    let padding_left_kebab = PropertyId::from_kebab_case("padding-left");
    let padding_left_camel = PropertyId::from_camel_case("paddingLeft");
    assert_ne!(padding_left_kebab, PropertyId::Ignored);
    assert_ne!(padding_left_camel, PropertyId::Ignored);
    assert_eq!(padding_left_kebab, padding_left_camel);

    let webkit_text_fill_color_kebab = PropertyId::from_kebab_case("-webkit-text-fill-color");
    let webkit_text_fill_color_camel = PropertyId::from_camel_case("WebkitTextFillColor");
    assert_ne!(webkit_text_fill_color_kebab, PropertyId::Ignored);
    assert_ne!(webkit_text_fill_color_camel, PropertyId::Ignored);
    assert_eq!(
      webkit_text_fill_color_kebab,
      PropertyId::Longhand(LonghandId::WebkitTextFillColor)
    );
    assert_eq!(
      webkit_text_fill_color_camel,
      PropertyId::Longhand(LonghandId::WebkitTextFillColor)
    );
  }

  #[test]
  fn custom_properties_do_not_map_to_supported_properties() {
    assert_eq!(
      PropertyId::from_kebab_case("--padding-left"),
      PropertyId::Ignored
    );
    assert_eq!(
      PropertyId::from_kebab_case("--webkit-mask-image"),
      PropertyId::Ignored
    );
  }

  #[test]
  fn property_id_accepts_webkit_aliases() {
    assert_eq!(
      PropertyId::from_kebab_case("-webkit-text-fill-color"),
      PropertyId::Longhand(LonghandId::WebkitTextFillColor)
    );
    assert_eq!(
      PropertyId::from_kebab_case("-webkit-text-stroke-color"),
      PropertyId::Longhand(LonghandId::WebkitTextStrokeColor)
    );
  }

  #[test]
  fn parse_style_declaration_supports_css_wide_keywords_for_longhands() {
    let declarations = parse_declarations("color", "inherit");

    assert_eq!(
      declarations.iter().collect::<Vec<_>>(),
      vec![&StyleDeclaration::CssWideKeyword(
        LonghandId::Color,
        CssWideKeyword::Inherit,
      )]
    );
  }

  #[test]
  fn parse_style_declaration_still_parses_normal_longhand_values() {
    let declarations = parse_declarations("color", "#ff0000");

    assert_eq!(
      declarations.iter().collect::<Vec<_>>(),
      vec![&StyleDeclaration::color(ColorInput::Value(Color([
        255, 0, 0, 255
      ])))]
    );
  }

  #[test]
  fn test_merge_from_text_decoration_longhands_clear_lower_priority_color() {
    let mut preset_style = style_with([StyleDeclaration::text_decoration_color(
      ColorInput::Value(Color([255, 0, 0, 255])),
    )]);
    let inline_style = style_with([
      StyleDeclaration::text_decoration_line(Some(TextDecorationLines::UNDERLINE)),
      StyleDeclaration::text_decoration_style(TextDecorationStyle::default()),
      StyleDeclaration::text_decoration_color(ColorInput::default()),
      StyleDeclaration::text_decoration_thickness(TextDecorationThickness::default()),
    ]);

    preset_style.merge_from(inline_style);

    let inherited = preset_style.inherit(&ResolvedStyle::default());
    assert_eq!(inherited.text_decoration_color, ColorInput::default());
    assert_eq!(
      inherited.text_decoration_line,
      Some(TextDecorationLines::UNDERLINE)
    );
  }

  #[test]
  fn test_merge_from_background_longhands_clear_lower_priority_background_color() {
    let mut preset_style = style_with([StyleDeclaration::background_color(ColorInput::Value(
      Color([255, 0, 0, 255]),
    ))]);
    let inline_style = style_with([
      StyleDeclaration::background_image(Some([BackgroundImage::None].into())),
      StyleDeclaration::background_position([BackgroundPosition::default()].into()),
      StyleDeclaration::background_size([BackgroundSize::default()].into()),
      StyleDeclaration::background_repeat([BackgroundRepeat::default()].into()),
      StyleDeclaration::background_blend_mode([BlendMode::default()].into()),
      StyleDeclaration::background_color(ColorInput::default()),
      StyleDeclaration::background_clip(BackgroundClip::default()),
    ]);

    preset_style.merge_from(inline_style);

    let inherited = preset_style.inherit(&ResolvedStyle::default());
    assert_eq!(inherited.background_color, ColorInput::default());
  }

  #[test]
  fn test_isolated_for_clip_path_and_mask_image() {
    let mut style = ResolvedStyle::default();
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
    let mut style = ResolvedStyle::default();
    let sizing = Sizing {
      viewport: Viewport::new(Some(1200), Some(630)),
      container_size: Size::NONE,
      font_size: 16.0,
      calc_arena: Rc::new(CalcArena::default()),
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
    let style = ResolvedStyle {
      text_wrap_mode: TextWrapMode::NoWrap,
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
    let mut parent = style_with([
      StyleDeclaration::font_size(Length::Em(2.0).into()),
      StyleDeclaration::letter_spacing(Length::Em(1.0)),
      StyleDeclaration::line_height(LineHeight::Length(Length::Em(1.5))),
    ])
    .inherit(&ResolvedStyle::default());
    parent.make_computed(&Sizing {
      viewport: Viewport::new(Some(1200), Some(630)),
      container_size: Size::NONE,
      font_size: 32.0,
      calc_arena: Rc::new(CalcArena::default()),
    });

    let inherited_child = Style::default().inherit(&parent);
    let inherited_child_sizing = Sizing {
      viewport: Viewport::new(Some(1200), Some(630)),
      container_size: Size::NONE,
      font_size: 32.0,
      calc_arena: Rc::new(CalcArena::default()),
    };
    let inherited_font_size = inherited_child
      .font_size
      .to_px(&inherited_child_sizing, inherited_child_sizing.font_size);
    assert_eq!(inherited_font_size, 32.0);

    let child_with_own_font_size =
      style_with([StyleDeclaration::font_size(Length::Px(10.0).into())]).inherit(&parent);
    let child_sizing = Sizing {
      viewport: Viewport::new(Some(1200), Some(630)),
      container_size: Size::NONE,
      font_size: 10.0,
      calc_arena: Rc::new(CalcArena::default()),
    };

    let inherited_letter_spacing = child_with_own_font_size
      .letter_spacing
      .to_px(&child_sizing, child_sizing.font_size);
    assert_eq!(inherited_letter_spacing, 32.0);

    let inherited_line_height = match child_with_own_font_size.line_height {
      LineHeight::Length(length) => length.to_px(&child_sizing, child_sizing.font_size),
      _ => 0.0,
    };
    assert_eq!(inherited_line_height, 48.0);
  }
}
