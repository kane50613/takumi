use std::collections::HashMap;

use cssparser::{Parser, Token, TokenSerializationType};

use super::DeferredDeclaration;
use crate::style::{ComputedStyle, CssWideKeyword, CustomProperties, FromCssStr, PropertySyntax};

const MAX_VAR_DEPTH: u32 = 100;
const MAX_VAR_OUTPUT_BYTES: usize = 2 * 1024 * 1024;

#[derive(Clone, Default)]
struct TokenSequence {
  text: String,
  last_token: TokenSerializationType,
  last_non_whitespace_token: TokenSerializationType,
}
impl TokenSequence {
  fn append(&mut self, text: &str, budget: &mut usize) -> Result<(), ()> {
    let mut parser = Parser::new(text);
    let mut first_token = None;
    let mut last_token = None;
    let mut last_non_whitespace_token = None;

    while let Ok(token) = parser.next_including_whitespace_and_comments() {
      let token_type = token.serialization_type();
      first_token.get_or_insert(token_type);
      last_token = Some(token_type);
      if !matches!(token, Token::WhiteSpace(_) | Token::Comment(_)) {
        last_non_whitespace_token = Some(token_type);
      }
    }

    let Some(first_token) = first_token else {
      return Ok(());
    };

    let separator = self.last_token.needs_separator_when_before(first_token);
    charge(budget, text.len() + usize::from(separator) * 4)?;
    if separator {
      self.text.push_str("/**/");
    }
    self.text.push_str(text);
    self.last_token = last_token.unwrap_or(TokenSerializationType::Nothing);
    if let Some(token) = last_non_whitespace_token {
      self.last_non_whitespace_token = token;
    }
    Ok(())
  }

  fn append_sequence(
    &mut self,
    other: &Self,
    fallback: bool,
    budget: &mut usize,
  ) -> Result<(), ()> {
    let text = if fallback {
      strip_trailing_whitespace_and_comments(&other.text)
    } else {
      &other.text
    };
    if text.is_empty() {
      return Ok(());
    }

    self.append(text, budget)?;
    self.last_token = if fallback {
      other.last_non_whitespace_token
    } else {
      other.last_token
    };
    self.last_non_whitespace_token = other.last_non_whitespace_token;
    Ok(())
  }
}

fn strip_trailing_whitespace_and_comments(text: &str) -> &str {
  #[derive(Clone, Copy, PartialEq)]
  enum State {
    Default,
    SingleQuote,
    DoubleQuote,
    Comment,
  }

  if !text
    .as_bytes()
    .iter()
    .any(|byte| matches!(byte, b'/' | b'\\'))
  {
    return text.trim_end_matches([' ', '\t', '\n', '\r', '\u{0C}']);
  }

  let mut end = 0;
  let mut state = State::Default;
  let mut chars = text.char_indices().peekable();

  while let Some((index, ch)) = chars.next() {
    if state == State::Comment {
      if ch == '*' && chars.next_if(|(_, next)| *next == '/').is_some() {
        state = State::Default;
      }
      continue;
    }
    if state == State::Default && matches!(ch, ' ' | '\t' | '\n' | '\r' | '\u{0C}') {
      continue;
    }
    if ch == '\\'
      && let Some((escaped_index, escaped)) = chars.next()
    {
      if state == State::Default {
        end = escaped_index + escaped.len_utf8();
      }
      continue;
    }
    if state == State::Default && ch == '/' && chars.next_if(|(_, next)| *next == '*').is_some() {
      state = State::Comment;
      continue;
    }

    end = index + ch.len_utf8();
    state = match (state, ch) {
      (State::Default, '\'') => State::SingleQuote,
      (State::Default, '"') => State::DoubleQuote,
      (State::SingleQuote, '\'') | (State::DoubleQuote, '"') => State::Default,
      _ => state,
    };
  }

  &text[..end]
}

#[derive(Default)]
struct CycleResolver {
  stack: Vec<String>,
  cycle_start: Option<usize>,
  cycle_end: Option<usize>,
}

impl CycleResolver {
  fn detect_cycle(&mut self, name: &str) -> bool {
    let Some(index) = self.stack.iter().position(|entry| entry == name) else {
      return false;
    };
    self.cycle_start = Some(self.cycle_start.map_or(index, |start| start.min(index)));
    self.cycle_end = Some(self.stack.len());
    true
  }

