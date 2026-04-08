use crate::layout::style::unexpected_token;
use std::{borrow::Cow, collections::HashMap, marker::PhantomData, str::FromStr};

use cssparser::{Parser, ParserInput, Token, match_ignore_ascii_case};
use parley::{FontFeatures, FontVariations, TextStyle};
use paste::paste;
use serde::de::IgnoredAny;
use smallvec::{SmallVec, smallvec};
use taffy::{Line, Point, Rect, Size, prelude::FromLength};

use crate::layout::style::selector::{PropertyRule, StyleDeclarationParser};
use crate::{
  error::StyleDeclarationBlockParseError,
  layout::{
    inline::InlineBrush,
    style::{CssInput, CssValueSeed, properties::*},
  },
  rendering::{RenderContext, SizedShadow, Sizing},
};
use cssparser::RuleBodyParser;
#[path = "stylesheets_helpers.rs"]
mod stylesheets_helpers;
#[path = "stylesheets_vars.rs"]
mod stylesheets_vars;

use self::stylesheets_helpers::*;
use self::stylesheets_vars::apply_deferred_declaration;

macro_rules! define_inherited_default {
  ($parent:expr, $inherit:tt) => {
    $parent.to_owned()
  };
  ($parent:expr) => {
    Default::default()
  };
}

type ParsedDeclarations = SmallVec<[StyleDeclaration; 8]>;

#[derive(Debug, Clone, PartialEq, Eq)]
struct DeferredDeclaration {
  property: PropertyId,
  specified_value: String,
}

#[derive(Clone, Copy)]
struct InterpolationContext<'a> {
  progress: f32,
  sizing: &'a Sizing,
  current_color: Color,
}

fn interpolate_option_with_missing<T: Animatable + Clone>(
  target: &mut Option<T>,
  from: &Option<T>,
  to: &Option<T>,
  missing_from: T,
  missing_to: T,
  context: InterpolationContext<'_>,
) {
  *target = match (from, to) {
    (Some(from), Some(to)) => {
      let mut value = from.clone();
      value.interpolate(
        from,
        to,
        context.progress,
        context.sizing,
        context.current_color,
      );
      Some(value)
    }
    (Some(from), None) => {
      let mut value = from.clone();
      value.interpolate(
        from,
        &missing_to,
        context.progress,
        context.sizing,
        context.current_color,
      );
      Some(value)
    }
    (None, Some(to)) => {
      let mut value = missing_from.clone();
      value.interpolate(
        &missing_from,
        to,
        context.progress,
        context.sizing,
        context.current_color,
      );
      Some(value)
    }
    (None, None) => None,
  };
}

macro_rules! push_expanded_declarations {
  ($target:expr; $($declaration:expr),+ $(,)?) => {{
    $(
      $target.push($declaration);
    )+
  }};
}

macro_rules! push_axis_declarations {
  ($target:expr, $value:expr, $first:ident, $second:ident) => {{
    let value = $value;
    push_expanded_declarations!(
      $target;
      StyleDeclaration::$first(value.x),
      StyleDeclaration::$second(value.y),
    );
  }};
}

