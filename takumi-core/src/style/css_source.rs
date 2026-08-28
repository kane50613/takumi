//! The two shapes a caller can hand the `css` render option: stylesheet text,
//! or a rule written as an object.

use std::fmt::Write;

use cssparser::{ParseError, Parser, ParserInput};
use selectors::parser::{ParseRelative, SelectorList};
use serde::{
  Deserialize,
  de::{MapAccess, Visitor},
};

use crate::error::StyleSheetParseError;
use crate::style::{
  CssInput, CssUnexpected, CssValueSeed, PropertyId,
  media_query::MediaQueryList,
  selector::{SelectorImpl, TakumiSelectorParser, parse_layer_name},
  supports::parse_supports_condition,
};

/// A declaration as written. Kept in source order, because a shorthand and the
/// longhand it expands into read differently either way round.
#[derive(Debug, Clone, PartialEq)]
struct Declaration {
  name: String,
  value: String,
}

/// Declarations in the order the object listed them.
#[derive(Debug, Clone, Default, PartialEq)]
struct Declarations(Vec<Declaration>);

impl<'de> Deserialize<'de> for Declarations {
  fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
  where
    D: serde::Deserializer<'de>,
  {
    struct DeclarationsVisitor;

    impl<'de> Visitor<'de> for DeclarationsVisitor {
      type Value = Declarations;

      fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("a style object")
      }

      fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
      where
        A: MapAccess<'de>,
      {
        let mut declarations = Vec::new();

        while let Some(name) = map.next_key::<String>()? {
          let value = map.next_value_seed(CssValueSeed)?;

          // `undefined` / `null` is how a JS caller writes "no declaration".
          if !matches!(value, CssInput::Unexpected(CssUnexpected::Unit)) {
            declarations.push(Declaration {
              name,
              value: value.into_string(),
            });
          }
        }

        Ok(Declarations(declarations))
      }
    }

    deserializer.deserialize_map(DeclarationsVisitor)
  }
}

/// A style rule written as an object, so values that come from application data
/// never need string concatenation.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StyleRule {
  /// The selector this rule applies to.
  pub selector: String,
  /// Declarations for the selector, in source order.
  #[serde(default)]
  style: Declarations,
  /// Rules nested inside this one, read as CSS nesting.
  #[serde(default)]
  rules: Vec<StyleRule>,
}

/// One step of an animation, written as an object.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AnimationStep {
  /// Where the step sits: `from`, `to`, or a percentage.
  pub offset: String,
  /// Declarations for the step, in source order.
  #[serde(default)]
  style: Declarations,
}

/// An animation written as an object.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AnimationRule {
  /// The name `animation-name` matches.
  pub keyframes: String,
  /// The animation's steps.
  #[serde(default)]
  steps: Vec<AnimationStep>,
}

/// A group of entries gated by a media query.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MediaRule {
  /// The query the group is gated by.
  pub media: String,
  /// The entries inside the group.
  #[serde(default)]
  rules: Vec<CssSource>,
}

/// A group of entries gated by a support condition.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SupportsRule {
  /// The condition the group is gated by.
  pub supports: String,
  /// The entries inside the group.
  #[serde(default)]
  rules: Vec<CssSource>,
}

/// A cascade layer, either declaring the layer or filling it.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LayerRule {
  /// The layer's name.
  pub layer: String,
  /// The entries inside the layer. Absent declares the layer's order alone.
  rules: Option<Vec<CssSource>>,
}

/// One entry of the `css` render option.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(untagged)]
pub enum CssSource {
  /// Stylesheet text, parsed with error recovery.
  Text(String),
  /// A rule object, validated before it reaches the parser.
  Rule(StyleRule),
  /// An animation object, validated before it reaches the parser.
  Keyframes(AnimationRule),
  /// A media group, validated before it reaches the parser.
  Media(MediaRule),
  /// A support group, validated before it reaches the parser.
  Supports(SupportsRule),
  /// A cascade layer, validated before it reaches the parser.
  Layer(LayerRule),
}

