use std::{borrow::Cow, collections::HashMap};

use cssparser::{Parser, ParserInput, Token};

use super::DeferredDeclaration;
use crate::style::{ComputedStyle, CssInput};

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
  input: &mut Parser<'_, '_>,
  custom_properties: &HashMap<String, String>,
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
  input: &mut Parser<'_, '_>,
  custom_properties: &HashMap<String, String>,
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
      Token::Function(name) => {
        charge(budget, name.len() + 2)?;
        output.push_str(name);
        output.push('(');
        input
          .parse_nested_block(|input| {
            resolve_var_tokens_into(input, custom_properties, stack, budget, depth + 1, output)
              .ok_or_else(|| input.new_error_for_next_token::<()>())
          })
          .ok()?;
        output.push(')');
      }
      Token::ParenthesisBlock => {
        charge(budget, 2)?;
        output.push('(');
        input
          .parse_nested_block(|input| {
            resolve_var_tokens_into(input, custom_properties, stack, budget, depth + 1, output)
              .ok_or_else(|| input.new_error_for_next_token::<()>())
          })
          .ok()?;
        output.push(')');
      }
      Token::SquareBracketBlock => {
        charge(budget, 2)?;
        output.push('[');
        input
          .parse_nested_block(|input| {
            resolve_var_tokens_into(input, custom_properties, stack, budget, depth + 1, output)
              .ok_or_else(|| input.new_error_for_next_token::<()>())
          })
          .ok()?;
        output.push(']');
      }
      Token::CurlyBracketBlock => {
        charge(budget, 2)?;
        output.push('{');
        input
          .parse_nested_block(|input| {
            resolve_var_tokens_into(input, custom_properties, stack, budget, depth + 1, output)
              .ok_or_else(|| input.new_error_for_next_token::<()>())
          })
          .ok()?;
        output.push('}');
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
  custom_properties: &HashMap<String, String>,
  stack: &mut Vec<String>,
) -> Option<String> {
  let mut budget = MAX_VAR_OUTPUT_BYTES;

  resolve_var_references_with(specified_value, custom_properties, stack, &mut budget, 0)
}

fn resolve_var_references_with(
  specified_value: &str,
  custom_properties: &HashMap<String, String>,
  stack: &mut Vec<String>,
  budget: &mut usize,
  depth: u32,
) -> Option<String> {
  let mut parser_input = ParserInput::new(specified_value);
  let mut parser = Parser::new(&mut parser_input);
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

/// Applies a deferred declaration, reporting whether substitution produced a
/// value so a caller with a fallback can use it.
pub(crate) fn apply_deferred_declaration(
  style: &mut ComputedStyle,
  parent: Option<&ComputedStyle>,
  deferred: &DeferredDeclaration,
) -> bool {
  let Some(resolved_value) = resolve_var_references(
    &deferred.specified_value,
    &style.custom_properties,
    &mut Vec::new(),
  ) else {
    return false;
  };

  let declarations = deferred
    .property
    .parse_css_input_declarations(CssInput::Str(Cow::Owned(resolved_value)))
    .ok();

  let Some(declarations) = declarations else {
    return false;
  };

  for declaration in declarations {
    match parent {
      Some(parent) => declaration.apply_with_parent(style, parent),
      None => declaration.apply_to_computed(style),
    }
  }

  true
}