  fn in_cycle(&self) -> bool {
    matches!((self.cycle_start, self.cycle_end), (Some(start), Some(end))
      if self.stack.len() > start && self.stack.len() <= end)
  }

  fn push(&mut self, name: &str) {
    self.stack.push(name.to_owned());
  }

  fn pop(&mut self) {
    self.stack.pop();
    if let Some(end) = &mut self.cycle_end {
      *end = (*end).min(self.stack.len());
      if self.cycle_start.is_some_and(|start| *end <= start) {
        self.cycle_start = None;
        self.cycle_end = None;
      }
    }
  }
}

#[derive(Clone)]
enum Resolution {
  Invalid,
  Valid(String),
}

/// The longhands a registered custom property reads through font-relative units, with the units
/// that read them and the ones that read them only on the root, as nodes of the cycle graph.
/// <https://drafts.css-houdini.org/css-properties-values-api-1/#dependency-cycles>
const FONT_LONGHANDS: [(&str, &[&str], &[&str]); 2] = [
  (
    "font-size",
    &["em", "ex", "cap", "ch", "ic", "lh"],
    &["rem", "rlh"],
  ),
  ("line-height", &["lh"], &["rlh"]),
];

/// The `var()` values of [`FONT_LONGHANDS`] an element declares, which a registered custom
/// property with font-relative units reads.
#[derive(Clone, Copy)]
struct FontDependencies<'a> {
  values: [Option<&'a str>; 2],
  is_document_root: bool,
}

struct Resolver<'a> {
  specified: &'a CustomProperties,
  /// The inherited custom properties a substituted CSS-wide keyword reads, if it resolves.
  parent: Option<&'a CustomProperties>,
  resolved: HashMap<String, Resolution>,
  cycles: CycleResolver,
  budget: usize,
  fonts: Option<FontDependencies<'a>>,
  /// Which of [`FONT_LONGHANDS`] a cycle reached.
  font_cycles: [bool; 2],
  /// The custom properties a cycle reached.
  cycle_names: Vec<String>,
}

impl<'a> Resolver<'a> {
  fn new(specified: &'a CustomProperties, parent: Option<&'a CustomProperties>) -> Self {
    Self {
      specified,
      parent,
      resolved: HashMap::new(),
      cycles: CycleResolver::default(),
      budget: MAX_VAR_OUTPUT_BYTES,
      fonts: None,
      font_cycles: [false; 2],
      cycle_names: Vec::new(),
    }
  }

  /// Walks the font longhand values the registered `name`, resolved to `value`, reads through its
  /// font-relative units, as Blink's `CascadeResolver` does, so a cycle back to `name` reaches both.
  fn follow_font_longhands(&mut self, name: &str, value: &str, depth: u32) {
    let Some(fonts) = self.fonts else {
      return;
    };

    if self
      .specified
      .registration(name)
      .is_none_or(|rule| rule.syntax == PropertySyntax::Universal)
    {
      return;
    }
    for (index, ((longhand, units, root_units), longhand_value)) in
      FONT_LONGHANDS.into_iter().zip(fonts.values).enumerate()
    {
      let Some(longhand_value) = longhand_value else {
        continue;
      };

      if !has_unit(value, units) && !(fonts.is_document_root && has_unit(value, root_units)) {
        continue;
      }
      self.cycles.push(longhand);
      let _ = self.resolve_value(longhand_value, depth);
      if self.cycles.in_cycle() {
        self.font_cycles[index] = true;
      }
      self.cycles.pop();
    }
  }

  fn resolve_property(&mut self, name: &str, depth: u32) -> Option<String> {
    if depth >= MAX_VAR_DEPTH {
      return None;
    }

    match self.resolved.get(name) {
      Some(Resolution::Valid(value)) => return Some(value.clone()),
      Some(Resolution::Invalid) => return None,
      None => {}
    }

    let Some(specified) = self.specified.get(name) else {
      self.resolved.insert(name.to_owned(), Resolution::Invalid);
      return None;
    };

    self.cycles.push(name);
    let saved_budget = std::mem::replace(&mut self.budget, MAX_VAR_OUTPUT_BYTES);
    let value =
      self.resolve_value(specified, depth + 1).and_then(
        |value| match CssWideKeyword::from_css_str(&value) {
          Ok(keyword) if !keyword.reverts() && self.parent.is_some() => {
            self.specified.keyword_value(name, keyword, self.parent)
          }
          _ => Some(value),
        },
      );

    if let Some(value) = &value {
      self.follow_font_longhands(name, value, depth + 1);
    }
    self.budget = saved_budget;
    let in_cycle = self.cycles.in_cycle();

    if in_cycle {
      self.cycle_names.push(name.to_owned());
    }
    self.cycles.pop();

    match (value, in_cycle) {
      (Some(value), false) => {
        self
          .resolved
          .insert(name.to_owned(), Resolution::Valid(value.clone()));
        Some(value)
      }
      _ => {
        self.resolved.insert(name.to_owned(), Resolution::Invalid);
        None
      }
    }
  }