/// Why a rule object could not become CSS.
#[derive(Debug, Clone, PartialEq)]
pub enum CssSourceError {
  /// A rule's prelude is not what that rule takes.
  Prelude {
    /// What the prelude was read as, e.g. `selector` or `@media`.
    rule: &'static str,
    /// The prelude as written.
    value: String,
  },
  /// A declaration value is not a value for its property.
  Declaration {
    /// The property the value was written for.
    name: String,
    /// The value as written.
    value: String,
  },
}

impl std::fmt::Display for CssSourceError {
  fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::Prelude { rule, value } => write!(formatter, "invalid {rule} {value:?}"),
      Self::Declaration { name, value } => {
        write!(formatter, "invalid value for {name}: {value:?}")
      }
    }
  }
}

impl std::error::Error for CssSourceError {}

impl CssSource {
  /// The CSS this source stands for, with a rule object validated on the way.
  pub fn into_css(self) -> Result<String, CssSourceError> {
    let mut css = String::new();
    self.write_css(&mut css)?;
    Ok(css)
  }

  fn write_css(&self, css: &mut String) -> Result<(), CssSourceError> {
    match self {
      Self::Text(text) => {
        css.push_str(text);
        Ok(())
      }
      Self::Rule(rule) => rule.write_css(css),
      Self::Keyframes(rule) => rule.write_css(css),
      Self::Media(rule) => rule.write_css(css),
      Self::Supports(rule) => rule.write_css(css),
      Self::Layer(rule) => rule.write_css(css),
    }
  }
}

impl MediaRule {
  fn write_css(&self, css: &mut String) -> Result<(), CssSourceError> {
    validate_prelude("@media", &self.media, |parser| {
      MediaQueryList::parse(parser).map(|_| ())
    })?;
    write_group(css, "@media ", &self.media, &self.rules)
  }
}

impl SupportsRule {
  fn write_css(&self, css: &mut String) -> Result<(), CssSourceError> {
    validate_prelude("@supports", &self.supports, |parser| {
      parse_supports_condition(parser).map(|_| ())
    })?;
    write_group(css, "@supports ", &self.supports, &self.rules)
  }
}

impl LayerRule {
  fn write_css(&self, css: &mut String) -> Result<(), CssSourceError> {
    validate_prelude("@layer", &self.layer, |parser| {
      parse_layer_name(parser).map(|_| ())
    })?;

    let Some(rules) = &self.rules else {
      let _ = write!(css, "@layer {};", self.layer);
      return Ok(());
    };

    write_group(css, "@layer ", &self.layer, rules)
  }
}

fn write_group(
  css: &mut String,
  at_rule: &str,
  prelude: &str,
  rules: &[CssSource],
) -> Result<(), CssSourceError> {
  let _ = write!(css, "{at_rule}{prelude}{{");

  for entry in rules {
    entry.write_css(css)?;
  }

  css.push('}');
  Ok(())
}

/// Reads a prelude with the grammar its rule takes, leaving nothing over, so it
/// cannot close the rule and open another.
fn validate_prelude(
  rule: &'static str,
  prelude: &str,
  parse: impl for<'i, 't> FnOnce(
    &mut Parser<'i, 't>,
  ) -> Result<(), ParseError<'i, StyleSheetParseError>>,
) -> Result<(), CssSourceError> {
  let mut parser_input = ParserInput::new(prelude);
  let mut parser = Parser::new(&mut parser_input);

  match parser.parse_entirely(parse) {
    Ok(()) => Ok(()),
    Err(_) => Err(CssSourceError::Prelude {
      rule,
      value: prelude.to_owned(),
    }),
  }
}

impl AnimationRule {
  fn write_css(&self, css: &mut String) -> Result<(), CssSourceError> {
    validate_prelude("@keyframes name", &self.keyframes, |parser| {
      parser.expect_ident().map(|_| ()).map_err(Into::into)
    })?;

    let _ = write!(css, "@keyframes {}{{", self.keyframes);

    for step in &self.steps {
      validate_keyframe_offset(&step.offset)?;
      let _ = write!(css, "{}{{", step.offset);
      write_declarations(&step.style, css)?;
      css.push('}');
    }

    css.push('}');
    Ok(())
  }
}

