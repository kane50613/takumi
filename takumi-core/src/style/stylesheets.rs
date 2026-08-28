use std::{
  borrow::Cow,
  collections::HashMap,
  fmt,
  str::FromStr,
  sync::{Arc, OnceLock},
};

use cssparser::{Parser, ParserInput, RuleBodyParser, Token, match_ignore_ascii_case};
use parley::Language;
use pastey::paste;
use serde::{
  Deserialize,
  de::{Error as DeError, IgnoredAny},
};
use smallvec::{SmallVec, smallvec};

use crate::{
  Error,
  error::StyleDeclarationBlockParseError,
  style::{
    CssInput, CssUnexpected, CssValueSeed, SizingContext,
    properties::*,
    selector::{PropertyRule, StyleDeclarationParser},
    unexpected_token,
  },
};
#[path = "stylesheets_helpers.rs"]
mod stylesheets_helpers;
#[path = "stylesheets_mask.rs"]
mod stylesheets_mask;
#[path = "stylesheets_query.rs"]
mod stylesheets_query;
#[path = "stylesheets_vars.rs"]
mod stylesheets_vars;

pub(crate) use self::stylesheets_mask::PropertyMask;
use self::{stylesheets_helpers::*, stylesheets_vars::apply_deferred_declaration};

macro_rules! define_inherited_default {
  // Inherited property: take the parent's computed value.
  ($parent:expr, $default:expr, $inherit:tt) => {
    $parent.to_owned()
  };
  // Non-inherited property: reset to the field's initial value.
  ($parent:expr, $default:expr) => {
    $default
  };
}

type ParsedDeclarations = SmallVec<[StyleDeclaration; 8]>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DeferredDeclaration {
  pub(crate) property: PropertyId,
  pub(crate) specified_value: String,
}

/// `--tw-*` holds per-element composition state (gradient stops), registered
/// `inherits: false` by Tailwind's own `@property` rules.
fn inherited_custom_properties(
  parent: &Arc<HashMap<String, String>>,
) -> Arc<HashMap<String, String>> {
  if !parent.keys().any(|name| name.starts_with("--tw-")) {
    return parent.clone();
  }

  Arc::new(
    parent
      .iter()
      .filter(|(name, _)| !name.starts_with("--tw-"))
      .map(|(name, value)| (name.clone(), value.clone()))
      .collect(),
  )
}

/// A utility value read from a custom property, with the built-in scale as its
/// fallback. Tailwind compiles `bg-red-500` to `var(--color-red-500)`.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct TwVarRef {
  pub(crate) name: Arc<str>,
  pub(crate) deferred: DeferredDeclaration,
  pub(crate) fallback: Option<Box<StyleDeclaration>>,
}

fn deferred_to_css<W: fmt::Write>(deferred: &DeferredDeclaration, dest: &mut W) -> fmt::Result {
  let name = match deferred.property {
    PropertyId::Longhand(id) => id.css_name(),
    PropertyId::Shorthand(id) => id.css_name(),
    _ => return Ok(()),
  };

  write!(dest, "{}: {};", name, deferred.specified_value)
}

/// `webkit_text_fill_color` → `-webkit-text-fill-color`.
fn snake_to_css_name(name: &&str) -> Box<str> {
  let mut kebab = name.replace("r#", "").replace('_', "-");

  if kebab.starts_with("webkit-") {
    kebab.insert(0, '-');
  }

  kebab.into()
}

impl TwVarRef {
  fn apply(&self, style: &mut ComputedStyle, parent: Option<&ComputedStyle>) {
    let defined = style.custom_properties.contains_key(self.name.as_ref());

    if defined && apply_deferred_declaration(style, parent, &self.deferred) {
      return;
    }

    let Some(fallback) = &self.fallback else {
      return;
    };

    match parent {
      Some(parent) => fallback.clone().apply_with_parent(style, parent),
      None => fallback.apply_to_computed(style),
    }
  }
}

/// A resolved BCP-47 language tag (the canonicalized `language[-Script][-REGION]`
/// prefix), inherited from the `lang` attribute. Drives locale-aware shaping
/// (Han unification, line-breaking).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Lang(parley::Language);

impl<'de> Deserialize<'de> for Lang {
  fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
  where
    D: serde::Deserializer<'de>,
  {
    let tag = Cow::<str>::deserialize(deserializer)?;
    Lang::parse(&tag).map_err(|_| {
      D::Error::custom(format!(
        "expected a valid BCP-47 language tag, but got {:?}",
        tag
      ))
    })
  }
}

impl Lang {
  /// Parses a BCP-47 tag string, canonicalizing language/script/region casing.
  pub fn parse(tag: &str) -> crate::Result<Self> {
    Language::parse(tag)
      .map(Self)
      .map_err(|_| Error::InvalidLanguageTag(tag.to_string()))
  }

  /// The canonical string form (`language[-Script][-REGION]`).
  pub fn as_str(&self) -> &str {
    self.0.as_str()
  }

  pub(crate) fn into_parlance(self) -> Language {
    self.0
  }
}

