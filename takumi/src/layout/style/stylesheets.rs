use std::{borrow::Cow, collections::HashMap, marker::PhantomData};

use cssparser::{Parser, ParserInput, Token, match_ignore_ascii_case};
use parley::{FontSettings, FontStack, TextStyle};
use paste::paste;
use serde::de::IgnoredAny;
use smallvec::SmallVec;
use taffy::{Point, Rect, Size, prelude::FromLength};

#[cfg(feature = "css_stylesheet_parsing")]
use crate::layout::style::selector::PropertyRule;
use crate::{
  layout::{
    inline::InlineBrush,
    style::{RawCssInput, RawCssValueSeed, properties::*},
  },
  rendering::{RenderContext, SizedShadow, Sizing},
};

macro_rules! define_inherited_default {
  ($parent:expr, $inherit:tt) => {
    $parent.to_owned()
  };
  ($parent:expr) => {
    Default::default()
  };
}

enum ParsedRawStyleValue<T> {
  Keyword(CssWideKeyword),
  Value(T),
}

enum ParsedDeclarations {
  None,
  Single(StyleDeclaration),
  Many(Vec<StyleDeclaration>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DeferredDeclaration {
  property: PropertyId,
  raw_value: String,
}

type ExpectedMessageFn = fn() -> super::CssExpectedMessage<'static>;

enum RawStyleValueParseError<'de> {
  Value {
    value: Cow<'de, str>,
    expected_message: ExpectedMessageFn,
  },
  NumberType {
    number: super::RawCssNumber,
    expected_message: ExpectedMessageFn,
  },
  UnexpectedType {
    unexpected: super::RawCssUnexpected,
    expected_message: ExpectedMessageFn,
  },
}

impl RawStyleValueParseError<'_> {
  fn into_serde_error<E>(self) -> E
  where
    E: serde::de::Error,
  {
    match self {
      Self::Value {
        value,
        expected_message,
      } => E::invalid_value(
        serde::de::Unexpected::Str(value.as_ref()),
        &expected_message(),
      ),
      Self::NumberType {
        number,
        expected_message,
      } => E::invalid_type(number.unexpected(), &expected_message()),
      Self::UnexpectedType {
        unexpected,
        expected_message,
      } => E::invalid_type(unexpected.as_serde_unexpected(), &expected_message()),
    }
  }
}

fn expected_message<T>() -> super::CssExpectedMessage<'static>
where
  T: for<'i> FromCss<'i>,
{
  super::css_expected_message::<T>()
}

fn parse_raw_style_value<'de, T>(
  raw_value: RawCssInput<'de>,
) -> Result<ParsedRawStyleValue<T>, RawStyleValueParseError<'de>>
where
  T: for<'i> FromCss<'i>,
{
  match raw_value {
    RawCssInput::Str(value) => {
      if let Ok(keyword) = CssWideKeyword::from_str(value.as_ref()) {
        Ok(ParsedRawStyleValue::Keyword(keyword))
      } else {
        let parsed_value = T::from_str(value.as_ref()).ok();

        let Some(parsed_value) = parsed_value else {
          return Err(RawStyleValueParseError::Value {
            value,
            expected_message: expected_message::<T>,
          });
        };

        Ok(ParsedRawStyleValue::Value(parsed_value))
      }
    }
    RawCssInput::Number(number) => {
      let source = number.to_string();

      T::from_str(&source)
        .map(ParsedRawStyleValue::Value)
        .map_err(|_| RawStyleValueParseError::NumberType {
          number,
          expected_message: expected_message::<T>,
        })
    }
    RawCssInput::Unexpected(unexpected) => Err(RawStyleValueParseError::UnexpectedType {
      unexpected,
      expected_message: expected_message::<T>,
    }),
  }
}

fn parse_longhand_declaration<'i, T>(
  input: &mut Parser<'i, '_>,
  longhand_id: LonghandId,
  to_declaration: impl FnOnce(T) -> StyleDeclaration,
) -> ParseResult<'i, StyleDeclaration>
where
  T: for<'t> FromCss<'t>,
{
  let state = input.state();
  let keyword = input.try_parse(CssWideKeyword::from_css).ok();

  if let Some(keyword) = keyword {
    Ok(StyleDeclaration::CssWideKeyword(longhand_id, keyword))
  } else {
    input.reset(&state);
    Ok(to_declaration(T::from_css(input)?))
  }
}

fn parse_raw_longhand_declaration<'de, T>(
  longhand_id: LonghandId,
  raw_value: RawCssInput<'de>,
  to_declaration: impl FnOnce(T) -> StyleDeclaration,
) -> Result<StyleDeclaration, RawStyleValueParseError<'de>>
where
  T: for<'t> FromCss<'t>,
{
  match parse_raw_style_value::<T>(raw_value)? {
    ParsedRawStyleValue::Keyword(keyword) => {
      Ok(StyleDeclaration::CssWideKeyword(longhand_id, keyword))
    }
    ParsedRawStyleValue::Value(value) => Ok(to_declaration(value)),
  }
}

