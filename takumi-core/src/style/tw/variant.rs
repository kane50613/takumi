//! Tailwind's variants, such as `md:`, `first:`, and `before:`: the conditions a utility applies
//! under, and where they sort it.

use std::{collections::HashMap, sync::Arc};

use cssparser::{Parser, match_ignore_ascii_case};
use selectors::parser::{ParseRelative, Selector, SelectorList};

use super::is_ident;
use crate::{
  style::{
    Length,
    selector::{PseudoElement, SelectorImpl, TakumiSelectorParser},
  },
  viewport::Viewport,
};

/// Widths a stylesheet's `--breakpoint-*` variables assign, keyed by token.
pub(crate) type BreakpointOverrides = HashMap<String, Length>;

/// A breakpoint variant, `md:`, which applies from a viewport width on.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Breakpoint {
  /// Built-in width; `None` for a token only a `--breakpoint-*` variable defines.
  pub width: Option<Length>,
  /// The variant token, which a `--breakpoint-<token>` variable re-sizes.
  pub token: Arc<str>,
}

impl Breakpoint {
  /// Parse a breakpoint from a token.
  pub fn parse(token: &str) -> Option<Self> {
    let width = match_ignore_ascii_case! {token,
      "sm" => Some(Length::Rem(40.0)),
      "md" => Some(Length::Rem(48.0)),
      "lg" => Some(Length::Rem(64.0)),
      "xl" => Some(Length::Rem(80.0)),
      "2xl" => Some(Length::Rem(96.0)),
      _ => None,
    };

    if width.is_none() && !is_ident(token) {
      return None;
    }

    Some(Breakpoint {
      width,
      token: token.to_ascii_lowercase().into(),
    })
  }

  /// Check if the breakpoint matches the viewport width.
  pub fn matches(&self, viewport: Viewport, overrides: &BreakpointOverrides) -> bool {
    let Some(viewport_width) = viewport.size.width else {
      return false;
    };
    let Some(width) = overrides.get(&*self.token).copied().or(self.width) else {
      return false;
    };

    let breakpoint_width = match width {
      Length::Rem(value) => viewport.to_device(value * viewport.font_size),
      Length::Px(value) => viewport.to_device(value),
      Length::Vw(value) => (value / 100.0) * viewport_width as f32,
      _ => return false,
    };

    viewport_width >= breakpoint_width as u32
  }

  /// Where the breakpoint sorts among others: by its built-in width, then its token. Approximate:
  /// Tailwind compares the widths a theme gives.
  fn order(&self) -> (u32, Arc<str>) {
    let rem = match self.width {
      Some(Length::Rem(rem)) => rem,
      Some(Length::Px(px)) => px / 16.0,
      _ => f32::INFINITY,
    };

    (rem.to_bits(), Arc::clone(&self.token))
  }
}

/// One variant of a utility, as Tailwind's `parseVariant` reads it.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Variant {
  Breakpoint(Breakpoint),
  /// A pseudo-class on the element itself, such as `:first-child` for `first:`.
  Structural {
    pseudo_class: String,
    rank: u8,
  },
  PseudoElement(PseudoElement),
}

/// Where Tailwind registers `before:` and `after:`, after `not-*` and ahead of every structural
/// variant.
const BEFORE_RANK: u8 = 1;
const AFTER_RANK: u8 = 2;

/// The structural variants in the order Tailwind registers them, from `STRUCTURAL_RANK` on.
const REGISTERED: [&str; 9] = [
  "first",
  "last",
  "only",
  "odd",
  "even",
  "first-of-type",
  "last-of-type",
  "only-of-type",
  "empty",
];
const STRUCTURAL_RANK: u8 = 3;
const FUNCTIONAL: [&str; 4] = ["nth-", "nth-last-", "nth-of-type-", "nth-last-of-type-"];

impl Variant {
  /// The variant `token` names, or `None` for one takumi does not support.
  pub(super) fn parse(token: &str) -> Option<Self> {
    match token {
      "before" => return Some(Self::PseudoElement(PseudoElement::Before)),
      "after" => return Some(Self::PseudoElement(PseudoElement::After)),
      _ => {}
    }

    if let Some(pseudo_class) = pseudo_class(token) {
      return Some(Self::Structural {
        rank: structural_rank(token),
        pseudo_class,
      });
    }

    Breakpoint::parse(token).map(Self::Breakpoint)
  }
}

/// Where a structural variant sorts: `not-*` first, the registered ones in Tailwind's order, then
/// the `nth-*` families, and an arbitrary variant last. Approximate: Tailwind also orders the values
/// of one variant, such as `nth-2` before `nth-3`, and the variants inside `not-`.
fn structural_rank(token: &str) -> u8 {
  if token.starts_with("not-") {
    return 0;
  }

  if let Some(index) = REGISTERED.iter().position(|name| *name == token) {
    return STRUCTURAL_RANK + index as u8;
  }

  let functional = FUNCTIONAL
    .iter()
    .rposition(|prefix| token.starts_with(prefix))
    .unwrap_or(FUNCTIONAL.len());

  STRUCTURAL_RANK + REGISTERED.len() as u8 + functional as u8
}