  fn resolve_value(&mut self, specified: &str, depth: u32) -> Option<String> {
    let mut input = Parser::new(specified);
    let mut sequence = TokenSequence::default();
    self
      .resolve_tokens_into(&mut input, depth, &mut sequence)
      .ok()?;
    Some(sequence.text)
  }

  fn resolve_tokens_into(
    &mut self,
    input: &mut Parser<'_>,
    depth: u32,
    output: &mut TokenSequence,
  ) -> Result<(), ()> {
    if depth >= MAX_VAR_DEPTH {
      return Err(());
    }

    loop {
      let start = input.position();
      let Ok(token) = input.next_including_whitespace_and_comments() else {
        break;
      };

      match token {
        Token::Function(name) if name.eq_ignore_ascii_case("var") => {
          input
            .parse_nested_block(|input| {
              self
                .resolve_var_into(input, depth + 1, output)
                .map_err(|_| input.new_error_for_next_token::<()>())
            })
            .map_err(|_| ())?;
        }
        Token::Function(_) => {
          let prefix = input.slice_from(start).to_owned();
          output.append(&prefix, &mut self.budget)?;
          input
            .parse_nested_block(|input| {
              self
                .resolve_tokens_into(input, depth + 1, output)
                .map_err(|_| input.new_error_for_next_token::<()>())
            })
            .map_err(|_| ())?;
          output.append(")", &mut self.budget)?;
        }
        Token::ParenthesisBlock | Token::SquareBracketBlock | Token::CurlyBracketBlock => {
          let (open, close) = match token {
            Token::ParenthesisBlock => ("(", ")"),
            Token::SquareBracketBlock => ("[", "]"),
            _ => ("{", "}"),
          };
          output.append(open, &mut self.budget)?;
          input
            .parse_nested_block(|input| {
              self
                .resolve_tokens_into(input, depth + 1, output)
                .map_err(|_| input.new_error_for_next_token::<()>())
            })
            .map_err(|_| ())?;
          output.append(close, &mut self.budget)?;
        }
        _ => output.append(input.slice_from(start), &mut self.budget)?,
      }
    }

    Ok(())
  }

  fn resolve_var_into(
    &mut self,
    input: &mut Parser<'_>,
    depth: u32,
    output: &mut TokenSequence,
  ) -> Result<(), ()> {
    let property_name = input.expect_ident_cloned().map_err(|_| ())?;
    if !property_name.starts_with("--") {
      return Err(());
    }

    if !self.cycles.detect_cycle(property_name.as_ref()) {
      self.resolve_property(property_name.as_ref(), depth + 1);
    }
    if self.cycles.in_cycle() {
      return Err(());
    }

    let value = match self.resolved.get(property_name.as_ref()) {
      Some(Resolution::Valid(value)) => Some(TokenSequence::from_text(value)),
      Some(Resolution::Invalid) | None => None,
    };
    let has_fallback = input.try_parse(Parser::expect_comma).is_ok();

    match (value, has_fallback) {
      (Some(value), true) => {
        skip_tokens(input)?;
        input.expect_exhausted().map_err(|_| ())?;
        output.append_sequence(&value, false, &mut self.budget)
      }
      (Some(value), false) => {
        input.expect_exhausted().map_err(|_| ())?;
        output.append_sequence(&value, false, &mut self.budget)
      }
      (None, false) => Err(()),
      (None, true) => {
        let mut fallback = TokenSequence::default();
        self.resolve_tokens_into(input, depth + 1, &mut fallback)?;
        input.expect_exhausted().map_err(|_| ())?;
        output.append_sequence(&fallback, true, &mut self.budget)
      }
    }
  }
}

