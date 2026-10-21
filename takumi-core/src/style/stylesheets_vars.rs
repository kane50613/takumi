use std::{borrow::Cow, sync::LazyLock};

use cssparser::{Parser, Token};
use quick_cache::{Equivalent, Weighter, sync::Cache};

use super::{DeferredDeclaration, ParsedDeclarations, PropertyId};
use crate::style::{ComputedStyle, CssInput, CustomProperties};

// Block nesting recurses here where Blink's tokenizer iterates, so it needs a
// depth of its own; the value follows Blink's `kMaxExpressionDepth`.
const MAX_VAR_DEPTH: u32 = 100;
// <https://drafts.csswg.org/css-values-5/#long-substitution>, matching Blink's
// `CSSVariableData::kMaxVariableBytes` and Firefox.
const MAX_VAR_OUTPUT_BYTES: usize = 2 * 1024 * 1024;

// Counts substituted bytes across one whole top-level resolution, not per call:
// the cycle guard stops self-reference but not fan-out, so
// `--n: var(--n-1)var(--n-1)` doubles per link without a shared ceiling.
fn charge(budget: &mut usize, bytes: usize) -> Option<()> {
  *budget = budget.checked_sub(bytes)?;
  Some(())
}

fn resolve_var_function(
  input: &mut Parser<'_>,
  custom_properties: &CustomProperties,
  stack: &mut Vec<String>,
  budget: &mut usize,
  depth: u32,
) -> Option<String> {
  let property_name = input.expect_ident_cloned().ok()?;
  if !property_name.starts_with("--") {
    return None;
  }

  let fallback = if input.try_parse(Parser::expect_comma).is_ok() {
    let mut output = String::new();
    resolve_var_tokens_into(input, custom_properties, stack, budget, depth, &mut output)?;
    Some(output)
  } else {
    None
  };

  if input.next_including_whitespace_and_comments().is_ok() {
    return None;
  }

  if stack.iter().any(|entry| entry == property_name.as_ref()) {
    return fallback;
  }

  let resolved = if let Some(specified_value) = custom_properties.get(property_name.as_ref()) {
    stack.push(property_name.to_string());
    let resolved =
      resolve_var_references_with(specified_value, custom_properties, stack, budget, depth);
    stack.pop();
    resolved
  } else {
    None
  };

  resolved.or(fallback)
}

fn resolve_var_tokens_into(
  input: &mut Parser<'_>,
  custom_properties: &CustomProperties,
  stack: &mut Vec<String>,
  budget: &mut usize,
  depth: u32,
  output: &mut String,
) -> Option<()> {
  if depth >= MAX_VAR_DEPTH {
    return None;
  }

  while !input.is_exhausted() {
    let start = input.position();
    let token = input.next_including_whitespace_and_comments().ok()?;

    match token {
      Token::Function(name) if name.eq_ignore_ascii_case("var") => {
        let resolved = input
          .parse_nested_block(|input| {
            resolve_var_function(input, custom_properties, stack, budget, depth + 1)
              .ok_or_else(|| input.new_error_for_next_token::<()>())
          })
          .ok()?;
        charge(budget, resolved.len())?;
        output.push_str(&resolved);
      }
      Token::Function(_)
      | Token::ParenthesisBlock
      | Token::SquareBracketBlock
      | Token::CurlyBracketBlock => {
        let (name, open, close) = match token {
          Token::Function(name) => (name.as_ref(), '(', ')'),
          Token::SquareBracketBlock => ("", '[', ']'),
          Token::CurlyBracketBlock => ("", '{', '}'),
          _ => ("", '(', ')'),
        };

        charge(budget, name.len() + 2)?;
        output.push_str(name);
        output.push(open);
        input
          .parse_nested_block(|input| {
            resolve_var_tokens_into(input, custom_properties, stack, budget, depth + 1, output)
              .ok_or_else(|| input.new_error_for_next_token::<()>())
          })
          .ok()?;
        output.push(close);
      }
      _ => {
        let slice = input.slice_from(start);
        charge(budget, slice.len())?;
        output.push_str(slice);
      }
    }
  }

  Some(())
}