macro_rules! push_four_side_declarations {
  ($target:expr, $values:expr, $top:ident, $right:ident, $bottom:ident, $left:ident) => {{
    let values = $values;
    push_expanded_declarations!(
      $target;
      StyleDeclaration::$top(values[0]),
      StyleDeclaration::$right(values[1]),
      StyleDeclaration::$bottom(values[2]),
      StyleDeclaration::$left(values[3]),
    );
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
          |$value:ident, $target_var:ident|
          $expand:block,
      )*
    }
  ) => {
    paste! {
      #[repr(u8)]
      #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
      pub(crate) enum LonghandId {
        $([<$longhand:camel>],)*
      }

      impl LonghandId {
        const COUNT: usize = [$(Self::[<$longhand:camel>]),*].len();
        const ALL: [Self; Self::COUNT] = [$(Self::[<$longhand:camel>],)*];

        const fn index(self) -> usize {
          self as usize
        }
      }

      #[repr(u8)]
      #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
      pub(crate) enum ShorthandId {
        $([<$shorthand:camel>],)*
      }

      impl LonghandId {
        fn parse_declarations<'i>(
          self,
          input: &mut cssparser::Parser<'i, '_>,
        ) -> ParseResult<'i, ParsedDeclarations> {
          let state = input.state();
          let keyword = input.try_parse(CssWideKeyword::from_css).ok();

          if let Some(keyword) = keyword {
            return Ok(smallvec![StyleDeclaration::CssWideKeyword(self, keyword)]);
          }

          input.reset(&state);
          match self {
            $(
              Self::[<$longhand:camel>] => Ok(smallvec![StyleDeclaration::[<$longhand:camel>](
                <$longhand_ty as FromCss>::from_css(input)?,
              )]),
            )*
          }
        }

        fn parse_css_input_declarations<'de>(
          self,
          css_input: CssInput<'de>,
        ) -> Result<ParsedDeclarations, CssInputParseError<'de>> {
          match self {
            $(
              Self::[<$longhand:camel>] => {
                if let Some(keyword) = parse_css_wide_keyword(&css_input) {
                  return Ok(smallvec![StyleDeclaration::CssWideKeyword(self, keyword)]);
                }

                Ok(smallvec![StyleDeclaration::[<$longhand:camel>](
                  parse_css_input_value(css_input)?,
                )])
              }
            )*
          }
        }
      }

      impl ShorthandId {
        fn parse_declarations<'i>(
          self,
          input: &mut cssparser::Parser<'i, '_>,
        ) -> ParseResult<'i, ParsedDeclarations> {
          match self {
            $(
              Self::[<$shorthand:camel>] => Ok(expand_shorthand(
                <$shorthand_ty as FromCss>::from_css(input)?,
                |$value, $target_var| {
                  $expand
                },
              )),
            )*
          }
        }

        fn parse_css_input_declarations<'de>(
          self,
          css_input: CssInput<'de>,
        ) -> Result<ParsedDeclarations, CssInputParseError<'de>> {
          match self {
            $(
              Self::[<$shorthand:camel>] => {
                if let Some(keyword) = parse_css_wide_keyword(&css_input) {
                  let mut declarations = ParsedDeclarations::new();
                  $(
                    declarations.push(StyleDeclaration::CssWideKeyword(LonghandId::$target, keyword));
                  )+
                  return Ok(declarations);
                }

                Ok(expand_shorthand(
                  parse_css_input_value::<$shorthand_ty>(css_input)?,
                  |$value, $target_var| {
                    $expand
                  },
                ))
              }
            )*
          }
        }
      }

      #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
      pub(crate) enum PropertyId {
        Ignored,
        Custom,
        Longhand(LonghandId),
        Shorthand(ShorthandId),
      }

      impl PropertyId {
        fn from_normalized_name(name: &str) -> Self {
          match name {
            $(stringify!($longhand) => Self::Longhand(LonghandId::[<$longhand:camel>]),)*
            $(stringify!($shorthand) => Self::Shorthand(ShorthandId::[<$shorthand:camel>]),)*
            _ => Self::Ignored,
          }
        }

        fn from_kebab_case(name: &str) -> Self {
          property_id_from_name(name, normalize_kebab_property_name)
        }

        #[allow(dead_code)]
        pub(crate) fn from_camel_case(name: &str) -> Self {
          property_id_from_name(name, normalize_camel_property_name)
        }

        fn parse_declarations<'i>(
          self,
          name: &str,
          input: &mut cssparser::Parser<'i, '_>,
        ) -> ParseResult<'i, ParsedDeclarations> {
          match self {
            Self::Ignored => {
              while input.next_including_whitespace_and_comments().is_ok() {}
              Ok(ParsedDeclarations::new())
            }
            Self::Custom => {
              let start = input.position();
              while input.next_including_whitespace_and_comments().is_ok() {}
              Ok(smallvec![StyleDeclaration::CustomProperty(
                name.to_owned(),
                input.slice_from(start).trim().to_owned(),
              )])
            }
            Self::Shorthand(property) => property.parse_declarations(input),
            Self::Longhand(property) => property.parse_declarations(input),
          }
        }

        fn parse_css_input_declarations<'de>(
          self,
          css_input: CssInput<'de>,
        ) -> Result<ParsedDeclarations, CssInputParseError<'de>> {
          debug_assert!(
            !matches!(self, Self::Custom),
            "custom properties should be handled before parse_css_input_declarations",
          );

          let css_string = match &css_input {
            CssInput::Str(value) => Some(value.as_ref()),
            CssInput::Number(_) => None,
            CssInput::Unexpected(_) => None,
          };

            if css_string.is_some_and(contains_var_function) {
              return Ok(smallvec![StyleDeclaration::Deferred(DeferredDeclaration {
                property: self,
                specified_value: css_input.into_string(),
              })]);
            }

          match self {
            Self::Ignored => Ok(ParsedDeclarations::new()),
            Self::Custom => Ok(ParsedDeclarations::new()),
            Self::Shorthand(property) => property.parse_css_input_declarations(css_input),
            Self::Longhand(property) => property.parse_css_input_declarations(css_input),
          }
        }

        fn important_longhands(self) -> PropertyMask {
          match self {
            Self::Ignored | Self::Custom => PropertyMask::default(),
            Self::Longhand(property) => [property].into_iter().collect(),
            Self::Shorthand(property) => match property {
              $(ShorthandId::[<$shorthand:camel>] => {
                [$(LonghandId::$target),+].into_iter().collect()
              })*
            },
          }
        }
      }

      fn parse_style_declaration<'i>(
        name: &str,
        input: &mut cssparser::Parser<'i, '_>,
      ) -> ParseResult<'i, StyleDeclarationBlock> {
        let property = PropertyId::from_kebab_case(name);
        let start = input.position();
        match property.parse_declarations(name, input) {
          Ok(declarations) => Ok(StyleDeclarationBlock::from_parsed_declarations(
            declarations,
            false,
          )),
          Err(error) if !matches!(property, PropertyId::Ignored | PropertyId::Custom) => {
            while input.next_including_whitespace_and_comments().is_ok() {}
            let specified_value = input.slice_from(start).trim();
            if contains_var_function(specified_value) {
              Ok(StyleDeclarationBlock::from_parsed_declarations(
                smallvec![StyleDeclaration::Deferred(DeferredDeclaration {
                  property,
                  specified_value: specified_value.to_owned(),
                })],
                false,
              ))
            } else {
              Err(error)
            }
          }
          Err(error) => Err(error),
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

                let css_input = map.next_value_seed(CssValueSeed)?;
                if matches!(property, PropertyId::Custom) {
                  style.declarations.push(
                    StyleDeclaration::CustomProperty(key.into_owned(), css_input.into_string()),
                    false,
                  );
                } else {
                  style
                    .declarations
                    .append_parsed_declarations(
                      property
                        .parse_css_input_declarations(css_input)
                        .map_err(|error| error.into_serde_error(&key, property))?,
                      false,
                    );
                }
              }

              Ok(style)
            }
          }

          deserializer.deserialize_map(StyleVisitor)
        }
      }

      impl<'de> serde::Deserialize<'de> for StyleDeclarationBlock {
        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
          D: serde::Deserializer<'de>,
        {
          Style::deserialize(deserializer).map(Into::into)
        }
      }

      impl Style {
        fn with_declarations(
          mut self,
          declarations: impl IntoIterator<Item = StyleDeclaration>,
          important: bool,
        ) -> Self {
          for declaration in declarations {
            self.declarations.push(declaration, important);
          }
          self
        }

        /// Returns a new style with one declaration appended in source order.
        pub fn with(self, declaration: StyleDeclaration) -> Self {
          self.with_declarations([declaration], false)
        }

        $(
          /// Returns a new style with this shorthand expanded and appended in source order.
          pub fn [<with_ $shorthand>](self, value: $shorthand_ty) -> Self {
            self.with_declarations(
              expand_shorthand(value, |$value, $target_var| {
                $expand
              }),
              false,
            )
          }
        )*

        /// Returns a new style with one `!important` declaration appended in source order.
        pub fn with_important(self, declaration: StyleDeclaration) -> Self {
          self.with_declarations([declaration], true)
        }

        pub(crate) fn append_block(&mut self, declarations: StyleDeclarationBlock) {
          self.declarations.append(declarations);
        }

        pub(crate) fn push(&mut self, declaration: StyleDeclaration, important: bool) {
          self.declarations.push(declaration, important);
        }

        /// Collects resource URLs referenced by this style's declarations.
        pub fn resource_urls(&self) -> impl Iterator<Item = &str> {
          self.declarations.resource_urls()
        }

        pub(crate) fn inherit(self, parent: &ComputedStyle) -> ComputedStyle {
          let mut style = ComputedStyle::from_parent(parent);
          let mut declarations = ParsedDeclarations::new();

          for declaration in self.declarations.declarations {
            match declaration {
              StyleDeclaration::CustomProperty(name, value) => {
                style.custom_properties.insert(name, value);
              }
              declaration => declarations.push(declaration),
            }
          }

          for declaration in declarations {
            declaration.apply_with_parent(&mut style, parent);
          }
          style
        }

        pub(crate) fn merge_from(&mut self, other: Self) {
          self.append_block(other.declarations);
        }
      }

      impl From<StyleDeclarationBlock> for Style {
        fn from(declarations: StyleDeclarationBlock) -> Self {
          Self { declarations }
        }
      }

      impl From<Style> for StyleDeclarationBlock {
        fn from(style: Style) -> Self {
          style.declarations
        }
      }

      /// The computed style snapshot used during layout and rendering.
      #[derive(Clone, Debug, Default)]
      pub struct ComputedStyle {
        pub(crate) custom_properties: HashMap<String, String>,
        pub(crate) registered_custom_properties: HashMap<String, PropertyRule>,
        $(pub(crate) $longhand: $longhand_ty,)*
      }

      /// A single specified declaration stored in a declaration block.
      #[allow(private_interfaces)]
      #[derive(Debug, Clone, PartialEq)]
      #[non_exhaustive]
      pub enum StyleDeclaration {
        $(
          /// An explicit specified value for a non-shorthand property.
          [<$longhand:camel>]($longhand_ty),
        )*
        /// A custom property declaration such as `--token: value`.
        CustomProperty(String, String),
        /// A property value that must be resolved after `var()` substitution.
        Deferred(DeferredDeclaration),
        /// A CSS-wide keyword targeting a longhand property.
        CssWideKeyword(LonghandId, CssWideKeyword),
      }

      impl ComputedStyle {
        pub(crate) fn from_parent(parent: &Self) -> Self {
          Self {
            custom_properties: parent.custom_properties.clone(),
                        registered_custom_properties: parent.registered_custom_properties.clone(),
            $($longhand: define_inherited_default!(parent.$longhand $(, $longhand_inherit)?),)*
          }
        }

        pub(crate) fn make_computed_values(&mut self, sizing: &Sizing) {
          $(self.$longhand.make_computed(sizing);)*
        }

        pub(crate) fn apply_interpolated_properties(
          &mut self,
          from: &Self,
          to: &Self,
          animated_properties: &PropertyMask,
          progress: f32,
          sizing: &Sizing,
          current_color: Color,
        ) {
          let interpolation_context = InterpolationContext {
            progress,
            sizing,
            current_color,
          };

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

          // special cases
          if animated_properties.contains(&LonghandId::FlexGrow) {
            interpolate_option_with_missing(
              &mut self.flex_grow,
              &from.flex_grow,
              &to.flex_grow,
              FlexGrow(0.0),
              FlexGrow(0.0),
              interpolation_context,
            );
          }

          if animated_properties.contains(&LonghandId::FlexShrink) {
            interpolate_option_with_missing(
              &mut self.flex_shrink,
              &from.flex_shrink,
              &to.flex_shrink,
              FlexGrow(1.0),
              FlexGrow(1.0),
              interpolation_context,
            );
          }

          if animated_properties.contains(&LonghandId::WebkitTextStrokeWidth) {
            interpolate_option_with_missing(
              &mut self.webkit_text_stroke_width,
              &from.webkit_text_stroke_width,
              &to.webkit_text_stroke_width,
              Length::zero(),
              Length::zero(),
              interpolation_context,
            );
          }

          if animated_properties.contains(&LonghandId::WebkitTextStrokeColor) {
            interpolate_option_with_missing(
              &mut self.webkit_text_stroke_color,
              &from.webkit_text_stroke_color,
              &to.webkit_text_stroke_color,
              ColorInput::CurrentColor,
              ColorInput::CurrentColor,
              interpolation_context,
            );
          }

          if animated_properties.contains(&LonghandId::WebkitTextFillColor) {
            interpolate_option_with_missing(
              &mut self.webkit_text_fill_color,
              &from.webkit_text_fill_color,
              &to.webkit_text_fill_color,
              from.color,
              to.color,
              interpolation_context,
            );
          }
        }
      }

      impl StyleDeclaration {
        $(
          /// Returns a declaration for this property.
          pub fn $longhand(value: $longhand_ty) -> Self {
            Self::[<$longhand:camel>](value)
          }
        )*

        pub(crate) fn longhand_id(&self) -> LonghandId {
          match self {
            $(Self::[<$longhand:camel>](..) => LonghandId::[<$longhand:camel>],)*
            Self::CustomProperty(..) | Self::Deferred(..) => {
              unreachable!("custom and deferred declarations do not map to a single longhand")
            }
            Self::CssWideKeyword(id, _) => *id,
          }
        }

        pub(crate) fn affected_longhands(&self) -> PropertyMask {
          match self {
            Self::CssWideKeyword(id, _) => [*id].into_iter().collect(),
            Self::CustomProperty(..) => PropertyMask::default(),
            Self::Deferred(deferred) => deferred.property.important_longhands(),
            _ => [self.longhand_id()].into_iter().collect(),
          }
        }

        pub(crate) fn apply_with_parent(
          self,
          style: &mut ComputedStyle,
          parent: &ComputedStyle,
        ) {
          match self {
            Self::CssWideKeyword(property, keyword) => {
              match property {
                $(
                  LonghandId::[<$longhand:camel>] => {
                    style.$longhand = match keyword {
                      CssWideKeyword::Initial => Default::default(),
                      CssWideKeyword::Inherit => parent.$longhand.to_owned(),
                      CssWideKeyword::Unset => define_inherited_default!(parent.$longhand $(, $longhand_inherit)?),
                    };
                  }
                )*
              }
            }
            Self::CustomProperty(name, value) => {
              style.custom_properties.insert(name, value);
            }
            Self::Deferred(deferred) => {
              apply_deferred_declaration(style, Some(parent), &deferred);
            }
            $(Self::[<$longhand:camel>](value) => style.$longhand = value,)*
          }
        }

        pub(crate) fn apply_to_computed(&self, style: &mut ComputedStyle) {
          match self {
            Self::CssWideKeyword(property, keyword) => match keyword {
              CssWideKeyword::Initial => match property {
                $(
                  LonghandId::[<$longhand:camel>] => {
                    style.$longhand = Default::default();
                  }
                )*
              },
              CssWideKeyword::Inherit | CssWideKeyword::Unset => {}
            },
            Self::CustomProperty(name, value) => {
              style
                .custom_properties
                .insert(name.to_owned(), value.to_owned());
            }
            Self::Deferred(deferred) => apply_deferred_declaration(style, None, deferred),
            $(Self::[<$longhand:camel>](value) => style.$longhand.clone_from(value),)*
          }
        }

        pub(crate) fn merge_into_ref(&self, style: &mut Style) {
          style.declarations.push(self.to_owned(), false);
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
    padding_top: LengthDefaultsToZero,
    padding_right: LengthDefaultsToZero,
    padding_bottom: LengthDefaultsToZero,
    padding_left: LengthDefaultsToZero,
    margin_top: LengthDefaultsToZero,
    margin_right: LengthDefaultsToZero,
    margin_bottom: LengthDefaultsToZero,
    margin_left: LengthDefaultsToZero,
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
    order: Order,
    z_index: ZIndex,
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
    column_gap: LengthDefaultsToZero,
    row_gap: LengthDefaultsToZero,
    flex_grow: Option<FlexGrow>,
    flex_shrink: Option<FlexGrow>,
    border_top_left_radius: SpacePair<LengthDefaultsToZero>,
    border_top_right_radius: SpacePair<LengthDefaultsToZero>,
    border_bottom_right_radius: SpacePair<LengthDefaultsToZero>,
    border_bottom_left_radius: SpacePair<LengthDefaultsToZero>,
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
    grid_row_start: GridPlacement,
    grid_row_end: GridPlacement,
    grid_column_start: GridPlacement,
    grid_column_end: GridPlacement,
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
    font_family: FontFamily where inherit = true,
    line_height: LineHeight where inherit = true,
    font_weight: FontWeight where inherit = true,
    font_variation_settings: FontVariationSettings where inherit = true,
    font_feature_settings: FontFeatureSettings where inherit = true,
    font_synthesis_weight: FontSynthesic where inherit = true,
    font_synthesis_style: FontSynthesic where inherit = true,
    line_clamp: Option<LineClamp> where inherit = true,
    text_align: TextAlign where inherit = true,
    webkit_text_stroke_width: Option<LengthDefaultsToZero> where inherit = true,
    webkit_text_stroke_color: Option<ColorInput> where inherit = true,
    webkit_text_fill_color: Option<ColorInput> where inherit = true,
    stroke_linejoin: LineJoin where inherit = true,
    text_shadow: Option<TextShadows> where inherit = true,
    text_decoration_line: Option<TextDecorationLines>,
    text_decoration_style: TextDecorationStyle,
    text_decoration_color: ColorInput,
    text_decoration_thickness: TextDecorationThickness,
    text_decoration_skip_ink: TextDecorationSkipInk where inherit = true,
    text_indent: TextIndent where inherit = true,
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
    direction: Direction where inherit = true,
    float: Float,
    clear: Clear,
    isolation: Isolation,
    mix_blend_mode: BlendMode,
    visibility: Visibility,
    vertical_align: VerticalAlign,
  }
  shorthands {
    animation: Animations => [AnimationName, AnimationDuration, AnimationDelay, AnimationTimingFunction, AnimationIterationCount, AnimationDirection, AnimationFillMode, AnimationPlayState] |value, target| {
      target.push(StyleDeclaration::animation_duration(value.iter().map(|animation| animation.duration).collect()));
      target.push(StyleDeclaration::animation_delay(value.iter().map(|animation| animation.delay).collect()));
      target.push(StyleDeclaration::animation_timing_function(
        value
          .iter()
          .map(|animation| animation.timing_function)
          .collect(),
      ));
      target.push(StyleDeclaration::animation_iteration_count(
        value
          .iter()
          .map(|animation| animation.iteration_count)
          .collect(),
      ));
      target.push(StyleDeclaration::animation_direction(
        value.iter().map(|animation| animation.direction).collect(),
      ));
      target.push(StyleDeclaration::animation_fill_mode(
        value.iter().map(|animation| animation.fill_mode).collect(),
      ));
      target.push(StyleDeclaration::animation_play_state(
        value.iter().map(|animation| animation.play_state).collect(),
      ));
      target.push(StyleDeclaration::animation_name(value.into_iter().map(|animation| animation.name).collect()));
    },
    padding: Sides<LengthDefaultsToZero> => [PaddingTop, PaddingRight, PaddingBottom, PaddingLeft] |value, target| {
      push_four_side_declarations!(
        target,
        value.0,
        padding_top,
        padding_right,
        padding_bottom,
        padding_left
      );
    },
    padding_inline: SpacePair<LengthDefaultsToZero> => [PaddingLeft, PaddingRight] |value, target| {
      push_axis_declarations!(target, value, padding_left, padding_right);
    },
    padding_block: SpacePair<LengthDefaultsToZero> => [PaddingTop, PaddingBottom] |value, target| {
      push_axis_declarations!(target, value, padding_top, padding_bottom);
    },
    margin: Sides<LengthDefaultsToZero> => [MarginTop, MarginRight, MarginBottom, MarginLeft] |value, target| {
      push_four_side_declarations!(
        target,
        value.0,
        margin_top,
        margin_right,
        margin_bottom,
        margin_left
      );
    },
    margin_inline: SpacePair<LengthDefaultsToZero> => [MarginLeft, MarginRight] |value, target| {
      push_axis_declarations!(target, value, margin_left, margin_right);
    },
    margin_block: SpacePair<LengthDefaultsToZero> => [MarginTop, MarginBottom] |value, target| {
      push_axis_declarations!(target, value, margin_top, margin_bottom);
    },
    inset: Sides<Length> => [Top, Right, Bottom, Left] |value, target| {
      push_four_side_declarations!(target, value.0, top, right, bottom, left);
    },
    inset_inline: SpacePair<Length> => [Left, Right] |value, target| {
      push_axis_declarations!(target, value, left, right);
    },
    inset_block: SpacePair<Length> => [Top, Bottom] |value, target| {
      push_axis_declarations!(target, value, top, bottom);
    },
    mask: Backgrounds => [MaskImage, MaskPosition, MaskSize, MaskRepeat] |value, target| {
      target.push(StyleDeclaration::mask_position(
        value.iter().map(|background| background.position).collect(),
      ));
      target.push(StyleDeclaration::mask_size(
        value.iter().map(|background| background.size).collect(),
      ));
      target.push(StyleDeclaration::mask_repeat(
        value.iter().map(|background| background.repeat).collect(),
      ));
      target.push(StyleDeclaration::mask_image(Some(
        value
          .into_iter()
          .map(|background| background.image)
          .collect(),
      )));
    },
    gap: SpacePair<LengthDefaultsToZero> => [RowGap, ColumnGap] |value, target| {
      push_axis_declarations!(target, value, row_gap, column_gap);
    },
    flex_flow: FlexFlow => [FlexDirection, FlexWrap] |value, target| {
      target.push(StyleDeclaration::flex_direction(value.direction));
      target.push(StyleDeclaration::flex_wrap(value.wrap));
    },
    flex: Option<Flex> => [FlexGrow, FlexShrink, FlexBasis] |value, target| {
      target.push(StyleDeclaration::flex_grow(
        value.map(|value| FlexGrow(value.grow)),
      ));
      target.push(StyleDeclaration::flex_shrink(
        value.map(|value| FlexGrow(value.shrink)),
      ));
      target.push(StyleDeclaration::flex_basis(value.map(|value| value.basis)));
    },
    place_items: PlaceItems => [AlignItems, JustifyItems] |value, target| {
      target.push(StyleDeclaration::align_items(value.align));
      target.push(StyleDeclaration::justify_items(value.justify));
    },
    place_content: PlaceContent => [AlignContent, JustifyContent] |value, target| {
      target.push(StyleDeclaration::align_content(value.align));
      target.push(StyleDeclaration::justify_content(value.justify));
    },
    place_self: PlaceSelf => [AlignSelf, JustifySelf] |value, target| {
      target.push(StyleDeclaration::align_self(value.align));
      target.push(StyleDeclaration::justify_self(value.justify));
    },
    grid_column: GridLine => [GridColumnStart, GridColumnEnd] |value, target| {
      target.push(StyleDeclaration::grid_column_start(value.start));
      target.push(StyleDeclaration::grid_column_end(value.end));
    },
    grid_row: GridLine => [GridRowStart, GridRowEnd] |value, target| {
      target.push(StyleDeclaration::grid_row_start(value.start));
      target.push(StyleDeclaration::grid_row_end(value.end));
    },
    grid_area: GridArea => [GridRowStart, GridColumnStart, GridRowEnd, GridColumnEnd] |value, target| {
      target.push(StyleDeclaration::grid_row_start(value.row_start));
      target.push(StyleDeclaration::grid_column_start(value.column_start));
      target.push(StyleDeclaration::grid_row_end(value.row_end));
      target.push(StyleDeclaration::grid_column_end(value.column_end));
    },
    border_radius: BorderRadius => [BorderTopLeftRadius, BorderTopRightRadius, BorderBottomRightRadius, BorderBottomLeftRadius] |value, target| {
      push_four_side_declarations!(
        target,
        value.0.0,
        border_top_left_radius,
        border_top_right_radius,
        border_bottom_right_radius,
        border_bottom_left_radius
      );
    },
    border_width: Sides<Length> => [BorderTopWidth, BorderRightWidth, BorderBottomWidth, BorderLeftWidth] |value, target| {
      push_four_side_declarations!(
        target,
        value.0,
        border_top_width,
        border_right_width,
        border_bottom_width,
        border_left_width
      );
    },
    border_inline_width: SpacePair<Length> => [BorderLeftWidth, BorderRightWidth] |value, target| {
      push_axis_declarations!(
        target,
        value,
        border_left_width,
        border_right_width
      );
    },
    border_block_width: SpacePair<Length> => [BorderTopWidth, BorderBottomWidth] |value, target| {
      push_axis_declarations!(
        target,
        value,
        border_top_width,
        border_bottom_width
      );
    },
    border: Border => [BorderTopWidth, BorderRightWidth, BorderBottomWidth, BorderLeftWidth, BorderStyle, BorderColor] |value, target| {
      target.push(StyleDeclaration::border_top_width(value.width));
      target.push(StyleDeclaration::border_right_width(value.width));
      target.push(StyleDeclaration::border_bottom_width(value.width));
      target.push(StyleDeclaration::border_left_width(value.width));
      target.push(StyleDeclaration::border_style(value.style));
      target.push(StyleDeclaration::border_color(value.color));
    },
    outline: Border => [OutlineWidth, OutlineStyle, OutlineColor] |value, target| {
      target.push(StyleDeclaration::outline_width(value.width));
      target.push(StyleDeclaration::outline_style(value.style));
      target.push(StyleDeclaration::outline_color(value.color));
    },
    overflow: SpacePair<Overflow> => [OverflowX, OverflowY] |value, target| {
      push_axis_declarations!(target, value, overflow_x, overflow_y);
    },
    background: Backgrounds => [BackgroundImage, BackgroundPosition, BackgroundSize, BackgroundRepeat, BackgroundBlendMode, BackgroundColor, BackgroundClip] |value, target| {
      target.push(StyleDeclaration::background_position(
        value.iter().map(|background| background.position).collect(),
      ));
      target.push(StyleDeclaration::background_size(
        value.iter().map(|background| background.size).collect(),
      ));
      target.push(StyleDeclaration::background_repeat(
        value.iter().map(|background| background.repeat).collect(),
      ));
      target.push(StyleDeclaration::background_blend_mode(
        value
          .iter()
          .map(|background| background.blend_mode)
          .collect(),
      ));
      target.push(StyleDeclaration::background_color(
        value
          .iter()
          .filter_map(|background| background.color)
          .next_back()
          .unwrap_or_default(),
      ));
      target.push(StyleDeclaration::background_clip(
        value
          .last()
          .map(|background| background.clip)
          .unwrap_or_default(),
      ));
      target.push(StyleDeclaration::background_image(Some(
        value
          .into_iter()
          .map(|background| background.image)
          .collect(),
      )));
    },
    font_synthesis: FontSynthesis where inherit = true => [FontSynthesisWeight, FontSynthesisStyle] |value, target| {
      target.push(StyleDeclaration::font_synthesis_weight(value.weight));
      target.push(StyleDeclaration::font_synthesis_style(value.style));
    },
    webkit_text_stroke: Option<TextStroke> where inherit = true => [WebkitTextStrokeWidth, WebkitTextStrokeColor] |value, target| {
      target.push(StyleDeclaration::webkit_text_stroke_width(
        value.map(|value| value.width),
      ));
      target.push(StyleDeclaration::webkit_text_stroke_color(
        value.and_then(|value| value.color),
      ));
    },
    text_decoration: TextDecoration => [TextDecorationLine, TextDecorationStyle, TextDecorationColor, TextDecorationThickness] |value, target| {
      target.push(StyleDeclaration::text_decoration_line(Some(value.line)));
      target.push(StyleDeclaration::text_decoration_style(value.style));
      target.push(StyleDeclaration::text_decoration_color(value.color));
      target.push(StyleDeclaration::text_decoration_thickness(value.thickness));
    },
    white_space: WhiteSpace where inherit = true => [TextWrapMode, WhiteSpaceCollapse] |value, target| {
      target.push(StyleDeclaration::text_wrap_mode(value.text_wrap_mode));
      target.push(StyleDeclaration::white_space_collapse(
        value.white_space_collapse,
      ));
    },
    text_wrap: TextWrap where inherit = true => [TextWrapMode, TextWrapStyle] |value, target| {
      target.push(StyleDeclaration::text_wrap_mode(value.mode));
      target.push(StyleDeclaration::text_wrap_style(value.style));
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

impl<'i> FromCss<'i> for CssWideKeyword {
  fn from_css(input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    let location = input.current_source_location();
    let ident = input.expect_ident_cloned()?;

    match_ignore_ascii_case! { ident.as_ref(),
      "initial" => Ok(Self::Initial),
      "inherit" => Ok(Self::Inherit),
      "unset" => Ok(Self::Unset),
      _ => Err(unexpected_token!(location, &Token::Ident(ident))),
    }
  }

  const VALID_TOKENS: &'static [CssToken] = &[
    CssToken::Keyword("initial"),
    CssToken::Keyword("inherit"),
    CssToken::Keyword("unset"),
  ];
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PropertyMask {
  words: [usize; Self::WORD_COUNT],
}

impl PropertyMask {
  const BITS_PER_WORD: usize = usize::BITS as usize;
  const WORD_COUNT: usize = LonghandId::COUNT.div_ceil(Self::BITS_PER_WORD);

  pub(crate) const fn new() -> Self {
    Self {
      words: [0; Self::WORD_COUNT],
    }
  }

  pub(crate) fn insert(&mut self, property: LonghandId) -> bool {
    let word_index = property.index() / Self::BITS_PER_WORD;
    let bit_index = property.index() % Self::BITS_PER_WORD;
    let bit = 1usize << bit_index;
    let word = &mut self.words[word_index];
    let was_present = (*word & bit) != 0;
    *word |= bit;
    !was_present
  }

  pub(crate) fn contains(&self, property: &LonghandId) -> bool {
    let word_index = property.index() / Self::BITS_PER_WORD;
    let bit_index = property.index() % Self::BITS_PER_WORD;
    (self.words[word_index] & (1usize << bit_index)) != 0
  }

  pub(crate) fn append(&mut self, other: &mut Self) {
    for (word, other_word) in self.words.iter_mut().zip(other.words.iter_mut()) {
      *word |= *other_word;
      *other_word = 0;
    }
  }

  pub(crate) fn iter(&self) -> PropertyMaskIter<'_> {
    PropertyMaskIter {
      mask: self,
      next_index: 0,
    }
  }
}

impl Default for PropertyMask {
  fn default() -> Self {
    Self::new()
  }
}

impl Extend<LonghandId> for PropertyMask {
  fn extend<T: IntoIterator<Item = LonghandId>>(&mut self, iter: T) {
    for property in iter {
      self.insert(property);
    }
  }
}

impl FromIterator<LonghandId> for PropertyMask {
  fn from_iter<T: IntoIterator<Item = LonghandId>>(iter: T) -> Self {
    let mut mask = Self::new();
    mask.extend(iter);
    mask
  }
}

pub(crate) struct PropertyMaskIter<'a> {
  mask: &'a PropertyMask,
  next_index: usize,
}

impl Iterator for PropertyMaskIter<'_> {
  type Item = LonghandId;

  fn next(&mut self) -> Option<Self::Item> {
    while self.next_index < LonghandId::COUNT {
      let property = LonghandId::ALL[self.next_index];
      self.next_index += 1;
      if self.mask.contains(&property) {
        return Some(property);
      }
    }

    None
  }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct DeclarationImportance {
  pub(crate) longhands: PropertyMask,
  pub(crate) custom_properties: SmallVec<[Box<str>; 1]>,
}

impl DeclarationImportance {
  pub(crate) fn is_empty(&self) -> bool {
    self.custom_properties.is_empty() && self.longhands.iter().next().is_none()
  }

  pub(crate) fn insert_declaration(&mut self, declaration: &StyleDeclaration) {
    self
      .longhands
      .extend(declaration.affected_longhands().iter());

    if let StyleDeclaration::CustomProperty(name, _) = declaration {
      self.insert_custom_property(name);
    }
  }

  pub(crate) fn append(&mut self, other: &mut Self) {
    self.longhands.append(&mut other.longhands);

    for name in other.custom_properties.drain(..) {
      if self
        .custom_properties
        .iter()
        .all(|existing| existing != &name)
      {
        self.custom_properties.push(name);
      }
    }
  }

  fn insert_custom_property(&mut self, name: &str) {
    if self
      .custom_properties
      .iter()
      .all(|existing| existing.as_ref() != name)
    {
      self.custom_properties.push(name.into());
    }
  }
}

impl<T> From<T> for DeclarationImportance
where
  T: IntoIterator<Item = LonghandId>,
{
  fn from(value: T) -> Self {
    Self {
      longhands: value.into_iter().collect(),
      custom_properties: SmallVec::new(),
    }
  }
}

/// Ordered specified declarations plus the set of important properties.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct StyleDeclarationBlock {
  /// Ordered declarations in source order.
  pub(crate) declarations: SmallVec<[StyleDeclaration; 8]>,
  /// Properties that were marked with `!important`.
  pub(crate) importance: DeclarationImportance,
}

