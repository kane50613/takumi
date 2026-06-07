use std::{borrow::Cow, collections::HashMap};

use cssparser::{Parser, ParserInput, Token};

use super::DeferredDeclaration;
use crate::style::{ComputedStyle, CssInput};

pub(crate) fn resolve_var_function(
  input: &mut Parser<'_, '_>,
  custom_properties: &HashMap<String, String>,
  stack: &mut Vec<String>,
) -> Option<String> {
  let property_name = input.expect_ident_cloned().ok()?;
  if !property_name.starts_with("--") {
    return None;
  }

  let fallback = if input.try_parse(Parser::expect_comma).is_ok() {
    let mut output = String::new();
    resolve_var_tokens_into(input, custom_properties, stack, &mut output)?;
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
    let resolved = resolve_var_references(specified_value, custom_properties, stack);
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
  output: &mut String,
) -> Option<()> {
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
        input
          .parse_nested_block(|input| {
            resolve_var_tokens_into(input, custom_properties, stack, output)
              .ok_or_else(|| input.new_error_for_next_token::<()>())
          })
          .ok()?;
        output.push(')');
      }
      Token::ParenthesisBlock => {
        output.push('(');
        input
          .parse_nested_block(|input| {
            resolve_var_tokens_into(input, custom_properties, stack, output)
              .ok_or_else(|| input.new_error_for_next_token::<()>())
          })
          .ok()?;
        output.push(')');
      }
      Token::SquareBracketBlock => {
        output.push('[');
        input
          .parse_nested_block(|input| {
            resolve_var_tokens_into(input, custom_properties, stack, output)
              .ok_or_else(|| input.new_error_for_next_token::<()>())
          })
          .ok()?;
        output.push(']');
      }
      Token::CurlyBracketBlock => {
        output.push('{');
        input
          .parse_nested_block(|input| {
            resolve_var_tokens_into(input, custom_properties, stack, output)
              .ok_or_else(|| input.new_error_for_next_token::<()>())
          })
          .ok()?;
        output.push('}');
      }
      _ => output.push_str(input.slice_from(start)),
    }
  }

  Some(())
}

pub(crate) fn resolve_var_references(
  specified_value: &str,
  custom_properties: &HashMap<String, String>,
  stack: &mut Vec<String>,
) -> Option<String> {
  let mut parser_input = ParserInput::new(specified_value);
  let mut parser = Parser::new(&mut parser_input);
  let mut output = String::with_capacity(specified_value.len());
  resolve_var_tokens_into(&mut parser, custom_properties, stack, &mut output)?;
  Some(output)
}

pub(crate) fn apply_deferred_declaration(
  style: &mut ComputedStyle,
  parent: Option<&ComputedStyle>,
  deferred: &DeferredDeclaration,
) {
  let Some(resolved_value) = resolve_var_references(
    &deferred.specified_value,
    &style.custom_properties,
    &mut Vec::new(),
  ) else {
    return;
  };

  let declarations = deferred
    .property
    .parse_css_input_declarations(CssInput::Str(Cow::Owned(resolved_value)))
    .ok();

  let Some(declarations) = declarations else {
    return;
  };

  for declaration in declarations {
    match parent {
      Some(parent) => declaration.apply_with_parent(style, parent),
      None => declaration.apply_to_computed(style),
    }
  }
}
