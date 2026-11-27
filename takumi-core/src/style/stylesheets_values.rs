use std::cell::RefCell;

use rustc_hash::FxHashMap;
use smallvec::SmallVec;

use super::{CssInputParseError, ParsedDeclarations, PropertyId};
use crate::style::{CssInput, CssNumber, ParseMode};

/// The CSS values one node tree's styles parsed, reused where its nodes repeat them.
#[derive(Default)]
pub(crate) struct ParsedValues(RefCell<Values>);

impl ParsedValues {
  /// `input` parsed as `property`, reused when the tree parsed the same value for it before.
  pub(crate) fn parse<'i>(
    &self,
    property: PropertyId,
    input: CssInput<'i>,
  ) -> Result<ParsedDeclarations, CssInputParseError<'i>> {
    let kept = {
      let values = self.0.borrow();

      match &input {
        CssInput::Str(text) => values
          .texts
          .get(text.as_ref())
          .and_then(|by| by.find(property)),
        CssInput::Number(number) => values
          .numbers
          .get(&NumberKey::from(*number))
          .and_then(|by| by.find(property)),
        CssInput::Unexpected(_) => None,
      }
    };

    if let Some(declarations) = kept {
      return Ok(declarations);
    }

    let key = match &input {
      CssInput::Str(text) => Some(ValueKey::Text(text.as_ref().into())),
      CssInput::Number(number) => Some(ValueKey::Number(NumberKey::from(*number))),
      CssInput::Unexpected(_) => None,
    };
    let declarations = property.parse_css_input_declarations(input, ParseMode::Css)?;

    if let Some(key) = key {
      self
        .0
        .borrow_mut()
        .keep(key, property, declarations.clone());
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