#[derive(Clone, Copy)]
struct InterpolationContext<'a> {
  progress: f32,
  sizing: &'a SizingContext,
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
  // Field default for `ComputedStyle`: explicit `= expr` when given, else the type's `Default`.
  (@default $default:expr) => { $default };
  (@default) => { ::core::default::Default::default() };
  (
    longhands {
      $(
        $longhand:ident: $longhand_ty:ty
          $(where inherit = $longhand_inherit:literal)?
          $(= $longhand_default:expr)?,
      )*
    }
    // `name: type => (ltr_field, rtl_field)` — apply resolves to one of them.
    transient_longhands {
      $(
        $transient:ident: $transient_ty:ty
          $(= $transient_default:expr)?
          => ($transient_ltr:ident, $transient_rtl:ident),
      )*
    }
    shorthands {
      $(
        $shorthand:ident: $shorthand_ty:ty
          => [$($target:ident),+ $(,)?]
          |$value:ident, $target_var:ident|
          $expand:block,
      )*
    }
  ) => {
    paste! {
      /// Identifies a single longhand property.
      #[repr(u8)]
      #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
      #[non_exhaustive]
      pub(crate) enum LonghandId {
        $(
          #[doc = concat!("The `", stringify!($longhand), "` longhand.")]
          [<$longhand:camel>],
        )*
        $(
          #[doc = concat!("The `", stringify!($transient), "` logical-axis longhand.")]
          [<$transient:camel>],
        )*
      }

      impl LonghandId {
        const COUNT: usize = [$(Self::[<$longhand:camel>]),* $(, Self::[<$transient:camel>])*].len();
        const ALL: [Self; Self::COUNT] = [
          $(Self::[<$longhand:camel>],)*
          $(Self::[<$transient:camel>],)*
        ];

        const fn index(self) -> usize {
          self as usize
        }

        const SNAKE_NAMES: [&'static str; Self::COUNT] = [
          $(stringify!($longhand),)*
          $(stringify!($transient),)*
        ];

        /// The property's CSS name, e.g. `-webkit-text-fill-color`.
        pub(crate) fn css_name(self) -> &'static str {
          static NAMES: OnceLock<Box<[Box<str>]>> = OnceLock::new();
          let names =
            NAMES.get_or_init(|| LonghandId::SNAKE_NAMES.iter().map(snake_to_css_name).collect());

          &names[self.index()]
        }
      }

      /// Identifies a shorthand property that expands into longhands.
      #[repr(u8)]
      #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
      #[non_exhaustive]
      pub(crate) enum ShorthandId {
        $(
          #[doc = concat!("The `", stringify!($shorthand), "` shorthand.")]
          [<$shorthand:camel>],
        )*
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
            $(
              Self::[<$transient:camel>] => Ok(smallvec![StyleDeclaration::[<$transient:camel>](
                <$transient_ty as FromCss>::from_css(input)?,
              )]),
            )*
          }
        }

        const EXPECT_INFO: [(CssExpectedMessage, &'static [CssToken]); Self::COUNT] = [
          $((<$longhand_ty as FromCss>::EXPECT_MESSAGE, <$longhand_ty as FromCss>::VALID_TOKENS),)*
          $((<$transient_ty as FromCss>::EXPECT_MESSAGE, <$transient_ty as FromCss>::VALID_TOKENS),)*
        ];

        fn expect_info(self) -> (CssExpectedMessage, &'static [CssToken]) {
          Self::EXPECT_INFO[self.index()]
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

        const EXPECT_INFO: [(CssExpectedMessage, &'static [CssToken]);
          [$(Self::[<$shorthand:camel>]),*].len()] = [
          $((<$shorthand_ty as FromCss>::EXPECT_MESSAGE, <$shorthand_ty as FromCss>::VALID_TOKENS),)*
        ];

        fn expect_info(self) -> (CssExpectedMessage, &'static [CssToken]) {
          Self::EXPECT_INFO[self as usize]
        }

        const SNAKE_NAMES: [&'static str; Self::EXPECT_INFO.len()] =
          [$(stringify!($shorthand),)*];

        /// The property's CSS name, e.g. `border-radius`.
        pub(crate) fn css_name(self) -> &'static str {
          static NAMES: OnceLock<Box<[Box<str>]>> = OnceLock::new();
          let names =
            NAMES.get_or_init(|| ShorthandId::SNAKE_NAMES.iter().map(snake_to_css_name).collect());

          &names[self as usize]
        }
      }

      /// Identifies any property: longhand, shorthand, custom, or ignored.
      #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
      #[non_exhaustive]
      pub(crate) enum PropertyId {
        /// An unrecognized property that is dropped.
        Ignored,
        /// A custom property (`--name`).
        Custom,
        /// A longhand property.
        Longhand(LonghandId),
        /// A shorthand property.
        Shorthand(ShorthandId),
      }

      impl PropertyId {
        fn from_normalized_name(name: &str) -> Self {
          match name {
            $(stringify!($longhand) => Self::Longhand(LonghandId::[<$longhand:camel>]),)*
            $(stringify!($transient) => Self::Longhand(LonghandId::[<$transient:camel>]),)*
            $(stringify!($shorthand) => Self::Shorthand(ShorthandId::[<$shorthand:camel>]),)*
            _ => Self::Ignored,
          }
        }

        fn from_kebab_case(name: &str) -> Self {
          PropertyId::from_name(name, normalize_kebab_property_name)
        }

        /// Resolves a property from a camelCase name.
        pub(crate) fn from_camel_case(name: &str) -> Self {
          PropertyId::from_name(name, normalize_camel_property_name)
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
            Self::Shorthand(property) => {
              let state = input.state();

              if let Ok(keyword) = input.try_parse(CssWideKeyword::from_css) {
                return Ok(
                  self
                    .target_longhands()
                    .iter()
                    .map(|longhand| StyleDeclaration::CssWideKeyword(longhand, keyword))
                    .collect(),
                );
              }

              input.reset(&state);
              property.parse_declarations(input)
            }
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

          if matches!(self, Self::Ignored | Self::Custom) {
            return Ok(ParsedDeclarations::new());
          }

          if let Some(keyword) = parse_css_wide_keyword(&css_input) {
            return Ok(
              self
                .target_longhands()
                .iter()
                .map(|longhand| StyleDeclaration::CssWideKeyword(longhand, keyword))
                .collect(),
            );
          }

          if let CssInput::Unexpected(unexpected) = css_input {
            return Err(CssInputParseError::UnexpectedType {
              unexpected,
              expected: self.expected_message("input").into(),
            });
          }

          let source: Cow<'_, str> = match &css_input {
            CssInput::Str(value) => Cow::Borrowed(value.as_ref()),
            CssInput::Number(number) => Cow::Owned(number.to_string()),
            CssInput::Unexpected(_) => unreachable!(),
          };

          let result = {
            let mut parser_input = ParserInput::new(&source);
            let mut parser = Parser::new(&mut parser_input);

            match self {
              Self::Shorthand(property) => property.parse_declarations(&mut parser),
              Self::Longhand(property) => property.parse_declarations(&mut parser),
              Self::Ignored | Self::Custom => unreachable!(),
            }
          }
          .map_err(|error| {
            (
              self.expected_message(&source),
              css_input_parse_failure(&source, error),
            )
          });

          drop(source);

          match result {
            Ok(declarations) => Ok(declarations),
            Err((expected, failure)) => Err(css_input_parse_error(css_input, expected, failure)),
          }
        }

        /// Parse-error "expected ..." text for this property's value type.
        fn expected_message(self, token: &str) -> String {
          let (message, valid_tokens) = match self {
            Self::Longhand(property) => property.expect_info(),
            Self::Shorthand(property) => property.expect_info(),
            Self::Ignored | Self::Custom => return String::new(),
          };

          message.build_message(token, merge_enum_values(valid_tokens))
        }

        /// Longhands this property expands into (shorthand-expansion targets; unrelated to `!important`).
        fn target_longhands(self) -> PropertyMask {
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
        // Detect var() up-front; otherwise a partial parse (e.g. `0 var(--y)`)
        // would commit before deferral. See #712.
        if !matches!(property, PropertyId::Ignored | PropertyId::Custom) {
          let state = input.state();
          skip_to_important(input);
          let specified_value = input.slice_from(start).trim();
          if contains_var_function(specified_value) {
            return Ok(StyleDeclarationBlock::from_parsed_declarations(
              smallvec![StyleDeclaration::Deferred(DeferredDeclaration {
                property,
                specified_value: specified_value.to_owned(),
              })],
              false,
            ));
          }
          input.reset(&state);
        }

        property.parse_declarations(name, input).map(|declarations| {
          StyleDeclarationBlock::from_parsed_declarations(declarations, false)
        })
      }

      /// Defines the style of an element.
      #[derive(Debug, Default, Clone, PartialEq)]
      pub struct Style {
        /// The declaration block for this style.
        pub declarations: StyleDeclarationBlock,
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

                // `undefined` / `null` values are how JS callers express "no declaration".
                if matches!(css_input, CssInput::Unexpected(CssUnexpected::Unit)) {
                  continue;
                }

                let (css_input, important) = split_important(css_input);

                if matches!(property, PropertyId::Custom) {
                  if !matches!(css_input, CssInput::Unexpected(_)) {
                    style.declarations.push(
                      StyleDeclaration::CustomProperty(key.into_owned(), css_input.into_string()),
                      important,
                    );
                  }
                } else {
                  style
                    .declarations
                    .append_parsed_declarations(
                      property
                        .parse_css_input_declarations(css_input)
                        .map_err(|error| error.into_serde_error(&key, property))?,
                      important,
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

        /// Appends another declaration block in source order.
        pub(crate) fn append_block(&mut self, declarations: StyleDeclarationBlock) {
          self.declarations.append(declarations);
        }

        /// Appends one declaration, recording its importance.
        pub fn push(&mut self, declaration: StyleDeclaration, important: bool) {
          self.declarations.push(declaration, important);
        }

        /// Collects resource URLs referenced by this style's declarations.
        pub fn image_urls(&self) -> impl Iterator<Item = &str> {
          self.declarations.image_urls()
        }

        pub(crate) fn inherit_with_lang(self, parent: &ComputedStyle, lang: Option<Lang>) -> ComputedStyle {
          let mut style = self.inherit(parent);
          if let Some(lang) = lang {
            style.lang = Some(lang);
          }
          style
        }

        /// Resolves this style against a parent into a computed style.
        pub(crate) fn inherit(self, parent: &ComputedStyle) -> ComputedStyle {
          let mut style = ComputedStyle::from_parent(parent);
          let mut declarations = ParsedDeclarations::new();

          for declaration in self.declarations.declarations {
            match declaration {
              StyleDeclaration::CustomProperty(name, value) => {
                Arc::make_mut(&mut style.custom_properties).insert(name, value);
              }
              declaration => declarations.push(declaration),
            }
          }

          // Pre-resolve `direction` so logical-axis applies below see the
          // final value even if `direction:` is declared later in the block.
          for declaration in &declarations {
            match declaration {
              StyleDeclaration::Direction(d) => style.direction = *d,
              StyleDeclaration::CssWideKeyword(LonghandId::Direction, keyword) => {
                style.direction = match keyword {
                  CssWideKeyword::Initial => Direction::default(),
                  CssWideKeyword::Inherit | CssWideKeyword::Unset => parent.direction,
                };
              }
              StyleDeclaration::Deferred(deferred)
                if matches!(deferred.property, PropertyId::Longhand(LonghandId::Direction)) =>
              {
                apply_deferred_declaration(&mut style, Some(parent), deferred);
              }
              _ => {}
            }
          }

          let parent_font_weight = parent.font_weight.value();
          for mut declaration in declarations {
            if let StyleDeclaration::FontWeight(weight) = &mut declaration {
              *weight = weight.resolve_against(parent_font_weight);
            }
            declaration.apply_with_parent(&mut style, parent);
          }
          style
        }

        /// Merges another style's declarations into this one.
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
      #[derive(Clone, Debug)]
      pub struct ComputedStyle {
        /// Resolved custom property values by name.
        pub custom_properties: Arc<HashMap<String, String>>,
        /// Registered `@property` rules by name.
        pub registered_custom_properties: Arc<HashMap<String, PropertyRule>>,
        /// Resolved BCP-47 language, inherited from the `lang` attribute. Drives
        /// locale-aware shaping (Han unification, line-breaking). Has no CSS property.
        pub lang: Option<Lang>,
        $(
          #[doc = concat!("Computed `", stringify!($longhand), "` value.")]
          pub $longhand: $longhand_ty,
        )*
      }

      impl Default for ComputedStyle {
        fn default() -> Self {
          Self {
            custom_properties: Default::default(),
            registered_custom_properties: Default::default(),
            lang: None,
            $(
              $longhand: define_style!(@default $($longhand_default)?),
            )*
          }
        }
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
        $(
          /// Logical-axis value, resolved to a physical side at apply time.
          [<$transient:camel>]($transient_ty),
        )*
        /// A custom property declaration such as `--token: value`.
        CustomProperty(String, String),
        /// A property value that must be resolved after `var()` substitution.
        Deferred(DeferredDeclaration),
        /// A CSS variable with the built-in utility scale as its fallback.
        VarRef(TwVarRef),
        /// A CSS-wide keyword targeting a longhand property.
        CssWideKeyword(LonghandId, CssWideKeyword),
      }

      impl ComputedStyle {
        /// Builds a child computed style inheriting from a parent.
        pub(crate) fn from_parent(parent: &Self) -> Self {
          Self {
            custom_properties: inherited_custom_properties(&parent.custom_properties),
            registered_custom_properties: parent.registered_custom_properties.clone(),
            lang: parent.lang,
            $($longhand: define_inherited_default!(parent.$longhand, define_style!(@default $($longhand_default)?) $(, $longhand_inherit)?),)*
          }
        }

        /// Resolves relative units against the sizing context.
        pub(crate) fn make_computed_values(&mut self, sizing: &SizingContext) {
          $(self.$longhand.make_computed(sizing);)*
        }

        pub(crate) fn apply_interpolated_properties(
          &mut self,
          from: &Self,
          to: &Self,
          animated_properties: &PropertyMask,
          progress: f32,
          sizing: &SizingContext,
          current_color: Color,
        ) {
          let interpolation_context = InterpolationContext {
            progress,
            sizing,
            current_color,
          };

          for property in animated_properties.iter() {
            match property {
              $(
                LonghandId::[<$longhand:camel>] => {
                  self.$longhand.interpolate(
                    &from.$longhand,
                    &to.$longhand,
                    progress,
                    sizing,
                    current_color,
                  );
                }
              )*
              $(LonghandId::[<$transient:camel>] => {})*
            }
          }

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
        $(
          /// Returns a declaration for this property.
          pub fn $transient(value: $transient_ty) -> Self {
            Self::[<$transient:camel>](value)
          }
        )*

        /// The longhand this declaration targets.
        pub(crate) fn longhand_id(&self) -> LonghandId {
          match self {
            $(Self::[<$longhand:camel>](..) => LonghandId::[<$longhand:camel>],)*
            $(Self::[<$transient:camel>](..) => LonghandId::[<$transient:camel>],)*
            Self::CustomProperty(..) | Self::Deferred(..) | Self::VarRef(..) => {
              unreachable!("custom and deferred declarations do not map to a single longhand")
            }
            Self::CssWideKeyword(id, _) => *id,
          }
        }

        pub(crate) fn affected_longhands(&self) -> PropertyMask {
          match self {
            Self::CssWideKeyword(id, _) => [*id].into_iter().collect(),
            Self::CustomProperty(..) => PropertyMask::default(),
            Self::Deferred(deferred) => deferred.property.target_longhands(),
            Self::VarRef(var_ref) => var_ref.deferred.property.target_longhands(),
            _ => [self.longhand_id()].into_iter().collect(),
          }
        }

        /// Applies this declaration to a computed style, resolving against the parent.
        pub(crate) fn apply_with_parent(
          self,
          style: &mut ComputedStyle,
          parent: &ComputedStyle,
        ) {
          let is_rtl = style.direction == Direction::Rtl;
          match self {
            Self::CssWideKeyword(property, keyword) => {
              match property {
                $(
                  LonghandId::[<$longhand:camel>] => {
                    style.$longhand = match keyword {
                      CssWideKeyword::Initial => define_style!(@default $($longhand_default)?),
                      CssWideKeyword::Inherit => parent.$longhand.to_owned(),
                      CssWideKeyword::Unset => define_inherited_default!(parent.$longhand, define_style!(@default $($longhand_default)?) $(, $longhand_inherit)?),
                    };
                  }
                )*
                $(
                  LonghandId::[<$transient:camel>] => {
                    let target = if is_rtl { &mut style.$transient_rtl } else { &mut style.$transient_ltr };
                    *target = match keyword {
                      CssWideKeyword::Initial | CssWideKeyword::Unset => define_style!(@default $($transient_default)?),
                      CssWideKeyword::Inherit => {
                        if parent.direction == Direction::Rtl {
                          parent.$transient_rtl.to_owned()
                        } else {
                          parent.$transient_ltr.to_owned()
                        }
                      }
                    };
                  }
                )*
              }
            }
            Self::CustomProperty(name, value) => {
              Arc::make_mut(&mut style.custom_properties).insert(name, value);
            }
            Self::Deferred(deferred) => {
              apply_deferred_declaration(style, Some(parent), &deferred);
            }
            Self::VarRef(var_ref) => var_ref.apply(style, Some(parent)),
            $(Self::[<$longhand:camel>](value) => style.$longhand = value,)*
            $(
              Self::[<$transient:camel>](value) => {
                if is_rtl { style.$transient_rtl = value } else { style.$transient_ltr = value }
              }
            )*
          }
        }

        /// Applies this declaration to a computed style without a parent.
        pub(crate) fn apply_to_computed(&self, style: &mut ComputedStyle) {
          let is_rtl = style.direction == Direction::Rtl;
          match self {
            Self::CssWideKeyword(property, keyword) => match keyword {
              CssWideKeyword::Initial => match property {
                $(
                  LonghandId::[<$longhand:camel>] => {
                    style.$longhand = define_style!(@default $($longhand_default)?);
                  }
                )*
                $(
                  LonghandId::[<$transient:camel>] => {
                    if is_rtl { style.$transient_rtl = define_style!(@default $($transient_default)?) }
                    else { style.$transient_ltr = define_style!(@default $($transient_default)?) }
                  }
                )*
              },
              CssWideKeyword::Inherit | CssWideKeyword::Unset => {}
            },
            Self::CustomProperty(name, value) => {
              Arc::make_mut(&mut style.custom_properties).insert(name.to_owned(), value.to_owned());
            }
            Self::Deferred(deferred) => {
              apply_deferred_declaration(style, None, deferred);
            }
            Self::VarRef(var_ref) => var_ref.apply(style, None),
            $(Self::[<$longhand:camel>](value) => style.$longhand.clone_from(value),)*
            $(
              Self::[<$transient:camel>](value) => {
                if is_rtl { style.$transient_rtl.clone_from(value) }
                else { style.$transient_ltr.clone_from(value) }
              }
            )*
          }
        }

        /// Pushes a clone of this declaration onto a style.
        pub(crate) fn merge_into_ref(&self, style: &mut Style) {
          style.declarations.push(self.to_owned(), false);
        }

      }

      impl crate::style::properties::ToCss for StyleDeclaration {
        fn to_css<W: fmt::Write>(&self, dest: &mut W) -> fmt::Result {
          match self {
            $(
              Self::[<$longhand:camel>](value) => {
                dest.write_str(LonghandId::[<$longhand:camel>].css_name())?;
                dest.write_str(": ")?;
                value.to_css(dest)?;
                dest.write_str(";")
              }
            )*
            $(
              Self::[<$transient:camel>](value) => {
                dest.write_str(LonghandId::[<$transient:camel>].css_name())?;
                dest.write_str(": ")?;
                value.to_css(dest)?;
                dest.write_str(";")
              }
            )*
            Self::CustomProperty(name, value) => {
              write!(dest, "{}: {};", name, value)
            }
            Self::VarRef(var_ref) => deferred_to_css(&var_ref.deferred, dest),
            Self::Deferred(deferred) => deferred_to_css(deferred, dest),
            Self::CssWideKeyword(id, keyword) => {
              let keyword_str = match keyword {
                CssWideKeyword::Initial => "initial",
                CssWideKeyword::Inherit => "inherit",
                CssWideKeyword::Unset => "unset",
              };
              write!(dest, "{}: {};", id.css_name(), keyword_str)
            }
          }
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
    max_width: MaxSize,
    max_height: MaxSize,
    min_width: Length,
    min_height: Length,
    aspect_ratio: AspectRatio,
    padding_top: Length = Length::zero(),
    padding_right: Length = Length::zero(),
    padding_bottom: Length = Length::zero(),
    padding_left: Length = Length::zero(),
    margin_top: Length = Length::zero(),
    margin_right: Length = Length::zero(),
    margin_bottom: Length = Length::zero(),
    margin_left: Length = Length::zero(),
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
    transform_origin: PositionValue = PositionValue::center(),
    offset_path: Option<OffsetPath>,
    offset_distance: Length,
    offset_rotate: OffsetRotate,
    offset_anchor: OffsetAnchor,
    offset_position: OffsetPosition,
    mask_image: Option<BackgroundImages>,
    mask_size: BackgroundSizes,
    mask_position: PositionValues,
    mask_repeat: BackgroundRepeats,
    column_gap: Gap,
    row_gap: Gap,
    flex_grow: Option<FlexGrow>,
    flex_shrink: Option<FlexGrow>,
    border_top_left_radius: SpacePair<Length> = SpacePair::from_single(Length::zero()),
    border_top_right_radius: SpacePair<Length> = SpacePair::from_single(Length::zero()),
    border_bottom_right_radius: SpacePair<Length> = SpacePair::from_single(Length::zero()),
    border_bottom_left_radius: SpacePair<Length> = SpacePair::from_single(Length::zero()),
    corner_top_left_shape: Superellipse,
    corner_top_right_shape: Superellipse,
    corner_bottom_right_shape: Superellipse,
    corner_bottom_left_shape: Superellipse,
    border_top_width: LineWidth,
    border_right_width: LineWidth,
    border_bottom_width: LineWidth,
    border_left_width: LineWidth,
    border_top_style: BorderStyle,
    border_right_style: BorderStyle,
    border_bottom_style: BorderStyle,
    border_left_style: BorderStyle,
    border_top_color: ColorInput,
    border_right_color: ColorInput,
    border_bottom_color: ColorInput,
    border_left_color: ColorInput,
    outline_width: LineWidth,
    outline_style: BorderStyle,
    outline_color: ColorInput,
    outline_offset: Length,
    object_fit: ObjectFit,
    overflow_x: Overflow,
    overflow_y: Overflow,
    object_position: PositionValue = PositionValue::center(),
    background_image: Option<BackgroundImages>,
    background_position: PositionValues,
    background_size: BackgroundSizes,
    background_repeat: BackgroundRepeats,
    background_blend_mode: BlendModes,
    background_color: ColorInput = ColorInput::transparent(),
    background_clip: BackgroundClip,
    background_origin: BackgroundOrigin,
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
    text_fit: TextFit where inherit = true,
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
    font_variant_ligatures: FontVariantLigatures where inherit = true,
    font_variant_numeric: FontVariantNumeric where inherit = true,
    font_variant_east_asian: FontVariantEastAsian where inherit = true,
    font_variant_caps: FontVariantCaps where inherit = true,
    font_variant_position: FontVariantPosition where inherit = true,
    font_kerning: FontKerning where inherit = true,
    font_synthesis_weight: FontSynthesic where inherit = true,
    font_synthesis_style: FontSynthesic where inherit = true,
    max_lines: Option<u32>,
    block_ellipsis: BlockEllipsis where inherit = true,
    r#continue: Continue,
    text_align: TextAlign where inherit = true,
    webkit_text_stroke_width: Option<Length> where inherit = true,
    webkit_text_stroke_color: Option<ColorInput> where inherit = true,
    webkit_text_fill_color: Option<ColorInput> where inherit = true,
    stroke_linejoin: LineJoin where inherit = true,
    text_shadow: Option<TextShadows> where inherit = true,
    text_decoration_line: Option<TextDecorationLines>,
    text_decoration_style: TextDecorationStyle,
    break_before: BreakBetween,
    break_after: BreakBetween,
    break_inside: BreakInside,
    box_decoration_break: BoxDecorationBreak,
    widows: MinLines where inherit = true,
    orphans: MinLines where inherit = true,
    text_decoration_color: ColorInput,
    text_decoration_thickness: TextDecorationThickness,
    text_underline_offset: TextUnderlineOffset where inherit = true,
    text_underline_position: TextUnderlinePosition where inherit = true,
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
    tab_size: TabSize where inherit = true,
    text_wrap_mode: TextWrapMode where inherit = true,
    text_wrap_style: TextWrapStyle where inherit = true,
    direction: Direction where inherit = true,
    float: Float,
    clear: Clear,
    isolation: Isolation,
    mix_blend_mode: BlendMode,
    visibility: Visibility where inherit = true,
    caption_side: CaptionSide where inherit = true,
    border_collapse: BorderCollapse where inherit = true,
    table_layout: TableLayout,
    border_spacing: BorderSpacing where inherit = true,
    vertical_align: VerticalAlign,
    content: ContentValue,
    list_style_type: ListStyleType where inherit = true,
    list_style_position: ListStylePosition where inherit = true,
    list_style_image: ListStyleImage where inherit = true,
  }
  transient_longhands {
    margin_inline_start: Length = Length::zero() => (margin_left, margin_right),
    margin_inline_end: Length = Length::zero() => (margin_right, margin_left),
    padding_inline_start: Length = Length::zero() => (padding_left, padding_right),
    padding_inline_end: Length = Length::zero() => (padding_right, padding_left),
  }
  shorthands {
    list_style: ListStyleShorthand => [ListStyleType, ListStylePosition, ListStyleImage] |value, target| {
      target.push(StyleDeclaration::list_style_type(value.style_type));
      target.push(StyleDeclaration::list_style_position(value.position));
      target.push(StyleDeclaration::list_style_image(value.image));
    },
    offset: OffsetShorthand => [OffsetPosition, OffsetPath, OffsetDistance, OffsetRotate, OffsetAnchor] |value, target| {
      target.push(StyleDeclaration::offset_position(value.position));
      target.push(StyleDeclaration::offset_path(value.path));
      target.push(StyleDeclaration::offset_distance(value.distance));
      target.push(StyleDeclaration::offset_rotate(value.rotate));
      target.push(StyleDeclaration::offset_anchor(value.anchor));
    },
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
    padding: Sides<Length> => [PaddingTop, PaddingRight, PaddingBottom, PaddingLeft] |value, target| {
      push_four_side_declarations!(
        target,
        value.0,
        padding_top,
        padding_right,
        padding_bottom,
        padding_left
      );
    },
    padding_inline: SpacePair<Length> => [PaddingInlineStart, PaddingInlineEnd] |value, target| {
      push_axis_declarations!(target, value, padding_inline_start, padding_inline_end);
    },
    padding_block: SpacePair<Length> => [PaddingTop, PaddingBottom] |value, target| {
      push_axis_declarations!(target, value, padding_top, padding_bottom);
    },
    margin: Sides<Length> => [MarginTop, MarginRight, MarginBottom, MarginLeft] |value, target| {
      push_four_side_declarations!(
        target,
        value.0,
        margin_top,
        margin_right,
        margin_bottom,
        margin_left
      );
    },
    margin_inline: SpacePair<Length> => [MarginInlineStart, MarginInlineEnd] |value, target| {
      push_axis_declarations!(target, value, margin_inline_start, margin_inline_end);
    },
    margin_block: SpacePair<Length> => [MarginTop, MarginBottom] |value, target| {
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
    gap: SpacePair<Gap> => [RowGap, ColumnGap] |value, target| {
      push_axis_declarations!(target, value, row_gap, column_gap);
    },
    flex_flow: FlexFlow => [FlexDirection, FlexWrap] |value, target| {
      target.push(StyleDeclaration::flex_direction(value.direction));
      target.push(StyleDeclaration::flex_wrap(value.wrap));
    },
    box_orient: BoxOrient => [FlexDirection] |value, target| {
      target.push(StyleDeclaration::flex_direction(value.into()));
    },
    box_pack: BoxPack => [JustifyContent] |value, target| {
      target.push(StyleDeclaration::justify_content(value.into()));
    },
    box_align: BoxAlign => [AlignItems] |value, target| {
      target.push(StyleDeclaration::align_items(value.into()));
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
    corner_shape: Sides<Superellipse> => [CornerTopLeftShape, CornerTopRightShape, CornerBottomRightShape, CornerBottomLeftShape] |value, target| {
      push_four_side_declarations!(
        target,
        value.0,
        corner_top_left_shape,
        corner_top_right_shape,
        corner_bottom_right_shape,
        corner_bottom_left_shape
      );
    },
    border_width: Sides<LineWidth> => [BorderTopWidth, BorderRightWidth, BorderBottomWidth, BorderLeftWidth] |value, target| {
      push_four_side_declarations!(
        target,
        value.0,
        border_top_width,
        border_right_width,
        border_bottom_width,
        border_left_width
      );
    },
    border_inline_width: SpacePair<LineWidth> => [BorderLeftWidth, BorderRightWidth] |value, target| {
      push_axis_declarations!(
        target,
        value,
        border_left_width,
        border_right_width
      );
    },
    border_block_width: SpacePair<LineWidth> => [BorderTopWidth, BorderBottomWidth] |value, target| {
      push_axis_declarations!(
        target,
        value,
        border_top_width,
        border_bottom_width
      );
    },
    border: Border => [BorderTopWidth, BorderRightWidth, BorderBottomWidth, BorderLeftWidth, BorderTopStyle, BorderRightStyle, BorderBottomStyle, BorderLeftStyle, BorderTopColor, BorderRightColor, BorderBottomColor, BorderLeftColor] |value, target| {
      target.push(StyleDeclaration::border_top_width(value.width));
      target.push(StyleDeclaration::border_right_width(value.width));
      target.push(StyleDeclaration::border_bottom_width(value.width));
      target.push(StyleDeclaration::border_left_width(value.width));
      target.push(StyleDeclaration::border_top_style(value.style));
      target.push(StyleDeclaration::border_right_style(value.style));
      target.push(StyleDeclaration::border_bottom_style(value.style));
      target.push(StyleDeclaration::border_left_style(value.style));
      target.push(StyleDeclaration::border_top_color(value.color));
      target.push(StyleDeclaration::border_right_color(value.color));
      target.push(StyleDeclaration::border_bottom_color(value.color));
      target.push(StyleDeclaration::border_left_color(value.color));
    },
    border_top: Border => [BorderTopWidth, BorderTopStyle, BorderTopColor] |value, target| {
      target.push(StyleDeclaration::border_top_width(value.width));
      target.push(StyleDeclaration::border_top_style(value.style));
      target.push(StyleDeclaration::border_top_color(value.color));
    },
    border_right: Border => [BorderRightWidth, BorderRightStyle, BorderRightColor] |value, target| {
      target.push(StyleDeclaration::border_right_width(value.width));
      target.push(StyleDeclaration::border_right_style(value.style));
      target.push(StyleDeclaration::border_right_color(value.color));
    },
    border_bottom: Border => [BorderBottomWidth, BorderBottomStyle, BorderBottomColor] |value, target| {
      target.push(StyleDeclaration::border_bottom_width(value.width));
      target.push(StyleDeclaration::border_bottom_style(value.style));
      target.push(StyleDeclaration::border_bottom_color(value.color));
    },
    border_left: Border => [BorderLeftWidth, BorderLeftStyle, BorderLeftColor] |value, target| {
      target.push(StyleDeclaration::border_left_width(value.width));
      target.push(StyleDeclaration::border_left_style(value.style));
      target.push(StyleDeclaration::border_left_color(value.color));
    },
    border_style: Sides<BorderStyle> => [BorderTopStyle, BorderRightStyle, BorderBottomStyle, BorderLeftStyle] |value, target| {
      push_four_side_declarations!(
        target,
        value.0,
        border_top_style,
        border_right_style,
        border_bottom_style,
        border_left_style
      );
    },
    border_color: Sides<ColorInput> => [BorderTopColor, BorderRightColor, BorderBottomColor, BorderLeftColor] |value, target| {
      push_four_side_declarations!(
        target,
        value.0,
        border_top_color,
        border_right_color,
        border_bottom_color,
        border_left_color
      );
    },
    outline: Border => [OutlineWidth, OutlineStyle, OutlineColor] |value, target| {
      target.push(StyleDeclaration::outline_width(value.width));
      target.push(StyleDeclaration::outline_style(value.style));
      target.push(StyleDeclaration::outline_color(value.color));
    },
    overflow: SpacePair<Overflow> => [OverflowX, OverflowY] |value, target| {
      push_axis_declarations!(target, value, overflow_x, overflow_y);
    },
    background: Backgrounds => [BackgroundImage, BackgroundPosition, BackgroundSize, BackgroundRepeat, BackgroundColor, BackgroundClip, BackgroundOrigin] |value, target| {
      target.push(StyleDeclaration::background_position(
        value.iter().map(|background| background.position).collect(),
      ));
      target.push(StyleDeclaration::background_size(
        value.iter().map(|background| background.size).collect(),
      ));
      target.push(StyleDeclaration::background_repeat(
        value.iter().map(|background| background.repeat).collect(),
      ));
      target.push(StyleDeclaration::background_color(
        value
          .iter()
          .filter_map(|background| background.color)
          .next_back()
          .unwrap_or(ColorInput::transparent()),
      ));
      target.push(StyleDeclaration::background_clip(
        value
          .last()
          .map(|background| background.clip)
          .unwrap_or_default(),
      ));
      target.push(StyleDeclaration::background_origin(
        value
          .last()
          .map(|background| background.origin)
          .unwrap_or_default(),
      ));
      target.push(StyleDeclaration::background_image(Some(
        value
          .into_iter()
          .map(|background| background.image)
          .collect(),
      )));
    },
    font_synthesis: FontSynthesis => [FontSynthesisWeight, FontSynthesisStyle] |value, target| {
      target.push(StyleDeclaration::font_synthesis_weight(value.weight));
      target.push(StyleDeclaration::font_synthesis_style(value.style));
    },
    font_variant: FontVariant => [FontVariantLigatures, FontVariantNumeric, FontVariantEastAsian, FontVariantCaps, FontVariantPosition] |value, target| {
      target.push(StyleDeclaration::font_variant_ligatures(value.ligatures));
      target.push(StyleDeclaration::font_variant_numeric(value.numeric));
      target.push(StyleDeclaration::font_variant_east_asian(value.east_asian));
      target.push(StyleDeclaration::font_variant_caps(value.caps));
      target.push(StyleDeclaration::font_variant_position(value.position));
    },
    webkit_text_stroke: Option<TextStroke> => [WebkitTextStrokeWidth, WebkitTextStrokeColor] |value, target| {
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
    white_space: WhiteSpace => [TextWrapMode, WhiteSpaceCollapse] |value, target| {
      target.push(StyleDeclaration::text_wrap_mode(value.text_wrap_mode));
      target.push(StyleDeclaration::white_space_collapse(
        value.white_space_collapse,
      ));
    },
    text_wrap: TextWrap => [TextWrapMode, TextWrapStyle] |value, target| {
      target.push(StyleDeclaration::text_wrap_mode(value.mode));
      target.push(StyleDeclaration::text_wrap_style(value.style));
    },
    line_clamp: LineClamp => [MaxLines, BlockEllipsis, Continue] |value, target| {
      target.push(StyleDeclaration::max_lines(value.max_lines));
      target.push(StyleDeclaration::block_ellipsis(value.block_ellipsis));
      target.push(StyleDeclaration::r#continue(value.line_continue));
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

/// The set of properties marked `!important`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DeclarationImportance {
  pub(crate) longhands: PropertyMask,
  /// Custom property names marked important.
  pub(crate) custom_properties: SmallVec<[Box<str>; 1]>,
}

impl DeclarationImportance {
  /// Whether no property is marked important.
  pub fn is_empty(&self) -> bool {
    self.custom_properties.is_empty() && self.longhands.iter().next().is_none()
  }

  /// Records the longhands a declaration marks important.
  pub(crate) fn insert_declaration(&mut self, declaration: &StyleDeclaration) {
    self
      .longhands
      .extend(declaration.affected_longhands().iter());

    if let StyleDeclaration::CustomProperty(name, _) = declaration {
      self.insert_custom_property(name);
    }
  }

  /// Merges another importance set, deduping custom properties.
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
  /// Positional against `declarations`, because the mask below unions the block
  /// and cannot tell `p-2 !p-4` apart once both have marked the same longhand.
  important: SmallVec<[bool; 8]>,
  /// Properties that were marked with `!important`.
  pub importance: DeclarationImportance,
}

impl StyleDeclarationBlock {
  fn from_parsed_declarations(declarations: ParsedDeclarations, important: bool) -> Self {
    let mut block = Self::default();
    block.append_parsed_declarations(declarations, important);
    block
  }

  /// Appends a declaration and records whether it was important.
  pub fn push(&mut self, declaration: StyleDeclaration, important: bool) {
    if important {
      self.importance.insert_declaration(&declaration);
    }
    self.declarations.push(declaration);
    self.important.push(important);
  }

  fn append_parsed_declarations(&mut self, declarations: ParsedDeclarations, important: bool) {
    for declaration in declarations {
      self.push(declaration, important);
    }
  }

  /// Marks the block `!important`, the way a shorthand hands the marker to
  /// every longhand it expands into.
  pub(crate) fn mark_important(&mut self) {
    for (declaration, important) in self.declarations.iter().zip(&mut self.important) {
      self.importance.insert_declaration(declaration);
      *important = true;
    }
  }

  /// Splits the block at the two ends of the cascade: a layer's important
  /// declarations beat the layers that beat its normal ones.
  pub(crate) fn split_importance(self) -> (Self, Self) {
    if self.importance.is_empty() {
      return (self, Self::default());
    }

    let mut normal = Self::default();
    let mut important = Self::default();

    for (declaration, is_important) in self.declarations.into_iter().zip(self.important) {
      let target = if is_important {
        &mut important
      } else {
        &mut normal
      };

      target.push(declaration, is_important);
    }

    (normal, important)
  }

  /// Appends another block's declarations and importance.
  pub(crate) fn append(&mut self, mut other: Self) {
    self.importance.append(&mut other.importance);
    self.declarations.extend(other.declarations);
    self.important.extend(other.important);
  }

  /// Iterates over the declarations in source order.
  pub fn iter(&self) -> std::slice::Iter<'_, StyleDeclaration> {
    self.declarations.iter()
  }

  /// The number of declarations in this block.
  pub fn len(&self) -> usize {
    self.declarations.len()
  }

  /// Whether this block has no declarations.
  pub fn is_empty(&self) -> bool {
    self.declarations.is_empty()
  }

  /// Collects resource URLs referenced by declarations in this block.
  pub fn image_urls(&self) -> impl Iterator<Item = &str> {
    fn background_image_url(image: &BackgroundImage) -> Option<&str> {
      if let BackgroundImage::Url(url) = image {
        Some(url.as_ref())
      } else {
        None
      }
    }

    self
      .iter()
      .flat_map(|declaration| -> Box<dyn Iterator<Item = &str> + '_> {
        match declaration {
          StyleDeclaration::BackgroundImage(Some(images))
          | StyleDeclaration::MaskImage(Some(images)) => {
            Box::new(images.iter().filter_map(background_image_url))
          }
          StyleDeclaration::ListStyleImage(image) => {
            Box::new(image.image().and_then(background_image_url).into_iter())
          }
          StyleDeclaration::Content(ContentValue::Items(items)) => {
            Box::new(items.iter().filter_map(|item| match item {
              ContentItem::Image(image) => background_image_url(image.as_ref()),
              _ => None,
            }))
          }
          _ => Box::new(std::iter::empty()),
        }
      })
  }

  /// Parses one declaration block for the given property name.
  pub(crate) fn parse<'i>(name: &str, input: &mut Parser<'i, '_>) -> ParseResult<'i, Self> {
    parse_style_declaration(name, input)
  }

  /// Parses a declaration list, dropping the declarations that fail and keeping the rest.
  ///
  /// This is how CSS asks a `style` attribute to be read: an unsupported value invalidates
  /// its own declaration and nothing else. [`FromStr`] stays strict for callers that want to
  /// know the input was not fully understood.
  pub fn parse_loosy(input: &str) -> Self {
    let mut parser_input = ParserInput::new(input);
    let mut parser = Parser::new(&mut parser_input);
    let mut declaration_parser = StyleDeclarationParser;
    let mut block = Self::default();

    for declarations in RuleBodyParser::new(&mut parser, &mut declaration_parser).flatten() {
      block.append(declarations);
    }
    block
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

impl FromStr for Style {
  type Err = StyleDeclarationBlockParseError;

  fn from_str(input: &str) -> Result<Self, Self::Err> {
    StyleDeclarationBlock::from_str(input).map(Into::into)
  }
}

#[cfg(test)]
#[path = "stylesheets_tests.rs"]
mod tests;