impl TokenSequence {
  fn from_text(text: &str) -> Self {
    let mut sequence = Self::default();
    let mut budget = usize::MAX;
    let _ = sequence.append(text, &mut budget);
    sequence
  }
}

fn skip_tokens(input: &mut Parser<'_>) -> Result<(), ()> {
  while !input.is_exhausted() {
    match input
      .next_including_whitespace_and_comments()
      .map_err(|_| ())?
    {
      Token::Function(_)
      | Token::ParenthesisBlock
      | Token::SquareBracketBlock
      | Token::CurlyBracketBlock => {
        input
          .parse_nested_block(|input| {
            skip_tokens(input).map_err(|_| input.new_error_for_next_token::<()>())
          })
          .map_err(|_| ())?;
      }
      _ => {}
    }
  }
  Ok(())
}

fn charge(budget: &mut usize, bytes: usize) -> Result<(), ()> {
  *budget = budget.checked_sub(bytes).ok_or(())?;
  Ok(())
}

/// Resolves `var()` in the custom properties an element declares, `declared`, as Blink applies
/// them: the values it inherits resolved already. Blink's `ApplyMatchResult` applies every one it
/// declares, and resolves one sooner only when a property applied first reads it, so resolving them
/// all first gives its values; the cycles that order finds are the font ones walked below. Returns which of `font-size` and `line-height`,
/// whose `var()` values `fonts` holds, a cycle through a registered property's font-relative units
/// reaches. A registered property in a cycle takes its `unset` value, as Blink's `ApplyUnset` gives
/// it, and the others read it there.
pub(crate) fn resolve_declared_custom_properties(
  style: &mut ComputedStyle,
  declared: &[String],
  parent: &ComputedStyle,
  fonts: [Option<&str>; 2],
  is_document_root: bool,
) -> [bool; 2] {
  let properties = &mut style.custom_properties;

  properties.enforce_syntax(declared, &parent.custom_properties);

  let follows_fonts = fonts.iter().any(Option::is_some) && properties.has_registrations();

  if !follows_fonts
    && !declared
      .iter()
      .any(|name| properties.get(name).is_some_and(contains_var_references))
  {
    return [false; 2];
  }

  let specified: Vec<(String, Option<String>)> = declared
    .iter()
    .map(|name| (name.clone(), properties.get(name).map(str::to_owned)))
    .collect();
  let (resolved, font_cycles, cycle_names) = {
    let mut resolver = Resolver::new(properties, Some(&parent.custom_properties));

    resolver.fonts = Some(FontDependencies {
      values: fonts,
      is_document_root,
    });
    let resolved: Vec<Option<String>> = declared
      .iter()
      .map(|name| resolver.resolve_property(name, 0))
      .collect();

    (resolved, resolver.font_cycles, resolver.cycle_names)
  };

  set_resolved(properties, declared, resolved, &cycle_names);

  let registered_in_cycle = cycle_names.iter().any(|name| {
    properties
      .registration(name)
      .is_some_and(|rule| rule.syntax != PropertySyntax::Universal)
  });

  properties.enforce_syntax(declared, &parent.custom_properties);
  if registered_in_cycle {
    let others: Vec<String> = specified
      .into_iter()
      .filter(|(name, _)| !cycle_names.contains(name))
      .map(|(name, value)| {
        if let Some(value) = value {
          properties.set(name.clone(), value);
        }
        name
      })
      .collect();
    let resolved: Vec<Option<String>> = {
      let mut resolver = Resolver::new(properties, Some(&parent.custom_properties));

      others
        .iter()
        .map(|name| resolver.resolve_property(name, 0))
        .collect()
    };

    set_resolved(properties, &others, resolved, &[]);
    properties.enforce_syntax(&others, &parent.custom_properties);
  }
  font_cycles
}

/// Sets each of `names` to its `resolved` value, removing one that has none: one that reads
/// `var()` or is among `cycle_names`. A value without `var()` reads nothing, so one too deep to
/// walk stays as specified.
fn set_resolved(
  properties: &mut CustomProperties,
  names: &[String],
  resolved: Vec<Option<String>>,
  cycle_names: &[String],
) {
  for (name, value) in names.iter().zip(resolved) {
    match value {
      Some(value) => properties.set(name.clone(), value),
      None
        if cycle_names.contains(name)
          || properties.get(name).is_some_and(contains_var_references) =>
      {
        properties.remove(name);
      }
      None => {}
    }
  }
}