impl StyleRule {
  fn write_css(&self, css: &mut String) -> Result<(), CssSourceError> {
    validate_selector(&self.selector)?;

    // The selector and every value are checked before they are written, so the
    // text cannot carry a declaration or a rule the object did not name.
    let _ = write!(css, "{}{{", self.selector);
    write_declarations(&self.style, css)?;

    for nested in &self.rules {
      nested.write_css(css)?;
    }

    css.push('}');
    Ok(())
  }
}

fn write_declarations(declarations: &Declarations, css: &mut String) -> Result<(), CssSourceError> {
  for declaration in &declarations.0 {
    validate_declaration(declaration)?;
    let _ = write!(
      css,
      "{}:{};",
      css_name(&declaration.name),
      declaration.value
    );
  }

  Ok(())
}

/// A step selector is a comma list of `from`, `to`, or a percentage.
fn validate_keyframe_offset(offset: &str) -> Result<(), CssSourceError> {
  let mut parser_input = ParserInput::new(offset);
  let mut parser = Parser::new(&mut parser_input);

  parser
    .parse_entirely(|parser| {
      parser.parse_comma_separated(|parser| {
        if parser.try_parse(Parser::expect_percentage).is_ok() {
          return Ok(());
        }

        let location = parser.current_source_location();
        let ident = parser.expect_ident()?;

        if ident.eq_ignore_ascii_case("from") || ident.eq_ignore_ascii_case("to") {
          Ok(())
        } else {
          Err(location.new_custom_error(()))
        }
      })
    })
    .map(|_| ())
    .map_err(|_: ParseError<'_, ()>| CssSourceError::Prelude {
      rule: "keyframe offset",
      value: offset.to_owned(),
    })
}

/// The CSS spelling of a property name written in camelCase.
fn css_name(name: &str) -> String {
  if name.starts_with("--") || name.contains('-') {
    return name.to_owned();
  }

  let mut css = String::with_capacity(name.len() + 4);
  for character in name.chars() {
    if character.is_ascii_uppercase() {
      css.push('-');
      css.push(character.to_ascii_lowercase());
    } else {
      css.push(character);
    }
  }

  css
}

fn validate_selector(selector: &str) -> Result<(), CssSourceError> {
  let mut parser_input = ParserInput::new(selector);
  let mut parser = Parser::new(&mut parser_input);

  let parsed = parser.parse_entirely(|parser| {
    SelectorList::<SelectorImpl>::parse(&TakumiSelectorParser, parser, ParseRelative::ForNesting)
  });

  match parsed {
    Ok(_) => Ok(()),
    Err(_) => Err(CssSourceError::Prelude {
      rule: "selector",
      value: selector.to_owned(),
    }),
  }
}

fn validate_declaration(declaration: &Declaration) -> Result<(), CssSourceError> {
  let property = PropertyId::from_camel_case(&declaration.name);

  if matches!(property, PropertyId::Ignored | PropertyId::Custom) {
    return Ok(());
  }

  property
    .parse_css_input_declarations(CssInput::Str(declaration.value.as_str().into()))
    .map(|_| ())
    .map_err(|_| CssSourceError::Declaration {
      name: declaration.name.clone(),
      value: declaration.value.clone(),
    })
}

#[cfg(test)]
mod tests {
  use serde_json::{Value, from_value, json};

  use super::*;

  fn css(source: Value) -> Result<String, CssSourceError> {
    from_value::<CssSource>(source)
      .expect("source should deserialize")
      .into_css()
  }

  #[test]
  fn text_passes_through() {
    assert_eq!(css(json!(".a{color:red}")), Ok(".a{color:red}".into()));
  }

  /// Declarations follow the order the deserializer visits them, which for a JS
  /// object is the order it was written. `json!` sorts its keys, so this only
  /// pins the names and values.
  #[test]
  fn a_rule_writes_its_declarations() {
    assert_eq!(
      css(json!({
        "selector": ".card",
        "style": { "margin": "1px", "marginTop": "2px", "--brand": "#5b21b6", "width": 55 },
      })),
      Ok(".card{--brand:#5b21b6;margin:1px;margin-top:2px;width:55;}".into())
    );
  }

  #[test]
  fn nesting_writes_nested_rules() {
    assert_eq!(
      css(json!({
        "selector": ".card",
        "style": { "color": "red" },
        "rules": [{ "selector": "&:hover", "style": { "color": "blue" } }],
      })),
      Ok(".card{color:red;&:hover{color:blue;}}".into())
    );
  }

