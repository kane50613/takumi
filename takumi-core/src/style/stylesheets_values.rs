use std::cell::RefCell;

use rustc_hash::FxHashMap;
use smallvec::SmallVec;

use super::{CssInputParseError, ParsedDeclarations, PropertyId};
use crate::style::{CssInput, CssNumber};

thread_local! {
  static VALUES: RefCell<Option<Values>> = const { RefCell::new(None) };
}

/// The CSS values one node tree's styles parsed, reused where its nodes repeat them.
pub(crate) struct ParsedValues;

impl ParsedValues {
  /// Runs `read` with the values it parses kept for reuse until it returns, unless an outer
  /// `read` already keeps them.
  pub(crate) fn scope<T>(read: impl FnOnce() -> T) -> T {
    let _scope = VALUES
      .with_borrow_mut(|values| values.is_none().then(|| *values = Some(Values::default())))
      .map(|()| Scope);

    read()
  }

  /// `input` parsed as `property`, reused when the scope parsed the same value for it before.
  pub(crate) fn parse(
    property: PropertyId,
    input: CssInput<'_>,
  ) -> Result<ParsedDeclarations, CssInputParseError<'_>> {
    let kept = VALUES.with_borrow(|values| {
      let values = values.as_ref()?;

      Some(match &input {
        CssInput::Str(text) => values
          .texts
          .get(text.as_ref())
          .and_then(|by| by.find(property)),
        CssInput::Number(number) => values
          .numbers
          .get(&NumberKey::from(*number))
          .and_then(|by| by.find(property)),
        CssInput::Unexpected(_) => None,
      })
    });

    match kept {
      Some(Some(declarations)) => return Ok(declarations),
      Some(None) => {}
      None => return property.parse_css_input_declarations(input),
    }

    let key = match &input {
      CssInput::Str(text) => Some(ValueKey::Text(text.as_ref().into())),
      CssInput::Number(number) => Some(ValueKey::Number(NumberKey::from(*number))),
      CssInput::Unexpected(_) => None,
    };
    let declarations = property.parse_css_input_declarations(input)?;

    if let Some(key) = key {
      VALUES.with_borrow_mut(|values| {
        if let Some(values) = values {
          values.keep(key, property, declarations.clone());
        }
      });
    }
    Ok(declarations)
  }
}

/// Parsed declarations by value text and by number.
#[derive(Default)]
struct Values {
  texts: FxHashMap<Box<str>, ByProperty>,
  numbers: FxHashMap<NumberKey, ByProperty>,
}

impl Values {
  fn keep(&mut self, key: ValueKey, property: PropertyId, declarations: ParsedDeclarations) {
    let by_property = match key {
      ValueKey::Text(text) => self.texts.entry(text).or_default(),
      ValueKey::Number(number) => self.numbers.entry(number).or_default(),
    };

    by_property.0.push((property, declarations));
  }
}

/// One value's declarations under each property that parsed it.
#[derive(Default)]
struct ByProperty(SmallVec<[(PropertyId, ParsedDeclarations); 1]>);

impl ByProperty {
  fn find(&self, property: PropertyId) -> Option<ParsedDeclarations> {
    self
      .0
      .iter()
      .find(|(parsed_as, _)| *parsed_as == property)
      .map(|(_, declarations)| declarations.clone())
  }
}

enum ValueKey {
  Text(Box<str>),
  Number(NumberKey),
}

/// A [`CssNumber`] as a map key: its variant and its bits, since each variant prints differently.
#[derive(PartialEq, Eq, Hash)]
enum NumberKey {
  Signed(i64),
  Unsigned(u64),
  Float(u64),
}

impl From<CssNumber> for NumberKey {
  fn from(number: CssNumber) -> Self {
    match number {
      CssNumber::Signed(value) => Self::Signed(value),
      CssNumber::Unsigned(value) => Self::Unsigned(value),
      CssNumber::Float(value) => Self::Float(value.to_bits()),
    }
  }
}

/// Drops the kept values when the scope that opened them ends, even by unwinding.
struct Scope;

impl Drop for Scope {
  fn drop(&mut self) {
    VALUES.with_borrow_mut(|values| *values = None);
  }
}