/// Whether `value` holds a dimension in one of `units`, at any depth.
fn has_unit(value: &str, units: &[&str]) -> bool {
  fn walk(input: &mut Parser<'_>, units: &[&str]) -> bool {
    while let Ok(token) = input.next() {
      match token {
        Token::Dimension { unit, .. }
          if units.iter().any(|known| unit.eq_ignore_ascii_case(known)) =>
        {
          return true;
        }
        Token::Function(_) | Token::ParenthesisBlock => {
          let nested = input
            .parse_nested_block(|input| Ok::<_, cssparser::ParseError<()>>(walk(input, units)))
            .unwrap_or(false);

          if nested {
            return true;
          }
        }
        _ => {}
      }
    }
    false
  }

  walk(&mut Parser::new(value), units)
}

pub(crate) fn contains_var_references(value: &str) -> bool {
  // A reference spells `var(`, or escapes it, so text with neither skips tokenizing.
  let bytes = value.as_bytes();

  if !bytes.contains(&b'\\')
    && !bytes
      .windows(4)
      .any(|window| window.eq_ignore_ascii_case(b"var("))
  {
    return false;
  }

  let mut input = Parser::new(value);

  contains_var_references_in(&mut input).unwrap_or(true)
}

fn contains_var_references_in(input: &mut Parser<'_>) -> Result<bool, ()> {
  while let Ok(token) = input.next_including_whitespace_and_comments() {
    match token {
      Token::Function(name) if name.eq_ignore_ascii_case("var") => return Ok(true),
      Token::Function(_)
      | Token::ParenthesisBlock
      | Token::SquareBracketBlock
      | Token::CurlyBracketBlock => {
        let contains_var = input
          .parse_nested_block(|input| {
            contains_var_references_in(input).map_err(|_| input.new_error_for_next_token::<()>())
          })
          .map_err(|_| ())?;
        if contains_var {
          return Ok(true);
        }
      }
      _ => {}
    }
  }

  Ok(false)
}
pub(crate) fn resolve_var_references(
  specified_value: &str,
  custom_properties: &CustomProperties,
) -> Option<String> {
  let mut resolver = Resolver::new(custom_properties, None);
  resolver.resolve_value(specified_value, 0)
}

/// Applies a deferred declaration, reporting whether substitution produced a
/// value so a caller with a fallback can use it.
pub(crate) fn apply_deferred_declaration(
  style: &mut ComputedStyle,
  parent: Option<&ComputedStyle>,
  deferred: &DeferredDeclaration,
) -> bool {
  let Some(declarations) = deferred.substituted(&style.custom_properties) else {
    return false;
  };

  for declaration in declarations {
    declaration.apply(style, parent);
  }

  true
}

#[cfg(test)]
mod tests {
  use cssparser::Parser;

  use super::{
    contains_var_references, contains_var_references_in, strip_trailing_whitespace_and_comments,
  };

  #[test]
  fn the_byte_scan_agrees_with_the_tokenizer() {
    for value in [
      "red",
      "foo(",
      "a)",
      "[(",
      "\"unterminated",
      "url(x",
      "calc(1px + 2px",
    ] {
      assert_eq!(
        contains_var_references(value),
        contains_var_references_in(&mut Parser::new(value)).unwrap_or(true),
        "{value}"
      );
    }
  }

  #[test]
  fn strips_only_trailing_whitespace_and_comments() {
    assert_eq!(
      strip_trailing_whitespace_and_comments("rgb(251, 44, 54) /* fallback */ \t"),
      "rgb(251, 44, 54)"
    );
    assert_eq!(
      strip_trailing_whitespace_and_comments("'/* literal */' /* comment */"),
      "'/* literal */'"
    );
    assert_eq!(
      strip_trailing_whitespace_and_comments("value /* unfinished"),
      "value"
    );
    assert_eq!(
      strip_trailing_whitespace_and_comments("foo\\  \t"),
      "foo\\ "
    );
  }

  #[test]
  fn finds_variable_references_in_nested_and_escaped_functions() {
    assert!(!contains_var_references("rgb(251, 44, 54)"));
    assert!(contains_var_references("calc(var(--space) + 2px)"));
    assert!(contains_var_references("v\\61r(--space)"));
  }
}