impl StyleDeclarationBlock {
  fn from_parsed_declarations(declarations: ParsedDeclarations, important: bool) -> Self {
    let mut block = Self::default();
    block.append_parsed_declarations(declarations, important);
    block
  }

  /// Appends a declaration and records whether it was important.
  pub(crate) fn push(&mut self, declaration: StyleDeclaration, important: bool) {
    if important {
      self.importance.insert_declaration(&declaration);
    }
    self.declarations.push(declaration);
  }

  fn append_parsed_declarations(&mut self, declarations: ParsedDeclarations, important: bool) {
    for declaration in declarations {
      self.push(declaration, important);
    }
  }

  pub(crate) fn append(&mut self, mut other: Self) {
    self.importance.append(&mut other.importance);
    self.declarations.extend(other.declarations);
  }

  /// Iterates over the declarations in source order.
  pub fn iter(&self) -> std::slice::Iter<'_, StyleDeclaration> {
    self.declarations.iter()
  }

  /// Collects resource URLs referenced by declarations in this block.
  pub fn resource_urls(&self) -> impl Iterator<Item = &str> {
    self
      .iter()
      .filter_map(|declaration| match declaration {
        StyleDeclaration::BackgroundImage(Some(images))
        | StyleDeclaration::MaskImage(Some(images)) => Some(images.iter().filter_map(|image| {
          if let BackgroundImage::Url(url) = image {
            Some(url.as_ref())
          } else {
            None
          }
        })),
        _ => None,
      })
      .flatten()
  }

  pub(crate) fn parse<'i>(name: &str, input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    parse_style_declaration(name, input)
  }
}