  /// A value cannot close its declaration and open another, because it has to
  /// parse entirely as a value for its own property first.
  #[test]
  fn a_value_cannot_carry_a_second_declaration() {
    assert_eq!(
      css(json!({ "selector": ".card", "style": { "color": "red; width: 999px" } })),
      Err(CssSourceError::Declaration {
        name: "color".into(),
        value: "red; width: 999px".into(),
      })
    );
  }

  /// A selector cannot close its rule and open another.
  #[test]
  fn a_selector_cannot_carry_a_second_rule() {
    assert!(matches!(
      css(json!({ "selector": ".a{color:red}.b" })),
      Err(CssSourceError::Prelude {
        rule: "selector",
        ..
      })
    ));
    assert!(matches!(
      css(json!({ "selector": "}" })),
      Err(CssSourceError::Prelude {
        rule: "selector",
        ..
      })
    ));
    assert!(matches!(
      css(json!({ "selector": "@media print" })),
      Err(CssSourceError::Prelude {
        rule: "selector",
        ..
      })
    ));
  }

  #[test]
  fn keyframes_write_their_steps() {
    assert_eq!(
      css(json!({
        "keyframes": "spin",
        "steps": [
          { "offset": "from", "style": { "transform": "rotate(0deg)" } },
          { "offset": "50%, 75%", "style": { "opacity": "0.5" } },
          { "offset": "to", "style": { "transform": "rotate(360deg)" } },
        ],
      })),
      Ok(
        "@keyframes spin{from{transform:rotate(0deg);}50%, 75%{opacity:0.5;}to{transform:rotate(360deg);}}"
          .into()
      )
    );
  }

  #[test]
  fn a_keyframes_name_and_offset_cannot_carry_a_second_rule() {
    assert!(matches!(
      css(json!({ "keyframes": "a{}.b", "steps": [] })),
      Err(CssSourceError::Prelude {
        rule: "@keyframes name",
        ..
      })
    ));
    assert!(matches!(
      css(json!({ "keyframes": "spin", "steps": [{ "offset": "from{}.b" }] })),
      Err(CssSourceError::Prelude {
        rule: "keyframe offset",
        ..
      })
    ));
  }

  #[test]
  fn a_group_wraps_the_entries_it_holds() {
    assert_eq!(
      css(json!({
        "media": "(min-width: 800px)",
        "rules": [
          ".a{color:red}",
          { "selector": ".b", "style": { "width": "1px" } },
        ],
      })),
      Ok("@media (min-width: 800px){.a{color:red}.b{width:1px;}}".into())
    );
    assert_eq!(
      css(json!({ "supports": "(display: grid)", "rules": [{ "selector": ".a" }] })),
      Ok("@supports (display: grid){.a{}}".into())
    );
  }

  /// A layer with no entries declares its order alone.
  #[test]
  fn a_layer_writes_a_statement_without_entries() {
    assert_eq!(css(json!({ "layer": "base" })), Ok("@layer base;".into()));
    assert_eq!(
      css(json!({ "layer": "base.reset", "rules": [{ "selector": ".a" }] })),
      Ok("@layer base.reset{.a{}}".into())
    );
  }

  #[test]
  fn a_group_prelude_cannot_carry_a_second_rule() {
    assert!(matches!(
      css(json!({ "media": "print){} .b{color:red}(", "rules": [] })),
      Err(CssSourceError::Prelude { rule: "@media", .. })
    ));
    assert!(matches!(
      css(json!({ "supports": "(display:grid){} .b", "rules": [] })),
      Err(CssSourceError::Prelude {
        rule: "@supports",
        ..
      })
    ));
    assert!(matches!(
      css(json!({ "layer": "a{}.b" })),
      Err(CssSourceError::Prelude { rule: "@layer", .. })
    ));
  }

  #[test]
  fn a_null_value_is_no_declaration() {
    assert_eq!(
      css(json!({ "selector": ".a", "style": { "color": null, "width": "1px" } })),
      Ok(".a{width:1px;}".into())
    );
  }
}
