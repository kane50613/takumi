//! Tailwind's structural variants, such as `first:` and `nth-3:`, as the selectors they compile to.

use cssparser::Parser;
use selectors::parser::{ParseRelative, Selector, SelectorList};

use crate::style::selector::{SelectorImpl, TakumiSelectorParser};

/// The pseudo-class a structural variant compiles to, or `None` for any other variant.
pub(super) fn pseudo_class(variant: &str) -> Option<String> {
  let fixed = match variant {
    "first" => ":first-child",
    "last" => ":last-child",
    "only" => ":only-child",
    "odd" => ":nth-child(odd)",
    "even" => ":nth-child(even)",
    "first-of-type" => ":first-of-type",
    "last-of-type" => ":last-of-type",
    "only-of-type" => ":only-of-type",
    "empty" => ":empty",
    _ => "",
  };

  if !fixed.is_empty() {
    return Some(fixed.to_owned());
  }

  if let Some(negated) = variant.strip_prefix("not-") {
    return Some(format!(":not({})", pseudo_class(negated)?));
  }

  // An arbitrary variant that only adds to the element's own compound, such as `[&:nth-child(3)]`.
  if let Some(compound) = variant
    .strip_prefix("[&")
    .and_then(|rest| rest.strip_suffix(']'))
  {
    return compound.starts_with(':').then(|| arbitrary(compound));
  }

  for (prefix, function) in [
    ("nth-last-of-type-", "nth-last-of-type"),
    ("nth-of-type-", "nth-of-type"),
    ("nth-last-", "nth-last-child"),
    ("nth-", "nth-child"),
  ] {
    let Some(argument) = variant.strip_prefix(prefix) else {
      continue;
    };
    let argument = match argument
      .strip_prefix('[')
      .and_then(|rest| rest.strip_suffix(']'))
    {
      Some(bracketed) => arbitrary(bracketed),
      None => argument.parse::<u32>().ok()?.to_string(),
    };

    return Some(format!(":{function}({argument})"));
  }

  None
}

/// An arbitrary value, which Tailwind writes with underscores for spaces.
fn arbitrary(value: &str) -> String {
  value.replace('_', " ")
}

/// The selector `pseudo_classes` form on the element itself, or `None` when they do not parse or
/// reach past it through a combinator.
pub(super) fn condition(pseudo_classes: &str) -> Option<Selector<SelectorImpl>> {
  let list = SelectorList::parse(
    &TakumiSelectorParser,
    &mut Parser::new(pseudo_classes),
    ParseRelative::No,
  )
  .ok()?;
  let [selector] = list.slice() else {
    return None;
  };
  let mut components = selector.iter();

  components.by_ref().for_each(drop);

  components
    .next_sequence()
    .is_none()
    .then(|| selector.clone())
}

#[cfg(test)]
mod tests {
  use super::{condition, pseudo_class};

  #[test]
  fn structural_variants_compile_to_pseudo_classes() {
    for (variant, expected) in [
      ("first", Some(":first-child")),
      ("odd", Some(":nth-child(odd)")),
      ("nth-3", Some(":nth-child(3)")),
      ("nth-last-of-type-2", Some(":nth-last-of-type(2)")),
      ("nth-[2n_+_1]", Some(":nth-child(2n + 1)")),
      ("not-first", Some(":not(:first-child)")),
      ("[&:nth-child(4)]", Some(":nth-child(4)")),
      ("nth-x", None),
      ("hover", None),
      ("md", None),
    ] {
      assert_eq!(pseudo_class(variant).as_deref(), expected, "{variant}");
    }
  }

  #[test]
  fn a_condition_stays_on_the_element() {
    assert!(condition(":first-child:nth-child(odd)").is_some());
    assert!(condition(":first-child > span").is_none());
    assert!(condition(":first-child, :last-child").is_none());
  }
}