fn expand_shorthand<T>(
  value: T,
  expand: impl FnOnce(T, &mut Vec<StyleDeclaration>),
) -> Vec<StyleDeclaration> {
  let mut declarations = Vec::new();
  expand(value, &mut declarations);
  declarations
}

fn normalize_kebab_property_name(name: &str) -> Option<String> {
  Some(
    name
      .chars()
      .map(|ch| match ch {
        '-' => '_',
        _ => ch.to_ascii_lowercase(),
      })
      .collect(),
  )
}

#[cfg(feature = "css_stylesheet_parsing")]
#[allow(clippy::too_many_arguments)]
fn interpolate_option_with_missing<T: Animatable + Clone>(
  target: &mut Option<T>,
  from: &Option<T>,
  to: &Option<T>,
  missing_from: T,
  missing_to: T,
  progress: f32,
  sizing: &Sizing,
  current_color: Color,
) {
  *target = match (from, to) {
    (Some(from), Some(to)) => {
      let mut value = from.clone();
      value.interpolate(from, to, progress, sizing, current_color);
      Some(value)
    }
    (Some(from), None) => {
      let mut value = from.clone();
      value.interpolate(from, &missing_to, progress, sizing, current_color);
      Some(value)
    }
    (None, Some(to)) => {
      let mut value = missing_from.clone();
      value.interpolate(&missing_from, to, progress, sizing, current_color);
      Some(value)
    }
    (None, None) => None,
  };
}

fn normalize_camel_property_name(name: &str) -> String {
  let mut normalized = String::with_capacity(name.len() + 4);
  for ch in name.chars() {
    if ch.is_ascii_uppercase() {
      normalized.push('_');
      normalized.push(ch.to_ascii_lowercase());
    } else {
      normalized.push(ch);
    }
  }

  normalized.trim_start_matches('_').to_owned()
}

fn parse_custom_property_declaration<'i>(
  name: &str,
  input: &mut Parser<'i, '_>,
) -> ParseResult<'i, ParsedDeclarations> {
  let start = input.position();
  while input.next_including_whitespace_and_comments().is_ok() {}

  Ok(ParsedDeclarations::Single(
    StyleDeclaration::CustomProperty(name.to_owned(), input.slice_from(start).trim().to_owned()),
  ))
}

fn contains_var_function(raw_value: &str) -> bool {
  fn contains_in_parser(input: &mut Parser<'_, '_>) -> bool {
    while let Ok(token) = input.next_including_whitespace_and_comments() {
      match token {
        Token::Function(name) if name.eq_ignore_ascii_case("var") => return true,
        Token::Function(_)
        | Token::ParenthesisBlock
        | Token::SquareBracketBlock
        | Token::CurlyBracketBlock => {
          if input
            .parse_nested_block(|input| {
              Ok::<_, cssparser::ParseError<'_, Cow<'_, str>>>(contains_in_parser(input))
            })
            .unwrap_or(false)
          {
            return true;
          }
        }
        _ => {}
      }
    }

    false
  }

  let mut parser_input = ParserInput::new(raw_value);
  let mut parser = Parser::new(&mut parser_input);
  contains_in_parser(&mut parser)
}

