//! The two shapes a caller can hand the `css` render option: stylesheet text,
//! or a rule written as an object.

use std::fmt::Write;

use cssparser::{ParseError, Parser, ParserInput};
use selectors::parser::{ParseRelative, SelectorList};
use serde::{
  Deserialize,
  de::{MapAccess, Visitor},
};

use crate::style::{
  CssInput, CssUnexpected, CssValueSeed, PropertyId,
  selector::{SelectorImpl, TakumiSelectorParser},
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
}

/// Why a rule object could not become CSS.
#[derive(Debug, Clone, PartialEq)]
pub enum CssSourceError {
  /// The selector is not a selector list.
  Selector(String),
  /// The animation name is not an identifier.
  KeyframesName(String),
  /// The step is not `from`, `to`, or a percentage.
  KeyframeOffset(String),
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
      Self::Selector(selector) => write!(formatter, "invalid selector {selector:?}"),
      Self::KeyframesName(name) => write!(formatter, "invalid keyframes name {name:?}"),
      Self::KeyframeOffset(offset) => write!(formatter, "invalid keyframe offset {offset:?}"),
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
    match self {
      Self::Text(text) => Ok(text),
      Self::Rule(rule) => {
        let mut css = String::new();
        rule.write_css(&mut css)?;
        Ok(css)
      }
      Self::Keyframes(rule) => {
        let mut css = String::new();
        rule.write_css(&mut css)?;
        Ok(css)
      }
    }
  }
}

impl AnimationRule {
  fn write_css(&self, css: &mut String) -> Result<(), CssSourceError> {
    validate_ident(&self.keyframes)
      .map_err(|_| CssSourceError::KeyframesName(self.keyframes.clone()))?;

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

fn validate_ident(ident: &str) -> Result<(), ()> {
  let mut parser_input = ParserInput::new(ident);
  let mut parser = Parser::new(&mut parser_input);

  parser
    .parse_entirely(|parser| parser.expect_ident().cloned().map_err(Into::into))
    .map(|_| ())
    .map_err(|_: ParseError<'_, ()>| ())
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
    .map_err(|_: ParseError<'_, ()>| CssSourceError::KeyframeOffset(offset.to_owned()))
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
    Err(_) => Err(CssSourceError::Selector(selector.to_owned())),
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
      Err(CssSourceError::Selector(_))
    ));
    assert!(matches!(
      css(json!({ "selector": "}" })),
      Err(CssSourceError::Selector(_))
    ));
    assert!(matches!(
      css(json!({ "selector": "@media print" })),
      Err(CssSourceError::Selector(_))
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
      Err(CssSourceError::KeyframesName(_))
    ));
    assert!(matches!(
      css(json!({ "keyframes": "spin", "steps": [{ "offset": "from{}.b" }] })),
      Err(CssSourceError::KeyframeOffset(_))
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