pub(crate) fn resolve_var_references(
  specified_value: &str,
  custom_properties: &CustomProperties,
) -> Option<String> {
  let mut budget = MAX_VAR_OUTPUT_BYTES;
  let mut stack = Vec::new();

  resolve_var_references_with(
    specified_value,
    custom_properties,
    &mut stack,
    &mut budget,
    0,
  )
}

fn resolve_var_references_with(
  specified_value: &str,
  custom_properties: &CustomProperties,
  stack: &mut Vec<String>,
  budget: &mut usize,
  depth: u32,
) -> Option<String> {
  let mut parser = Parser::new(specified_value);
  let mut output = String::with_capacity(specified_value.len());
  resolve_var_tokens_into(
    &mut parser,
    custom_properties,
    stack,
    budget,
    depth,
    &mut output,
  )?;
  Some(output)
}

/// Substituted values parsed for a property, `None` for one that does not parse. Elements sharing a
/// variable substitute the same text, so it parses once until the cache evicts it to stay within
/// [`PARSED_VALUES_MAX_BYTES`] of text.
static PARSED_VALUES: LazyLock<Cache<ParsedValue, Option<ParsedDeclarations>, ByTextBytes>> =
  LazyLock::new(|| Cache::with_weighter(4096, PARSED_VALUES_MAX_BYTES, ByTextBytes));

/// The substituted text [`PARSED_VALUES`] keeps across its entries.
const PARSED_VALUES_MAX_BYTES: u64 = 1 << 20;

/// A substituted value and the property it was parsed for.
type ParsedValue = (PropertyId, Box<str>);

/// Weighs an entry by its text, which its parsed declarations grow with.
#[derive(Clone)]
struct ByTextBytes;

impl Weighter<ParsedValue, Option<ParsedDeclarations>> for ByTextBytes {
  fn weight(&self, (_, text): &ParsedValue, _: &Option<ParsedDeclarations>) -> u64 {
    text.len() as u64 + 1
  }
}

/// A [`PARSED_VALUES`] key borrowed from the text being looked up.
#[derive(Hash)]
struct ParsedValueKey<'a>(PropertyId, &'a str);

impl Equivalent<ParsedValue> for ParsedValueKey<'_> {
  fn equivalent(&self, key: &ParsedValue) -> bool {
    self.0 == key.0 && self.1 == &*key.1
  }
}

/// `value` parsed for `property`, parsing it only the first time.
fn parse_substituted(property: PropertyId, value: String) -> Option<ParsedDeclarations> {
  if let Some(declarations) = PARSED_VALUES.get(&ParsedValueKey(property, &value)) {
    return declarations;
  }

  let key = (property, value.clone().into_boxed_str());
  let declarations = property
    .parse_css_input_declarations(CssInput::Str(Cow::Owned(value)))
    .ok();

  PARSED_VALUES.insert(key, declarations.clone());
  declarations
}

/// Applies a deferred declaration, reporting whether substitution produced a
/// value so a caller with a fallback can use it.
pub(crate) fn apply_deferred_declaration(
  style: &mut ComputedStyle,
  parent: Option<&ComputedStyle>,
  deferred: &DeferredDeclaration,
) -> bool {
  let Some(resolved_value) =
    resolve_var_references(&deferred.specified_value, &style.custom_properties)
  else {
    return false;
  };
  let Some(declarations) = parse_substituted(deferred.property, resolved_value) else {
    return false;
  };

  for declaration in declarations {
    declaration.apply(style, parent);
  }

  true
}

#[cfg(test)]
mod tests {
  use super::{PARSED_VALUES, ParsedValueKey, parse_substituted};
  use crate::style::{LonghandId, PropertyId};

  #[test]
  fn a_substituted_value_parses_once_and_is_found_by_its_text() {
    let property = PropertyId::Longhand(LonghandId::RowGap);
    let parsed = parse_substituted(property, "12px".to_owned());

    assert!(parsed.is_some());
    assert_eq!(
      PARSED_VALUES.get(&ParsedValueKey(property, "12px")),
      Some(parsed)
    );
    assert_eq!(parse_substituted(property, "not a gap".to_owned()), None);
    assert_eq!(
      PARSED_VALUES.get(&ParsedValueKey(property, "not a gap")),
      Some(None)
    );
  }
}