fn property_alias(name: &str) -> Option<PropertyId> {
  match name {
    "-webkit-text-stroke" | "textStroke" | "WebkitTextStroke" => {
      Some(PropertyId::Shorthand(ShorthandId::WebkitTextStroke))
    }
    "-webkit-text-stroke-width" | "textStrokeWidth" | "WebkitTextStrokeWidth" => {
      Some(PropertyId::Longhand(LonghandId::WebkitTextStrokeWidth))
    }
    "-webkit-text-stroke-color" | "textStrokeColor" | "WebkitTextStrokeColor" => {
      Some(PropertyId::Longhand(LonghandId::WebkitTextStrokeColor))
    }
    "-webkit-text-fill-color" | "textFillColor" | "WebkitTextFillColor" => {
      Some(PropertyId::Longhand(LonghandId::WebkitTextFillColor))
    }
    _ => None,
  }
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
        const COUNT: usize = 0 $(+ { let _ = Self::[<$longhand:camel>]; 1 })*;
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

      impl ShorthandId {
        const COUNT: usize = 0 $(+ { let _ = Self::[<$shorthand:camel>]; 1 })*;

        const fn index(self) -> usize {
          self as usize
        }
      }

      type LonghandParseFn =
        for<'i> fn(&mut cssparser::Parser<'i, '_>)
          -> ParseResult<'i, ParsedDeclarations>;
      type ShorthandParseFn =
        for<'i> fn(&mut cssparser::Parser<'i, '_>)
          -> ParseResult<'i, ParsedDeclarations>;

      $(
        fn [<parse_ $longhand _declarations>]<'i>(
          input: &mut cssparser::Parser<'i, '_>,
        ) -> ParseResult<'i, ParsedDeclarations> {
          Ok(ParsedDeclarations::Single(parse_longhand_declaration::<$longhand_ty>(
            input,
            LonghandId::[<$longhand:camel>],
            StyleDeclaration::[<$longhand:camel>],
          )?))
        }

        fn [<parse_raw_ $longhand _declarations>]<'de>(
          raw_value: RawCssInput<'de>,
        ) -> Result<ParsedDeclarations, RawStyleValueParseError<'de>> {
          Ok(ParsedDeclarations::Single(
            parse_raw_longhand_declaration::<$longhand_ty>(
              LonghandId::[<$longhand:camel>],
              raw_value,
              StyleDeclaration::[<$longhand:camel>],
            )?,
          ))
        }
      )*

      const LONGHAND_PARSE_FNS: [LonghandParseFn; LonghandId::COUNT] = [
        $([<parse_ $longhand _declarations>],)*
      ];
      const RAW_LONGHAND_PARSE_FNS: [for<'de> fn(RawCssInput<'de>) -> Result<ParsedDeclarations, RawStyleValueParseError<'de>>; LonghandId::COUNT] = [
        $([<parse_raw_ $longhand _declarations>],)*
      ];

      $(
        fn [<parse_ $shorthand _declarations>]<'i>(
          input: &mut cssparser::Parser<'i, '_>,
        ) -> ParseResult<'i, ParsedDeclarations> {
          Ok(ParsedDeclarations::Many(expand_shorthand(
            <$shorthand_ty as FromCss>::from_css(input)?,
            |$value, $target_var| {
              $expand
            },
          )))
        }

        fn [<parse_raw_ $shorthand _declarations>]<'de>(
          raw_value: RawCssInput<'de>,
        ) -> Result<ParsedDeclarations, RawStyleValueParseError<'de>> {
          match parse_raw_style_value::<$shorthand_ty>(raw_value)? {
            ParsedRawStyleValue::Keyword(keyword) => Ok(ParsedDeclarations::Many(vec![
              $(StyleDeclaration::CssWideKeyword(LonghandId::$target, keyword)),+
            ])),
            ParsedRawStyleValue::Value(value) => Ok(ParsedDeclarations::Many(expand_shorthand(
              value,
              |$value, $target_var| {
                $expand
              },
            ))),
          }
        }
      )*

      const SHORTHAND_PARSE_FNS: [ShorthandParseFn; ShorthandId::COUNT] = [
        $([<parse_ $shorthand _declarations>],)*
      ];
      const RAW_SHORTHAND_PARSE_FNS: [for<'de> fn(RawCssInput<'de>) -> Result<ParsedDeclarations, RawStyleValueParseError<'de>>; ShorthandId::COUNT] = [
        $([<parse_raw_ $shorthand _declarations>],)*
      ];

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
          if name.starts_with("--") {
            return Self::Custom;
          }

          if let Some(property) = property_alias(name) {
            return property;
          }

          normalize_kebab_property_name(name)
            .map_or(Self::Ignored, |normalized| Self::from_normalized_name(&normalized))
        }

        #[allow(dead_code)]
        pub(crate) fn from_camel_case(name: &str) -> Self {
          if name.starts_with("--") {
            return Self::Custom;
          }

          if let Some(property) = property_alias(name) {
            return property;
          }

          Self::from_normalized_name(&normalize_camel_property_name(name))
        }

      fn parse_declarations<'i>(
        self,
        name: &str,
        input: &mut cssparser::Parser<'i, '_>,
      ) -> ParseResult<'i, ParsedDeclarations> {
        match self {
          Self::Ignored => {
            while input.next_including_whitespace_and_comments().is_ok() {}
            Ok(ParsedDeclarations::None)
            }
            Self::Custom => parse_custom_property_declaration(name, input),
            Self::Shorthand(property) => SHORTHAND_PARSE_FNS[property.index()](input),
            Self::Longhand(property) => LONGHAND_PARSE_FNS[property.index()](input),
          }
        }

        fn parse_raw_declarations<'de, E>(
          self,
          raw_value: RawCssInput<'de>,
        ) -> Result<ParsedDeclarations, E>
        where
          E: serde::de::Error,
        {
          debug_assert!(
            !matches!(self, Self::Custom),
            "custom properties should be handled before parse_raw_declarations",
          );

          let raw_string = match &raw_value {
            RawCssInput::Str(value) => Some(value.as_ref()),
            RawCssInput::Number(_) => None,
            RawCssInput::Unexpected(_) => None,
          };

          if raw_string.is_some_and(contains_var_function) {
            return Ok(ParsedDeclarations::Single(StyleDeclaration::Deferred(
              DeferredDeclaration {
                property: self,
                raw_value: raw_value.to_string(),
              },
            )));
          }

          match self {
            Self::Ignored => Ok(ParsedDeclarations::None),
            Self::Custom => unreachable!(),
            Self::Shorthand(property) => {
              RAW_SHORTHAND_PARSE_FNS[property.index()](raw_value)
                .map_err(RawStyleValueParseError::into_serde_error)
            }
            Self::Longhand(property) => {
              RAW_LONGHAND_PARSE_FNS[property.index()](raw_value)
                .map_err(RawStyleValueParseError::into_serde_error)
            }
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
            let raw_value = input.slice_from(start).trim();
            if contains_var_function(raw_value) {
              Ok(StyleDeclarationBlock::from_parsed_declarations(
                ParsedDeclarations::Single(StyleDeclaration::Deferred(DeferredDeclaration {
                  property,
                  raw_value: raw_value.to_owned(),
                })),
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

                let raw_value = map.next_value_seed(RawCssValueSeed)?;
                style.push_property_from_raw::<A::Error>(&key, property, raw_value, false)?;
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
        fn push_declarations(
          &mut self,
          declarations: impl IntoIterator<Item = StyleDeclaration>,
          important: bool,
        ) {
          self.declarations.push_declarations(declarations, important);
        }

        fn with_declarations(
          mut self,
          declarations: impl IntoIterator<Item = StyleDeclaration>,
          important: bool,
        ) -> Self {
          self.push_declarations(declarations, important);
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

        pub(crate) fn push(&mut self, declaration: StyleDeclaration, important: bool) {
          self.declarations.push(declaration, important);
        }

        pub(crate) fn append_block(&mut self, declarations: StyleDeclarationBlock) {
          self.declarations.append(declarations);
        }

        pub(crate) fn iter(&self) -> std::slice::Iter<'_, StyleDeclaration> {
          self.declarations.iter()
        }

        pub(crate) fn inherit(self, parent: &ComputedStyle) -> ComputedStyle {
          let mut style = ComputedStyle::from_parent(parent);
          for declaration in self.declarations.declarations {
            declaration.apply_with_parent(&mut style, parent);
          }
          style
        }

        pub(crate) fn merge_from(&mut self, other: Self) {
          self.append_block(other.declarations);
        }

        #[inline(never)]
        fn push_property_from_raw<'de, E>(
          &mut self,
          name: &str,
          property: PropertyId,
          raw_value: RawCssInput<'de>,
          important: bool,
        ) -> Result<(), E>
        where
          E: serde::de::Error,
        {
          if matches!(property, PropertyId::Custom) {
            self.declarations.push(
              StyleDeclaration::CustomProperty(name.to_owned(), raw_value.to_string()),
              important,
            );
            return Ok(());
          }

          self
            .declarations
            .append_parsed_declarations(property.parse_raw_declarations(raw_value)?, important);
          Ok(())
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
        #[cfg(feature = "css_stylesheet_parsing")]
        pub(crate) registered_custom_properties: HashMap<String, PropertyRule>,
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
            #[cfg(feature = "css_stylesheet_parsing")]
            registered_custom_properties: parent.registered_custom_properties.clone(),
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

          // special cases
          if animated_properties.contains(&LonghandId::FlexGrow) {
            interpolate_option_with_missing(
              &mut self.flex_grow,
              &from.flex_grow,
              &to.flex_grow,
              FlexGrow(0.0),
              FlexGrow(0.0),
              progress,
              sizing,
              current_color,
            );
          }

          if animated_properties.contains(&LonghandId::FlexShrink) {
            interpolate_option_with_missing(
              &mut self.flex_shrink,
              &from.flex_shrink,
              &to.flex_shrink,
              FlexGrow(1.0),
              FlexGrow(1.0),
              progress,
              sizing,
              current_color,
            );
          }

          if animated_properties.contains(&LonghandId::WebkitTextStrokeWidth) {
            interpolate_option_with_missing(
              &mut self.webkit_text_stroke_width,
              &from.webkit_text_stroke_width,
              &to.webkit_text_stroke_width,
              Length::zero(),
              Length::zero(),
              progress,
              sizing,
              current_color,
            );
          }

          if animated_properties.contains(&LonghandId::WebkitTextStrokeColor) {
            interpolate_option_with_missing(
              &mut self.webkit_text_stroke_color,
              &from.webkit_text_stroke_color,
              &to.webkit_text_stroke_color,
              ColorInput::CurrentColor,
              ColorInput::CurrentColor,
              progress,
              sizing,
              current_color,
            );
          }

          if animated_properties.contains(&LonghandId::WebkitTextFillColor) {
            interpolate_option_with_missing(
              &mut self.webkit_text_fill_color,
              &from.webkit_text_fill_color,
              &to.webkit_text_fill_color,
              from.color,
              to.color,
              progress,
              sizing,
              current_color,
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

        #[inline(never)]
        pub(crate) fn apply_with_parent(
          self,
          style: &mut ComputedStyle,
          parent: &ComputedStyle,
        ) {
          match self {
            Self::CssWideKeyword(property, keyword) => {
              apply_css_wide_keyword(style, parent, property, keyword)
            }
            Self::CustomProperty(name, raw_value) => {
              style.custom_properties.insert(name, raw_value);
            }
            Self::Deferred(deferred) => {
              apply_deferred_declaration(style, Some(parent), &deferred);
            }
            $(Self::[<$longhand:camel>](value) => style.$longhand = value,)*
          }
        }

        #[inline(never)]
        pub(crate) fn apply_to_computed(&self, style: &mut ComputedStyle) {
          match self {
            Self::CssWideKeyword(property, keyword) => match keyword {
              CssWideKeyword::Initial => apply_initial_longhand(style, *property),
              CssWideKeyword::Inherit | CssWideKeyword::Unset => {}
            },
            Self::CustomProperty(name, raw_value) => {
              style
                .custom_properties
                .insert(name.to_owned(), raw_value.to_owned());
            }
            Self::Deferred(deferred) => apply_deferred_declaration(style, None, deferred),
            $(Self::[<$longhand:camel>](value) => style.$longhand.clone_from(value),)*
          }
        }

        pub(crate) fn merge_into_ref(&self, style: &mut Style) {
          style.push(self.to_owned(), false);
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
                CssWideKeyword::Inherit => parent.$longhand.to_owned(),
                CssWideKeyword::Unset => define_inherited_default!(parent.$longhand $(, $longhand_inherit)?),
              };
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
    animation: Animations => [AnimationName, AnimationDuration, AnimationDelay, AnimationTimingFunction, AnimationIterationCount, AnimationDirection, AnimationFillMode, AnimationPlayState] |value, target| {
      expand_animation_shorthand(value, target);
    },
    padding: Sides<Length<false>> => [PaddingTop, PaddingRight, PaddingBottom, PaddingLeft] |value, target| {
      push_four_side_declarations!(
        target,
        value.0,
        padding_top,
        padding_right,
        padding_bottom,
        padding_left
      );
    },
    padding_inline: SpacePair<Length<false>> => [PaddingLeft, PaddingRight] |value, target| {
      push_axis_declarations!(target, value, padding_left, padding_right);
    },
    padding_block: SpacePair<Length<false>> => [PaddingTop, PaddingBottom] |value, target| {
      push_axis_declarations!(target, value, padding_top, padding_bottom);
    },
    margin: Sides<Length<false>> => [MarginTop, MarginRight, MarginBottom, MarginLeft] |value, target| {
      push_four_side_declarations!(
        target,
        value.0,
        margin_top,
        margin_right,
        margin_bottom,
        margin_left
      );
    },
    margin_inline: SpacePair<Length<false>> => [MarginLeft, MarginRight] |value, target| {
      push_axis_declarations!(target, value, margin_left, margin_right);
    },
    margin_block: SpacePair<Length<false>> => [MarginTop, MarginBottom] |value, target| {
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
      expand_mask_shorthand(value, target);
    },
    gap: Gap => [RowGap, ColumnGap] |value, target| {
      push_axis_declarations!(target, value, row_gap, column_gap);
    },
    flex: Option<Flex> => [FlexGrow, FlexShrink, FlexBasis] |value, target| {
      expand_flex_shorthand(value, target);
    },
    border_radius: Box<BorderRadius> => [BorderTopLeftRadius, BorderTopRightRadius, BorderBottomRightRadius, BorderBottomLeftRadius] |value, target| {
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
    border_inline_width: Option<SpacePair<Length>> => [BorderLeftWidth, BorderRightWidth] |value, target| {
      push_axis_declarations!(
        target,
        value.unwrap_or_default(),
        border_left_width,
        border_right_width
      );
    },
    border_block_width: Option<SpacePair<Length>> => [BorderTopWidth, BorderBottomWidth] |value, target| {
      push_axis_declarations!(
        target,
        value.unwrap_or_default(),
        border_top_width,
        border_bottom_width
      );
    },
    border: Border => [BorderTopWidth, BorderRightWidth, BorderBottomWidth, BorderLeftWidth, BorderStyle, BorderColor] |value, target| {
      expand_border_shorthand(value, target);
    },
    outline: Border => [OutlineWidth, OutlineStyle, OutlineColor] |value, target| {
      expand_outline_shorthand(value, target);
    },
    overflow: SpacePair<Overflow> => [OverflowX, OverflowY] |value, target| {
      push_axis_declarations!(target, value, overflow_x, overflow_y);
    },
    background: Backgrounds => [BackgroundImage, BackgroundPosition, BackgroundSize, BackgroundRepeat, BackgroundBlendMode, BackgroundColor, BackgroundClip] |value, target| {
      expand_background_shorthand(value, target);
    },
    font_synthesis: FontSynthesis where inherit = true => [FontSynthesisWeight, FontSynthesisStyle] |value, target| {
      expand_font_synthesis_shorthand(value, target);
    },
    webkit_text_stroke: Option<TextStroke> where inherit = true => [WebkitTextStrokeWidth, WebkitTextStrokeColor] |value, target| {
      expand_text_stroke_shorthand(value, target);
    },
    text_decoration: TextDecoration => [TextDecorationLine, TextDecorationStyle, TextDecorationColor, TextDecorationThickness] |value, target| {
      expand_text_decoration_shorthand(value, target);
    },
    white_space: WhiteSpace where inherit = true => [TextWrapMode, WhiteSpaceCollapse] |value, target| {
      expand_white_space_shorthand(value, target);
    },
    text_wrap: TextWrap where inherit = true => [TextWrapMode, TextWrapStyle] |value, target| {
      expand_text_wrap_shorthand(value, target);
    },
  }
}

fn expand_animation_shorthand(value: Animations, target: &mut Vec<StyleDeclaration>) {
  let has_animation_name = value.iter().any(|animation| animation.name.is_some());
  push_expanded_declarations!(
    target;
    StyleDeclaration::animation_duration(AnimationDurations(value.iter().map(|animation| animation.duration).collect())),
    StyleDeclaration::animation_delay(AnimationDurations(value.iter().map(|animation| animation.delay).collect())),
    StyleDeclaration::animation_timing_function(AnimationTimingFunctions(value.iter().map(|animation| animation.timing_function).collect())),
    StyleDeclaration::animation_iteration_count(AnimationIterationCounts(value.iter().map(|animation| animation.iteration_count).collect())),
    StyleDeclaration::animation_direction(AnimationDirections(value.iter().map(|animation| animation.direction).collect())),
    StyleDeclaration::animation_fill_mode(AnimationFillModes(value.iter().map(|animation| animation.fill_mode).collect())),
    StyleDeclaration::animation_play_state(AnimationPlayStates(value.iter().map(|animation| animation.play_state).collect())),
    StyleDeclaration::animation_name(if has_animation_name {
      AnimationNames(value.into_iter().map(|animation| animation.name.unwrap_or_default()).collect())
    } else {
      AnimationNames::default()
    }),
  );
}

fn expand_mask_shorthand(value: Backgrounds, target: &mut Vec<StyleDeclaration>) {
  push_expanded_declarations!(
    target;
    StyleDeclaration::mask_position(value.iter().map(|background| background.position).collect()),
    StyleDeclaration::mask_size(value.iter().map(|background| background.size).collect()),
    StyleDeclaration::mask_repeat(value.iter().map(|background| background.repeat).collect()),
    StyleDeclaration::mask_image(Some(value.into_iter().map(|background| background.image).collect())),
  );
}

fn expand_flex_shorthand(value: Option<Flex>, target: &mut Vec<StyleDeclaration>) {
  push_expanded_declarations!(
    target;
    StyleDeclaration::flex_grow(value.map(|value| FlexGrow(value.grow))),
    StyleDeclaration::flex_shrink(value.map(|value| FlexGrow(value.shrink))),
    StyleDeclaration::flex_basis(value.map(|value| value.basis)),
  );
}

fn expand_border_shorthand(value: Border, target: &mut Vec<StyleDeclaration>) {
  push_expanded_declarations!(
    target;
    StyleDeclaration::border_top_width(value.width),
    StyleDeclaration::border_right_width(value.width),
    StyleDeclaration::border_bottom_width(value.width),
    StyleDeclaration::border_left_width(value.width),
    StyleDeclaration::border_style(value.style),
    StyleDeclaration::border_color(value.color),
  );
}

fn expand_outline_shorthand(value: Border, target: &mut Vec<StyleDeclaration>) {
  push_expanded_declarations!(
    target;
    StyleDeclaration::outline_width(value.width),
    StyleDeclaration::outline_style(value.style),
    StyleDeclaration::outline_color(value.color),
  );
}

fn expand_background_shorthand(value: Backgrounds, target: &mut Vec<StyleDeclaration>) {
  push_expanded_declarations!(
    target;
    StyleDeclaration::background_position(value.iter().map(|background| background.position).collect()),
    StyleDeclaration::background_size(value.iter().map(|background| background.size).collect()),
    StyleDeclaration::background_repeat(value.iter().map(|background| background.repeat).collect()),
    StyleDeclaration::background_blend_mode(value.iter().map(|background| background.blend_mode).collect()),
    StyleDeclaration::background_color(value.iter().filter_map(|background| background.color).next_back().unwrap_or_default()),
    StyleDeclaration::background_clip(value.last().map(|background| background.clip).unwrap_or_default()),
    StyleDeclaration::background_image(Some(value.into_iter().map(|background| background.image).collect())),
  );
}

fn expand_font_synthesis_shorthand(value: FontSynthesis, target: &mut Vec<StyleDeclaration>) {
  push_expanded_declarations!(
    target;
    StyleDeclaration::font_synthesis_weight(value.weight),
    StyleDeclaration::font_synthesis_style(value.style),
  );
}

fn expand_text_stroke_shorthand(value: Option<TextStroke>, target: &mut Vec<StyleDeclaration>) {
  push_expanded_declarations!(
    target;
    StyleDeclaration::webkit_text_stroke_width(value.map(|value| value.width)),
    StyleDeclaration::webkit_text_stroke_color(value.and_then(|value| value.color)),
  );
}

fn expand_text_decoration_shorthand(value: TextDecoration, target: &mut Vec<StyleDeclaration>) {
  push_expanded_declarations!(
    target;
    StyleDeclaration::text_decoration_line(Some(value.line)),
    StyleDeclaration::text_decoration_style(value.style.unwrap_or_default()),
    StyleDeclaration::text_decoration_color(value.color.unwrap_or_default()),
    StyleDeclaration::text_decoration_thickness(value.thickness.unwrap_or_default()),
  );
}

fn expand_white_space_shorthand(value: WhiteSpace, target: &mut Vec<StyleDeclaration>) {
  push_expanded_declarations!(
    target;
    StyleDeclaration::text_wrap_mode(value.text_wrap_mode),
    StyleDeclaration::white_space_collapse(value.white_space_collapse),
  );
}

fn expand_text_wrap_shorthand(value: TextWrap, target: &mut Vec<StyleDeclaration>) {
  push_expanded_declarations!(
    target;
    StyleDeclaration::text_wrap_mode(value.mode.unwrap_or_default()),
    StyleDeclaration::text_wrap_style(value.style),
  );
}

fn resolve_custom_property_value(
  name: &str,
  custom_properties: &HashMap<String, String>,
  stack: &mut Vec<String>,
) -> Option<String> {
  if stack.iter().any(|entry| entry == name) {
    return None;
  }

  let raw_value = custom_properties.get(name)?;
  stack.push(name.to_owned());
  let resolved = resolve_var_references(raw_value, custom_properties, stack);
  stack.pop();
  resolved
}

fn resolve_var_function(
  input: &mut Parser<'_, '_>,
  custom_properties: &HashMap<String, String>,
  stack: &mut Vec<String>,
) -> Option<String> {
  let property_name = input.expect_ident_cloned().ok()?;
  if !property_name.starts_with("--") {
    return None;
  }

  let fallback = if input.try_parse(Parser::expect_comma).is_ok() {
    Some(resolve_var_tokens(input, custom_properties, stack)?)
  } else {
    None
  };

  if input.next_including_whitespace_and_comments().is_ok() {
    return None;
  }

  resolve_custom_property_value(property_name.as_ref(), custom_properties, stack).or(fallback)
}

fn resolve_var_tokens(
  input: &mut Parser<'_, '_>,
  custom_properties: &HashMap<String, String>,
  stack: &mut Vec<String>,
) -> Option<String> {
  let mut output = String::new();

  while !input.is_exhausted() {
    let start = input.position();
    let token = input.next_including_whitespace_and_comments().ok()?;

    match token {
      Token::Function(name) if name.eq_ignore_ascii_case("var") => {
        output.push_str(
          &input
            .parse_nested_block(|input| {
              resolve_var_function(input, custom_properties, stack)
                .ok_or_else(|| input.new_error_for_next_token::<()>())
            })
            .ok()?,
        );
      }
      Token::Function(name) => {
        output.push_str(name);
        output.push('(');
        let nested = input
          .parse_nested_block(|input| {
            resolve_var_tokens(input, custom_properties, stack)
              .ok_or_else(|| input.new_error_for_next_token::<()>())
          })
          .ok()?;
        output.push_str(&nested);
        output.push(')');
      }
      Token::ParenthesisBlock => {
        output.push('(');
        let nested = input
          .parse_nested_block(|input| {
            resolve_var_tokens(input, custom_properties, stack)
              .ok_or_else(|| input.new_error_for_next_token::<()>())
          })
          .ok()?;
        output.push_str(&nested);
        output.push(')');
      }
      Token::SquareBracketBlock => {
        output.push('[');
        let nested = input
          .parse_nested_block(|input| {
            resolve_var_tokens(input, custom_properties, stack)
              .ok_or_else(|| input.new_error_for_next_token::<()>())
          })
          .ok()?;
        output.push_str(&nested);
        output.push(']');
      }
      Token::CurlyBracketBlock => {
        output.push('{');
        let nested = input
          .parse_nested_block(|input| {
            resolve_var_tokens(input, custom_properties, stack)
              .ok_or_else(|| input.new_error_for_next_token::<()>())
          })
          .ok()?;
        output.push_str(&nested);
        output.push('}');
      }
      _ => output.push_str(input.slice_from(start)),
    }
  }

  Some(output)
}

fn resolve_var_references(
  raw_value: &str,
  custom_properties: &HashMap<String, String>,
  stack: &mut Vec<String>,
) -> Option<String> {
  let mut parser_input = ParserInput::new(raw_value);
  let mut parser = Parser::new(&mut parser_input);
  resolve_var_tokens(&mut parser, custom_properties, stack)
}

fn apply_resolved_declarations(
  style: &mut ComputedStyle,
  parent: Option<&ComputedStyle>,
  declarations: ParsedDeclarations,
) {
  match declarations {
    ParsedDeclarations::None => {}
    ParsedDeclarations::Single(declaration) => match parent {
      Some(parent) => declaration.apply_with_parent(style, parent),
      None => declaration.apply_to_computed(style),
    },
    ParsedDeclarations::Many(declarations) => {
      for declaration in declarations {
        match parent {
          Some(parent) => declaration.apply_with_parent(style, parent),
          None => declaration.apply_to_computed(style),
        }
      }
    }
  }
}

fn apply_deferred_declaration(
  style: &mut ComputedStyle,
  parent: Option<&ComputedStyle>,
  deferred: &DeferredDeclaration,
) {
  let Some(resolved_value) = resolve_var_references(
    &deferred.raw_value,
    &style.custom_properties,
    &mut Vec::new(),
  ) else {
    return;
  };

  let declarations = deferred
    .property
    .parse_raw_declarations::<serde::de::value::Error>(RawCssInput::Str(Cow::Owned(resolved_value)))
    .ok();

  let Some(declarations) = declarations else {
    return;
  };

  apply_resolved_declarations(style, parent, declarations);
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
      _ => Err(Self::unexpected_token_error(location, &Token::Ident(ident))),
    }
  }

  fn valid_tokens() -> &'static [CssToken] {
    &[
      CssToken::Keyword("initial"),
      CssToken::Keyword("inherit"),
      CssToken::Keyword("unset"),
    ]
  }
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
  longhands: PropertyMask,
  custom_properties: SmallVec<[Box<str>; 1]>,
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
    match declarations {
      ParsedDeclarations::None => {}
      ParsedDeclarations::Single(declaration) => self.push(declaration, important),
      ParsedDeclarations::Many(declarations) => {
        for declaration in declarations {
          self.push(declaration, important);
        }
      }
    }
  }

  fn push_declarations(
    &mut self,
    declarations: impl IntoIterator<Item = StyleDeclaration>,
    important: bool,
  ) {
    for declaration in declarations {
      self.push(declaration, important);
    }
  }

  pub(crate) fn append(&mut self, mut other: Self) {
    self.importance.append(&mut other.importance);
    self.declarations.extend(other.declarations);
  }

  /// Iterates over the declarations in source order.
  pub(crate) fn iter(&self) -> std::slice::Iter<'_, StyleDeclaration> {
    self.declarations.iter()
  }

  pub(crate) fn parse<'i>(name: &str, input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    parse_style_declaration(name, input)
  }
}