impl FromStr for StyleDeclarationBlock {
  type Err = StyleDeclarationBlockParseError;

  fn from_str(input: &str) -> Result<Self, Self::Err> {
    let mut parser_input = ParserInput::new(input);
    let mut parser = Parser::new(&mut parser_input);
    let mut declaration_parser = StyleDeclarationParser;
    let mut block = Self::default();

    for result in RuleBodyParser::new(&mut parser, &mut declaration_parser) {
      match result {
        Ok(declarations) => block.append(declarations),
        Err((error, context)) => {
          return Err(StyleDeclarationBlockParseError::InvalidDeclarationBlock {
            input: input.to_owned(),
            context: context.to_owned(),
            reason: format!("{error:?}"),
          });
        }
      }
    }

    Ok(block)
  }
}

/// Sized font style with computed font size and line height.
#[derive(Clone)]
pub(crate) struct SizedFontStyle<'s> {
  pub parent: &'s ComputedStyle,
  pub line_height: parley::LineHeight,
  pub stroke_width: f32,
  pub outline_width: f32,
  pub outline_offset: f32,
  pub letter_spacing: f32,
  pub word_spacing: f32,
  pub text_shadow: SmallVec<[SizedShadow; 4]>,
  pub color: Color,
  pub outline_color: Color,
  pub outline_style: BorderStyle,
  pub text_stroke_color: Color,
  pub text_decoration_color: Color,
  pub text_decoration_thickness: SizedTextDecorationThickness,
  pub sizing: Sizing,
}