/// The variants of one utility: at most one breakpoint and one pseudo-element, and the
/// pseudo-classes they put on the element.
#[derive(Debug, Default)]
pub(super) struct Variants {
  pub(super) breakpoint: Option<Breakpoint>,
  pub(super) pseudo_element: Option<PseudoElement>,
  pseudo_classes: String,
  /// A bit per variant rank, so comparing two sorts them as Tailwind's variant bit sets do.
  ranks: u32,
}

impl Variants {
  /// Adds `variant`, or `None` for a second breakpoint or pseudo-element.
  pub(super) fn push(&mut self, variant: Variant) -> Option<()> {
    match variant {
      Variant::Breakpoint(breakpoint) => {
        if self.breakpoint.replace(breakpoint).is_some() {
          return None;
        }
      }
      Variant::Structural { pseudo_class, rank } => {
        self.pseudo_classes.push_str(&pseudo_class);
        self.ranks |= 1 << rank;
      }
      Variant::PseudoElement(pseudo_element) => {
        let rank = match pseudo_element {
          PseudoElement::Before => BEFORE_RANK,
          _ => AFTER_RANK,
        };

        if self.pseudo_element.replace(pseudo_element).is_some() {
          return None;
        }
        self.ranks |= 1 << rank;
      }
    }

    Some(())
  }

  /// The selector the pseudo-classes form on the element, `Some(None)` without any, or `None`
  /// when they do not parse or reach past the element.
  pub(super) fn condition(&self) -> Option<Option<Selector<SelectorImpl>>> {
    if self.pseudo_classes.is_empty() {
      return Some(None);
    }

    condition(&self.pseudo_classes).map(Some)
  }

  /// The variants' sort key: the breakpoint dominates, as Tailwind registers breakpoints after
  /// every structural variant, then the variant bits.
  pub(super) fn order(&self) -> VariantOrder {
    VariantOrder {
      breakpoint: self.breakpoint.as_ref().map(Breakpoint::order),
      ranks: self.ranks,
    }
  }
}

/// Tailwind's variant order for one utility.
#[derive(Debug, Clone, Default, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct VariantOrder {
  breakpoint: Option<(u32, Arc<str>)>,
  ranks: u32,
}

/// The pseudo-class a structural variant compiles to, or `None` for any other variant.
fn pseudo_class(variant: &str) -> Option<String> {
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

/// An arbitrary value, which Tailwind writes with underscores for spaces and `\_` for an
/// underscore.
fn arbitrary(value: &str) -> String {
  let mut decoded = String::with_capacity(value.len());
  let mut chars = value.chars();

  while let Some(character) = chars.next() {
    match character {
      '\\' if chars.as_str().starts_with('_') => {
        decoded.push('_');
        chars.next();
      }
      '_' => decoded.push(' '),
      character => decoded.push(character),
    }
  }

  decoded
}

/// The selector `pseudo_classes` form on the element itself, or `None` when they do not parse or
/// reach past it through a combinator.
fn condition(pseudo_classes: &str) -> Option<Selector<SelectorImpl>> {
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
  use super::{Variant, Variants, condition, pseudo_class};
  use crate::style::selector::PseudoElement;

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
      (
        "[&:is([data-state=foo\\_bar])]",
        Some(":is([data-state=foo_bar])"),
      ),
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

  #[test]
  fn variants_parse_by_kind() {
    assert_eq!(
      Variant::parse("before"),
      Some(Variant::PseudoElement(PseudoElement::Before))
    );
    assert!(matches!(
      Variant::parse("first"),
      Some(Variant::Structural { .. })
    ));
    assert!(matches!(Variant::parse("md"), Some(Variant::Breakpoint(_))));
  }

  #[test]
  fn a_utility_takes_one_breakpoint_and_one_pseudo_element() {
    let mut variants = Variants::default();

    assert!(variants.push(Variant::parse("md").unwrap()).is_some());
    assert!(variants.push(Variant::parse("lg").unwrap()).is_none());

    let mut variants = Variants::default();

    assert!(variants.push(Variant::parse("before").unwrap()).is_some());
    assert!(variants.push(Variant::parse("after").unwrap()).is_none());
  }

  #[test]
  fn variants_order_as_tailwind_registers_them() {
    let ordered = [
      "not-first",
      "before",
      "after",
      "first",
      "odd",
      "empty",
      "nth-3",
      "nth-last-of-type-2",
      "[&:nth-child(4)]",
      "sm",
      "md",
    ];
    let order = |token: &str| {
      let mut variants = Variants::default();

      variants.push(Variant::parse(token).unwrap()).unwrap();
      variants.order()
    };

    for pair in ordered.windows(2) {
      assert!(order(pair[0]) < order(pair[1]), "{pair:?}");
    }
  }
}