/// Sized font style with computed font size and line height.
#[derive(Clone)]
pub(crate) struct SizedFontStyle<'s> {
  pub parent: &'s ComputedStyle,
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
  fn resolved_gap(&self) -> SpacePair<Length<false>> {
    SpacePair::from_pair(self.row_gap, self.column_gap)
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
      gap: self.resolved_gap().resolve_to_size(sizing),
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
      grid_column: self
        .grid_column
        .as_ref()
        .map_or_else(Default::default, Into::into),
      grid_row: self
        .grid_row
        .as_ref()
        .map_or_else(Default::default, Into::into),
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
  use std::{collections::HashMap, rc::Rc};

  use cssparser::{Parser, ParserInput};
  use taffy::Size;

  use super::{
    CssWideKeyword, LonghandId, PropertyId, StyleDeclarationBlock, resolve_var_references,
  };
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
    let Ok(declarations) = declarations_result else {
      unreachable!()
    };
    declarations
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
    raw_value: &str,
    custom_properties: impl IntoIterator<Item = (&'static str, &'static str)>,
  ) -> Option<String> {
    let custom_properties = custom_properties
      .into_iter()
      .map(|(name, value)| (name.to_owned(), value.to_owned()))
      .collect::<HashMap<_, _>>();

    resolve_var_references(raw_value, &custom_properties, &mut Vec::new())
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
  fn parse_style_declaration_ignores_unknown_properties() {
    let declarations = parse_declarations("not-a-real-property", "123");

    assert!(declarations.iter().next().is_none());
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
  fn test_isolated_for_clip_path_and_mask_image() {
    let mut style = ComputedStyle::default();
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
    let mut style = ComputedStyle::default();
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
  fn test_inherited_em_text_lengths_are_computed_once() {
    let mut parent = style_with([
      StyleDeclaration::font_size(Length::Em(2.0).into()),
      StyleDeclaration::letter_spacing(Length::Em(1.0)),
      StyleDeclaration::line_height(LineHeight::Length(Length::Em(1.5))),
    ])
    .inherit(&ComputedStyle::default());
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
}