impl<'s> From<&'s SizedFontStyle<'s>> for TextStyle<'s, 's, InlineBrush> {
  fn from(style: &'s SizedFontStyle<'s>) -> Self {
    TextStyle {
      font_size: style.sizing.font_size,
      line_height: style.line_height,
      font_weight: style.parent.font_weight.into(),
      font_style: style.parent.font_style.into(),
      font_variations: FontVariations::List(Cow::Borrowed(
        style.parent.font_variation_settings.as_ref(),
      )),
      font_features: FontFeatures::List(Cow::Borrowed(style.parent.font_feature_settings.as_ref())),
      font_family: (&style.parent.font_family).into(),
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
        source_span_id: None,
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

impl ComputedStyle {
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

    // https://www.w3.org/TR/css-display-3/#transformations
    // Elements with position: absolute or fixed are blockified
    if self.position == Position::Absolute || self.float != Float::None {
      self.display.blockify();
    }
  }

  pub(crate) fn is_invisible(&self) -> bool {
    self.opacity.0 == 0.0 || self.display == Display::None || self.visibility == Visibility::Hidden
  }

  pub(crate) fn is_z_index_applicable(&self, is_flex_or_grid_item: bool) -> bool {
    !matches!(self.z_index, ZIndex::Auto)
      && (matches!(self.position, Position::Absolute | Position::Relative) || is_flex_or_grid_item)
  }

  pub(crate) fn participates_in_positioned_paint_bucket(&self, is_flex_or_grid_item: bool) -> bool {
    matches!(self.position, Position::Absolute | Position::Relative)
      || self.is_z_index_applicable(is_flex_or_grid_item)
  }

  pub(crate) fn creates_stacking_context(
    &self,
    border_box: Size<f32>,
    sizing: &Sizing,
    is_flex_or_grid_item: bool,
  ) -> bool {
    self.isolation == Isolation::Isolate
      || self.is_z_index_applicable(is_flex_or_grid_item)
      || self.has_non_identity_transform(border_box, sizing)
      || self.needs_offscreen_compositing()
  }

  pub(crate) fn needs_offscreen_compositing(&self) -> bool {
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
      line_clamp = Some(Cow::Owned(self.single_line_ellipsis_clamp()));

      text_wrap_mode = TextWrapMode::Wrap;
    }

    (text_wrap_mode, line_clamp)
  }

  #[inline]
  fn single_line_ellipsis_clamp(&self) -> LineClamp {
    LineClamp {
      count: 1,
      ellipsis: Some(self.ellipsis_char().to_string()),
    }
  }

  #[inline]
  fn grid_template(
    components: &Option<GridTemplateComponents>,
    sizing: &Sizing,
  ) -> (Vec<taffy::GridTemplateComponent<String>>, Vec<Vec<String>>) {
    components.as_deref().map_or_else(
      || (Vec::new(), vec![Vec::new()]),
      |components| components.collect_components_and_names(sizing),
    )
  }

  #[inline]
  fn resolved_text_shadows(&self, context: &RenderContext) -> SmallVec<[SizedShadow; 4]> {
    self
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
      })
  }

  #[inline]
  fn resolved_text_decoration_thickness(&self, sizing: &Sizing) -> SizedTextDecorationThickness {
    match self.text_decoration_thickness {
      TextDecorationThickness::Length(Length::Auto) | TextDecorationThickness::FromFont => {
        SizedTextDecorationThickness::FromFont
      }
      TextDecorationThickness::Length(thickness) => {
        SizedTextDecorationThickness::Value(thickness.to_px(sizing, sizing.font_size))
      }
    }
  }

  pub(crate) fn to_sized_font_style(&'_ self, context: &RenderContext) -> SizedFontStyle<'_> {
    let line_height = self.line_height.into_parley(&context.sizing);

    SizedFontStyle {
      sizing: context.sizing.to_owned(),
      parent: self,
      line_height,
      stroke_width: self
        .webkit_text_stroke_width
        .unwrap_or_default()
        .to_px(&context.sizing, context.sizing.font_size),
      outline_width: self.outline_width.to_px(&context.sizing, 0.0).max(0.0),
      outline_offset: self.outline_offset.to_px(&context.sizing, 0.0),
      letter_spacing: self
        .letter_spacing
        .to_px(&context.sizing, context.sizing.font_size),
      word_spacing: self
        .word_spacing
        .to_px(&context.sizing, context.sizing.font_size),
      text_shadow: self.resolved_text_shadows(context),
      color: self
        .webkit_text_fill_color
        .unwrap_or(self.color)
        .resolve(context.current_color),
      outline_color: self.outline_color.resolve(context.current_color),
      outline_style: self.outline_style,
      text_stroke_color: self
        .webkit_text_stroke_color
        .unwrap_or_default()
        .resolve(context.current_color),
      text_decoration_color: self.text_decoration_color.resolve(context.current_color),
      text_decoration_thickness: self.resolved_text_decoration_thickness(&context.sizing),
    }
  }

  pub(crate) fn to_taffy_style(&self, sizing: &Sizing) -> taffy::Style {
    // Convert grid templates and associated line names
    let (grid_template_columns, grid_template_column_names) =
      Self::grid_template(&self.grid_template_columns, sizing);
    let (grid_template_rows, grid_template_row_names) =
      Self::grid_template(&self.grid_template_rows, sizing);

    taffy::Style {
      float: self.float.resolve(self.direction),
      clear: self.clear.resolve(self.direction),
      direction: self.direction.into(),
      box_sizing: self.box_sizing.into(),
      size: Size {
        width: self.width,
        height: self.height,
      }
      .map(|length| length.resolve_to_dimension(sizing)),
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
      gap: Size {
        width: self.column_gap.resolve_to_length_percentage(sizing),
        height: self.row_gap.resolve_to_length_percentage(sizing),
      },
      flex_basis: self
        .flex_basis
        .unwrap_or(Length::Auto)
        .resolve_to_dimension(sizing),
      flex_shrink: self.flex_shrink.map(|shrink| shrink.0).unwrap_or(1.0),
      flex_wrap: self.flex_wrap.into(),
      min_size: Size {
        width: self.min_width,
        height: self.min_height,
      }
      .map(|length| length.resolve_to_dimension(sizing)),
      max_size: Size {
        width: self.max_width,
        height: self.max_height,
      }
      .map(|length| length.resolve_to_dimension(sizing)),
      grid_auto_columns: self
        .grid_auto_columns
        .as_ref()
        .map_or_else(Vec::new, |tracks| {
          tracks
            .iter()
            .map(|track| track.to_min_max(sizing))
            .collect()
        }),
      grid_auto_rows: self
        .grid_auto_rows
        .as_ref()
        .map_or_else(Vec::new, |tracks| {
          tracks
            .iter()
            .map(|track| track.to_min_max(sizing))
            .collect()
        }),
      grid_auto_flow: self.grid_auto_flow.into(),
      grid_column: Line {
        start: self.grid_column_start.clone().into(),
        end: self.grid_column_end.clone().into(),
      },
      grid_row: Line {
        start: self.grid_row_start.clone().into(),
        end: self.grid_row_end.clone().into(),
      },
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
  use std::{collections::HashMap, rc::Rc, str::FromStr};

  use cssparser::{Parser, ParserInput};
  use taffy::Size;

  use super::stylesheets_vars::resolve_var_references;
  use super::{CssWideKeyword, LonghandId, PropertyId, ShorthandId, StyleDeclarationBlock};
  use crate::{
    layout::{
      Viewport,
      style::{ComputedStyle, Style, StyleDeclaration, properties::*},
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
    declarations_result.unwrap_or_default()
  }

  fn inherited_style_from_pairs(
    declarations: impl IntoIterator<Item = (&'static str, &'static str)>,
    parent: &ComputedStyle,
  ) -> ComputedStyle {
    let mut style = Style::default();
    for (name, value) in declarations {
      style.append_block(parse_declarations(name, value));
    }
    style.inherit(parent)
  }

  fn resolve_var(
    specified_value: &str,
    custom_properties: impl IntoIterator<Item = (&'static str, &'static str)>,
  ) -> Option<String> {
    let custom_properties = custom_properties
      .into_iter()
      .map(|(name, value)| (name.to_owned(), value.to_owned()))
      .collect::<HashMap<_, _>>();

    resolve_var_references(specified_value, &custom_properties, &mut Vec::new())
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

    let resolved = tw_style.inherit(&ComputedStyle::default());
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
  fn custom_properties_map_to_custom_property_id() {
    assert_eq!(
      PropertyId::from_kebab_case("--padding-left"),
      PropertyId::Custom
    );
    assert_eq!(
      PropertyId::from_kebab_case("--webkit-mask-image"),
      PropertyId::Custom
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
    assert_eq!(
      PropertyId::from_camel_case("WebKitTextStroke"),
      PropertyId::Shorthand(ShorthandId::WebkitTextStroke)
    );
  }

  #[test]
  fn property_id_accepts_legacy_gap_aliases() {
    assert_eq!(
      PropertyId::from_kebab_case("grid-gap"),
      PropertyId::Shorthand(ShorthandId::Gap)
    );
    assert_eq!(
      PropertyId::from_camel_case("gridGap"),
      PropertyId::Shorthand(ShorthandId::Gap)
    );
    assert_eq!(
      PropertyId::from_kebab_case("grid-row-gap"),
      PropertyId::Longhand(LonghandId::RowGap)
    );
    assert_eq!(
      PropertyId::from_camel_case("gridColumnGap"),
      PropertyId::Longhand(LonghandId::ColumnGap)
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
  fn parse_style_declaration_parses_order_integer() {
    let declarations = parse_declarations("order", "-2");

    assert_eq!(
      declarations.iter().collect::<Vec<_>>(),
      vec![&StyleDeclaration::order(Order(-2))]
    );
  }

  #[test]
  fn parse_style_declaration_parses_z_index_values() {
    let numeric = parse_declarations("z-index", "8");
    assert_eq!(
      numeric.iter().collect::<Vec<_>>(),
      vec![&StyleDeclaration::z_index(ZIndex::Integer(8))]
    );

    let auto = parse_declarations("z-index", "auto");
    assert_eq!(
      auto.iter().collect::<Vec<_>>(),
      vec![&StyleDeclaration::z_index(ZIndex::Auto)]
    );
  }

  #[test]
  fn parse_style_declaration_expands_shorthands_in_order() {
    let declarations = parse_declarations("padding", "1px 2px");

    assert_eq!(
      declarations.iter().collect::<Vec<_>>(),
      vec![
        &StyleDeclaration::padding_top(Length::Px(1.0)),
        &StyleDeclaration::padding_right(Length::Px(2.0)),
        &StyleDeclaration::padding_bottom(Length::Px(1.0)),
        &StyleDeclaration::padding_left(Length::Px(2.0)),
      ]
    );
  }

  #[test]
  fn parse_style_declaration_expands_flex_flow() {
    let declarations = parse_declarations("flex-flow", "wrap row-reverse");

    assert_eq!(
      declarations.iter().collect::<Vec<_>>(),
      vec![
        &StyleDeclaration::flex_direction(FlexDirection::RowReverse),
        &StyleDeclaration::flex_wrap(FlexWrap::Wrap),
      ]
    );
  }

  #[test]
  fn parse_style_declaration_expands_place_items() {
    let declarations = parse_declarations("place-items", "center stretch");

    assert_eq!(
      declarations.iter().collect::<Vec<_>>(),
      vec![
        &StyleDeclaration::align_items(AlignItems::Center),
        &StyleDeclaration::justify_items(AlignItems::Stretch),
      ]
    );
  }

  #[test]
  fn parse_style_declaration_expands_place_content() {
    let declarations = parse_declarations("place-content", "space-between center");

    assert_eq!(
      declarations.iter().collect::<Vec<_>>(),
      vec![
        &StyleDeclaration::align_content(JustifyContent::SpaceBetween),
        &StyleDeclaration::justify_content(JustifyContent::Center),
      ]
    );
  }

  #[test]
  fn parse_style_declaration_expands_place_self() {
    let declarations = parse_declarations("place-self", "end stretch");

    assert_eq!(
      declarations.iter().collect::<Vec<_>>(),
      vec![
        &StyleDeclaration::align_self(AlignItems::End),
        &StyleDeclaration::justify_self(AlignItems::Stretch),
      ]
    );
  }

  #[test]
  fn parse_style_declaration_expands_grid_row_shorthand() {
    let declarations = parse_declarations("grid-row", "span 2 / 5");

    assert_eq!(
      declarations.iter().collect::<Vec<_>>(),
      vec![
        &StyleDeclaration::grid_row_start(GridPlacement::span(2)),
        &StyleDeclaration::grid_row_end(GridPlacement::Line(5)),
      ]
    );
  }

  #[test]
  fn parse_style_declaration_expands_grid_area_shorthand() {
    let declarations = parse_declarations("grid-area", "header / sidebar");

    assert_eq!(
      declarations.iter().collect::<Vec<_>>(),
      vec![
        &StyleDeclaration::grid_row_start(GridPlacement::Named("header".to_string())),
        &StyleDeclaration::grid_column_start(GridPlacement::Named("sidebar".to_string())),
        &StyleDeclaration::grid_row_end(GridPlacement::Named("header".to_string())),
        &StyleDeclaration::grid_column_end(GridPlacement::Named("sidebar".to_string())),
      ]
    );
  }

  #[test]
  fn parse_style_declaration_supports_legacy_grid_gap_aliases() {
    let row_gap = parse_declarations("grid-row-gap", "4px");
    assert_eq!(
      row_gap.iter().collect::<Vec<_>>(),
      vec![&StyleDeclaration::row_gap(LengthDefaultsToZero::Px(4.0))]
    );

    let column_gap = parse_declarations("grid-column-gap", "3px");
    assert_eq!(
      column_gap.iter().collect::<Vec<_>>(),
      vec![&StyleDeclaration::column_gap(LengthDefaultsToZero::Px(3.0))]
    );

    let gap = parse_declarations("grid-gap", "1px 2px");
    assert_eq!(
      gap.iter().collect::<Vec<_>>(),
      vec![
        &StyleDeclaration::row_gap(LengthDefaultsToZero::Px(1.0)),
        &StyleDeclaration::column_gap(LengthDefaultsToZero::Px(2.0)),
      ]
    );
  }

  #[test]
  fn parse_style_declaration_ignores_unknown_properties() {
    let declarations = parse_declarations("not-a-real-property", "123");

    assert!(declarations.iter().next().is_none());
  }

  #[test]
  fn style_declaration_block_from_str_parses_multiple_declarations() {
    let Ok(declarations) = StyleDeclarationBlock::from_str("color: #ff0000; padding: 1px 2px;")
    else {
      return;
    };

    assert_eq!(
      declarations.iter().collect::<Vec<_>>(),
      vec![
        &StyleDeclaration::color(ColorInput::Value(Color([255, 0, 0, 255]))),
        &StyleDeclaration::padding_top(Length::Px(1.0)),
        &StyleDeclaration::padding_right(Length::Px(2.0)),
        &StyleDeclaration::padding_bottom(Length::Px(1.0)),
        &StyleDeclaration::padding_left(Length::Px(2.0)),
      ]
    );
  }

  #[test]
  fn style_declaration_block_from_str_tracks_important_declarations() {
    let Ok(declarations) = StyleDeclarationBlock::from_str("color: inherit !important;") else {
      return;
    };

    assert_eq!(
      declarations.iter().collect::<Vec<_>>(),
      vec![&StyleDeclaration::CssWideKeyword(
        LonghandId::Color,
        CssWideKeyword::Inherit,
      )]
    );
    assert!(
      declarations
        .importance
        .longhands
        .contains(&LonghandId::Color)
    );
  }

  #[test]
  fn style_declaration_block_supports_reference_iteration() {
    let declarations = parse_declarations("padding", "1px 2px");

    assert_eq!(declarations.iter().count(), 4);
  }

  #[test]
  fn style_declaration_block_collect_style_fetch_tasks_collects_url_images() {
    let background_url = "https://placehold.co/80x80/22c55e/white";
    let mask_url = "https://placehold.co/40x40/000000/white";
    let declarations = StyleDeclarationBlock::from(style_with([
      StyleDeclaration::background_image(Some(
        [
          BackgroundImage::Url(background_url.into()),
          BackgroundImage::None,
        ]
        .into(),
      )),
      StyleDeclaration::mask_image(Some([BackgroundImage::Url(mask_url.into())].into())),
    ]));

    assert_eq!(
      declarations.resource_urls().collect::<Vec<_>>(),
      vec![background_url, mask_url]
    );
  }

  #[test]
  fn style_declaration_block_into_iter_yields_owned_values() {
    let declarations = parse_declarations("padding", "1px 2px");

    assert_eq!(
      declarations.iter().collect::<Vec<_>>(),
      vec![
        &StyleDeclaration::padding_top(Length::Px(1.0)),
        &StyleDeclaration::padding_right(Length::Px(2.0)),
        &StyleDeclaration::padding_bottom(Length::Px(1.0)),
        &StyleDeclaration::padding_left(Length::Px(2.0)),
      ]
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

    let inherited = preset_style.inherit(&ComputedStyle::default());
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

    let inherited = preset_style.inherit(&ComputedStyle::default());
    assert_eq!(inherited.background_color, ColorInput::default());
  }

  #[test]
  fn test_needs_offscreen_compositing_for_clip_path_and_mask_image() {
    let mut style = ComputedStyle::default();
    assert!(!style.needs_offscreen_compositing());

    style.clip_path = BasicShape::from_str("inset(10px)").ok();
    assert!(style.needs_offscreen_compositing());

    style.clip_path = None;
    style.mask_image = Some([BackgroundImage::Url("https://example.com/mask.png".into())].into());
    assert!(style.needs_offscreen_compositing());
  }

  #[test]
  fn test_is_z_index_applicable_matches_supported_scope() {
    let mut style = ComputedStyle {
      z_index: ZIndex::Integer(2),
      ..Default::default()
    };
    assert!(style.is_z_index_applicable(false));

    style.position = Position::Absolute;
    assert!(style.is_z_index_applicable(false));

    style.position = Position::Relative;
    assert!(style.is_z_index_applicable(false));

    style.z_index = ZIndex::Auto;
    assert!(!style.is_z_index_applicable(false));
  }

  #[test]
  fn test_creates_stacking_context_from_z_index_scope() {
    let mut style = ComputedStyle::default();
    let sizing = Sizing {
      viewport: Viewport::new((1200, 630)),
      container_size: Size::NONE,
      font_size: 16.0,
      calc_arena: Rc::new(CalcArena::default()),
    };
    let border_box = Size {
      width: 200.0,
      height: 100.0,
    };

    style.z_index = ZIndex::Integer(1);
    assert!(style.creates_stacking_context(border_box, &sizing, false));

    style.position = Position::Absolute;
    assert!(style.creates_stacking_context(border_box, &sizing, false));
  }

  #[test]
  fn test_relative_position_participates_in_positioned_paint_bucket() {
    let style = ComputedStyle {
      position: Position::Relative,
      ..Default::default()
    };
    assert!(style.participates_in_positioned_paint_bucket(false));
  }

  #[test]
  fn test_non_identity_transform_detection() {
    let mut style = ComputedStyle::default();
    let sizing = Sizing {
      viewport: Viewport::new((1200, 630)),
      container_size: Size::NONE,
      font_size: 16.0,
      calc_arena: Rc::new(CalcArena::default()),
    };
    let border_box = Size {
      width: 200.0,
      height: 100.0,
    };

    assert!(!style.has_non_identity_transform(border_box, &sizing));

    style.transform = Some([Transform::Rotate(Angle::new(0.0))].into());
    assert!(!style.has_non_identity_transform(border_box, &sizing));

    style.transform = Some([Transform::Rotate(Angle::new(10.0))].into());
    assert!(style.has_non_identity_transform(border_box, &sizing));
  }

  #[test]
  fn test_transform_creates_stacking_context_without_offscreen_compositing() {
    let mut style = ComputedStyle::default();
    let sizing = Sizing {
      viewport: Viewport::new((1200, 630)),
      container_size: Size::NONE,
      font_size: 16.0,
      calc_arena: Rc::new(CalcArena::default()),
    };
    let border_box = Size {
      width: 200.0,
      height: 100.0,
    };

    style.transform = Some([Transform::Rotate(Angle::new(10.0))].into());

    assert!(style.creates_stacking_context(border_box, &sizing, false));
    assert!(!style.needs_offscreen_compositing());
  }

  #[test]
  fn test_text_overflow_ellipsis_forces_single_line_clamp_on_nowrap() {
    let style = ComputedStyle {
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
  fn test_position_absolute_blockifies_inline_display() {
    let mut style = style_with([
      StyleDeclaration::display(Display::Inline),
      StyleDeclaration::position(Position::Absolute),
    ])
    .inherit(&ComputedStyle::default());

    let sizing = Sizing {
      viewport: Viewport::new((1200, 630)),
      container_size: Size::NONE,
      font_size: 16.0,
      calc_arena: Rc::new(CalcArena::default()),
    };

    style.make_computed(&sizing);

    assert_eq!(style.display, Display::Block);
  }

  #[test]
  fn test_inherited_em_text_lengths_are_computed_once() {
    let mut parent = style_with([
      StyleDeclaration::font_size(Length::Em(2.0).into()),
      StyleDeclaration::letter_spacing(Length::Em(1.0)),
      StyleDeclaration::line_height(LineHeight::Length(Length::Em(1.5))),
    ])
    .inherit(&ComputedStyle::default());
    parent.make_computed(&Sizing {
      viewport: Viewport::new((1200, 630)),
      container_size: Size::NONE,
      font_size: 32.0,
      calc_arena: Rc::new(CalcArena::default()),
    });

    let inherited_child = Style::default().inherit(&parent);
    let inherited_child_sizing = Sizing {
      viewport: Viewport::new((1200, 630)),
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
      viewport: Viewport::new((1200, 630)),
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

  #[test]
  fn test_var_resolves_local_custom_property() {
    let style = inherited_style_from_pairs(
      [("--size", "24px"), ("width", "var(--size)")],
      &ComputedStyle::default(),
    );

    assert_eq!(style.width, Length::Px(24.0));
  }

  #[test]
  fn test_var_uses_fallback_when_missing() {
    let style = inherited_style_from_pairs(
      [("width", "var(--missing, 18px)")],
      &ComputedStyle::default(),
    );

    assert_eq!(style.width, Length::Px(18.0));
  }

  #[test]
  fn test_var_supports_nested_custom_properties() {
    let style = inherited_style_from_pairs(
      [
        ("--space-base", "12px"),
        ("--space", "var(--space-base)"),
        ("padding-left", "var(--space)"),
      ],
      &ComputedStyle::default(),
    );

    assert_eq!(style.padding_left, Length::Px(12.0));
  }

  #[test]
  fn test_var_resolves_custom_property_declared_later_on_same_element() {
    let style = inherited_style_from_pairs(
      [("width", "var(--card-width)"), ("--card-width", "24px")],
      &ComputedStyle::default(),
    );

    assert_eq!(style.width, Length::Px(24.0));
  }

  #[test]
  fn test_var_inherits_custom_properties_from_parent() {
    let parent = inherited_style_from_pairs([("--card-width", "320px")], &ComputedStyle::default());
    let child = inherited_style_from_pairs([("width", "var(--card-width)")], &parent);

    assert_eq!(child.width, Length::Px(320.0));
  }

  #[test]
  fn test_var_drops_invalid_declaration_without_fallback() {
    let style =
      inherited_style_from_pairs([("width", "var(--missing)")], &ComputedStyle::default());

    assert_eq!(style.width, Length::default());
  }

  #[test]
  fn test_var_uses_fallback_for_cycles() {
    let style = inherited_style_from_pairs(
      [
        ("--a", "var(--b)"),
        ("--b", "var(--a)"),
        ("width", "var(--a, 14px)"),
      ],
      &ComputedStyle::default(),
    );

    assert_eq!(style.width, Length::Px(14.0));
  }

  #[test]
  fn test_var_resolves_inside_shorthand() {
    let style = inherited_style_from_pairs(
      [
        ("--block", "6px"),
        ("--inline", "10px"),
        ("padding", "var(--block) var(--inline)"),
      ],
      &ComputedStyle::default(),
    );

    assert_eq!(style.padding_top, Length::Px(6.0));
    assert_eq!(style.padding_right, Length::Px(10.0));
    assert_eq!(style.padding_bottom, Length::Px(6.0));
    assert_eq!(style.padding_left, Length::Px(10.0));
  }

  #[test]
  fn test_var_rejects_non_custom_property_name() {
    let style =
      inherited_style_from_pairs([("width", "var(size, 18px)")], &ComputedStyle::default());

    assert_eq!(style.width, Length::default());
  }

  #[test]
  fn test_var_allows_trailing_tokens_when_property_parser_is_loose() {
    let style = inherited_style_from_pairs(
      [("--size", "24px"), ("width", "var(--size) 10px")],
      &ComputedStyle::default(),
    );

    assert_eq!(style.width, Length::Px(24.0));
  }

  #[test]
  fn test_var_rejects_missing_separator_in_function() {
    let style = inherited_style_from_pairs(
      [("--size", "24px"), ("width", "var(--size 18px)")],
      &ComputedStyle::default(),
    );

    assert_eq!(style.width, Length::default());
  }

  #[test]
  fn test_var_supports_nested_fallback_chains() {
    let style = inherited_style_from_pairs(
      [
        ("--backup", "22px"),
        ("width", "var(--missing, var(--backup, 14px))"),
      ],
      &ComputedStyle::default(),
    );

    assert_eq!(style.width, Length::Px(22.0));
  }

  #[test]
  fn test_var_resolves_inside_nested_functions() {
    let resolved = resolve_var("calc(var(--space) + 2px)", [("--space", "8px")]);

    assert_eq!(resolved.as_deref(), Some("calc(8px + 2px)"));
  }

  #[test]
  fn test_var_resolves_inside_nested_blocks() {
    let resolved = resolve_var(
      "(var(--x)) [var(--y)] {var(--z)}",
      [("--x", "1px"), ("--y", "2px"), ("--z", "3px")],
    );

    assert_eq!(resolved.as_deref(), Some("(1px) [2px] {3px}"));
  }

  #[test]
  fn test_var_drops_declaration_when_substitution_stays_invalid() {
    let style = inherited_style_from_pairs(
      [("--size", "red"), ("width", "var(--size)")],
      &ComputedStyle::default(),
    );

    assert_eq!(style.width, Length::default());
  }

  #[test]
  fn test_border_radius_calc_infinity_parses_from_stylesheet_declaration() {
    let style = inherited_style_from_pairs(
      [("border-radius", "calc(infinity * 1px)")],
      &ComputedStyle::default(),
    );
    let sizing = Sizing {
      viewport: Viewport::new((1200, 630)),
      container_size: Size::NONE,
      font_size: 16.0,
      calc_arena: Rc::new(CalcArena::default()),
    };
    let radius = style.border_top_left_radius.x.to_px(
      &sizing,
      sizing.viewport.size.width.unwrap_or_default() as f32,
    );

    assert_eq!(
      radius,
      i32::MAX as f32,
      "parsed={:?}",
      style.border_top_left_radius.x
    );
  }
}
