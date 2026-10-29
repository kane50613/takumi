//! `counter-reset`, `counter-increment`, and `counter-set`: the counters an element changes.
//! <https://drafts.csswg.org/css-lists-3/#auto-numbering>

use std::{fmt, sync::Arc};

use cssparser::{Parser, Token, match_ignore_ascii_case, serialize_identifier};

use crate::style::{Animatable, FromCss, MakeComputed, ParseResult, ToCss, unexpected_token};

/// One counter an element changes, and the integer it changes the counter by or to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CounterChange {
  /// The counter's name.
  pub name: Arc<str>,
  /// The integer the counter is reset or set to, or incremented by.
  pub value: i32,
  /// Whether `counter-reset` named the counter as `reversed(...)`.
  pub reversed: bool,
}

/// A `counter-*` list, where a counter written without an integer takes `DEFAULT`, and only
/// `counter-reset`, the one that is `REVERSIBLE`, takes `reversed(<name>)`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CounterChanges<const DEFAULT: i32, const REVERSIBLE: bool>(Box<[CounterChange]>);

/// The `counter-reset` property.
pub type CounterReset = CounterChanges<0, true>;
/// The `counter-increment` property.
pub type CounterIncrement = CounterChanges<1, false>;
/// The `counter-set` property.
pub type CounterSet = CounterChanges<0, false>;

impl<const DEFAULT: i32, const REVERSIBLE: bool> CounterChanges<DEFAULT, REVERSIBLE> {
  /// The counters, in the order the list names them.
  pub fn iter(&self) -> impl Iterator<Item = &CounterChange> {
    self.0.iter()
  }

  /// A counter's name, then its integer when it has one.
  fn change<'i>(input: &mut Parser<'i>) -> ParseResult<'i, CounterChange> {
    let (name, reversed) = match input.next()?.clone() {
      Token::Function(function) if REVERSIBLE && function.eq_ignore_ascii_case("reversed") => {
        (input.parse_nested_block(counter_name::<Self>)?, true)
      }
      token @ Token::Ident(_) => (name_of::<Self>(&token)?, false),
      token => return Err(unexpected_token!(Self, &token)),
    };
    let value = input.try_parse(Parser::expect_integer).unwrap_or(DEFAULT);

    Ok(CounterChange {
      name,
      value,
      reversed,
    })
  }
}

impl<const DEFAULT: i32, const REVERSIBLE: bool> MakeComputed
  for CounterChanges<DEFAULT, REVERSIBLE>
{
}

impl<const DEFAULT: i32, const REVERSIBLE: bool> Animatable
  for CounterChanges<DEFAULT, REVERSIBLE>
{
}

impl<'i, const DEFAULT: i32, const REVERSIBLE: bool> FromCss<'i>
  for CounterChanges<DEFAULT, REVERSIBLE>
{
  fn from_css(input: &mut Parser<'i>) -> ParseResult<'i, Self> {
    if input
      .try_parse(|input| input.expect_ident_matching("none"))
      .is_ok()
    {
      return Ok(Self::default());
    }

    let mut changes = vec![Self::change(input)?];

    while !input.is_exhausted() {
      changes.push(Self::change(input)?);
    }

    Ok(Self(changes.into_boxed_slice()))
  }

  const VALID_TOKENS: &'static [&'static str] = &["none", "<custom-ident>", "<integer>"];
}

impl<const DEFAULT: i32, const REVERSIBLE: bool> ToCss for CounterChanges<DEFAULT, REVERSIBLE> {
  fn to_css<W: fmt::Write>(&self, dest: &mut W) -> fmt::Result {
    if self.0.is_empty() {
      return dest.write_str("none");
    }

    for (index, change) in self.0.iter().enumerate() {
      if index > 0 {
        dest.write_char(' ')?;
      }

      if change.reversed {
        dest.write_str("reversed(")?;
        serialize_identifier(&change.name, dest)?;
        dest.write_char(')')?;
      } else {
        serialize_identifier(&change.name, dest)?;
      }

      write!(dest, " {}", change.value)?;
    }

    Ok(())
  }
}

/// A `<counter-name>`: a `<custom-ident>` other than `none`, errors reported as `T`'s.
/// <https://drafts.csswg.org/css-lists-3/#typedef-counter-name>
pub(crate) fn counter_name<'i, T: FromCss<'i>>(
  input: &mut Parser<'i>,
) -> ParseResult<'i, Arc<str>> {
  let token = input.next()?.clone();

  name_of::<T>(&token)
}

/// The counter name `token` spells, which excludes `none` and the CSS-wide keywords.
fn name_of<'i, T: FromCss<'i>>(token: &Token<'i>) -> ParseResult<'i, Arc<str>> {
  let Token::Ident(ident) = token else {
    return Err(unexpected_token!(T, token));
  };

  match_ignore_ascii_case! { ident,
    "none" | "initial" | "inherit" | "unset" | "revert" | "revert-layer" | "default" => {
      Err(unexpected_token!(T, token))
    },
    _ => Ok(ident.as_ref().into()),
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::style::FromCssStr;

  #[test]
  fn a_counter_without_an_integer_takes_the_property_default() {
    let increment = CounterIncrement::from_css_str("chapter section 2").unwrap();

    assert_eq!(increment.to_css_string(), "chapter 1 section 2");
  }

  #[test]
  fn only_counter_reset_takes_reversed() {
    let reset = CounterReset::from_css_str("reversed(item) 5").unwrap();

    assert_eq!(reset.to_css_string(), "reversed(item) 5");
    assert!(CounterIncrement::from_css_str("reversed(item)").is_err());
  }

  #[test]
  fn none_is_not_a_counter_name() {
    assert_eq!(
      CounterSet::from_css_str("none").unwrap(),
      CounterSet::default()
    );
    assert!(CounterSet::from_css_str("a none").is_err());
  }
}
