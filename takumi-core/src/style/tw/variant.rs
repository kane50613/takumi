//! Tailwind's variants, such as `md:`, `first:`, and `before:`: the conditions a utility applies
//! under, and where they sort it.

use std::{cmp::Ordering, collections::HashMap, sync::Arc};

use cssparser::Parser;
use selectors::parser::{ParseRelative, Selector, SelectorList};

use super::{extract_arbitrary_value, is_ident};
use crate::{
  style::{
    FromCssStr, Length, ToCss,
    media_query::{ContainerQuery, MediaQueryList},
    selector::{PseudoElement, SelectorImpl, TakumiSelectorParser},
  },
  viewport::Viewport,
};

/// Widths a stylesheet's `--breakpoint-*` variables assign, keyed by token.
pub(crate) type BreakpointOverrides = HashMap<String, Length>;

/// A breakpoint variant, `md:`, `min-md:`, or `max-[600px]:`, which applies from a viewport width
/// on, or below one.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Breakpoint {
  /// Built-in width; `None` for a token only a `--breakpoint-*` variable defines.
  pub width: Option<Length>,
  /// The variant token, which a `--breakpoint-<token>` variable re-sizes.
  pub token: Arc<str>,
  /// Whether the variant applies below its width, as `max-*` does.
  below: bool,
  /// Whether a `not-` variant flipped `below`, so the breakpoint sorts as Tailwind's `not` does.
  negated: bool,
}

impl Breakpoint {
  /// Parse a breakpoint from a token, `max-` and `min-` prefixed or not.
  pub fn parse(token: &str) -> Option<Self> {
    match token.strip_prefix("max-") {
      Some(value) => Self::with_width(value, true),
      None => Self::with_width(token.strip_prefix("min-").unwrap_or(token), false),
    }
  }

  fn with_width(value: &str, below: bool) -> Option<Self> {
    if let Some(arbitrary) = extract_arbitrary_value(value) {
      let width = Length::from_css_str(&arbitrary).ok().filter(|width| {
        matches!(
          width,
          Length::Rem(_) | Length::Em(_) | Length::Px(_) | Length::Vw(_)
        )
      })?;

      return Some(Breakpoint {
        width: Some(width),
        token: value.into(),
        below,
        negated: false,
      });
    }

    let width = match value {
      "sm" => Some(Length::Rem(40.0)),
      "md" => Some(Length::Rem(48.0)),
      "lg" => Some(Length::Rem(64.0)),
      "xl" => Some(Length::Rem(80.0)),
      "2xl" => Some(Length::Rem(96.0)),
      _ => None,
    };

    if width.is_none() && !is_ident(value) {
      return None;
    }

    Some(Breakpoint {
      width,
      token: value.into(),
      below,
      negated: false,
    })
  }

  /// The breakpoint `not-` makes of this one: Tailwind's `@media not (width >= …)` holds exactly
  /// where the opposite `max-*` or `min-*` breakpoint does.
  fn negated(self) -> Self {
    Self {
      below: !self.below,
      negated: !self.negated,
      ..self
    }
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
      Length::Rem(value) | Length::Em(value) => viewport.to_device(value * viewport.font_size),
      Length::Px(value) => viewport.to_device(value),
      Length::Vw(value) => (value / 100.0) * viewport_width as f32,
      _ => return false,
    };

    if self.below {
      return (viewport_width as f32) < breakpoint_width;
    }

    viewport_width as f32 >= breakpoint_width
  }

  /// Where the breakpoint sorts among others: `max-*` first, then the rest, each by its built-in
  /// width as Tailwind's `compareBreakpoints` compares it, then its token. Approximate: Tailwind
  /// compares the widths a theme gives.
  fn order(&self) -> BreakpointOrder {
    let at_least = !self.below;
    let width = self
      .width
      .map(|width| width.to_css_string().into())
      .unwrap_or_default();

    (WidthOrder::new(at_least, width), Arc::clone(&self.token))
  }
}

/// A breakpoint's sort key, from `Breakpoint::order`.
type BreakpointOrder = (WidthOrder, Arc<str>);

/// A container query variant, `@md:` or `@max-[400px]/sidebar:`, which applies while the nearest
/// inline-size container, or the one it names, meets a width. Approximate: a named width is
/// Tailwind's default, which a stylesheet's `--container-*` does not re-size.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ContainerVariant {
  pub(crate) query: ContainerQuery,
  order: WidthOrder,
}

/// Where a breakpoint or container variant sorts: `max-*` first, then the rest, each by its width
/// as Tailwind's `compareBreakpoints` compares them.
#[derive(Debug, Clone, PartialEq, Eq)]
struct WidthOrder {
  at_least: bool,
  width: Box<str>,
}

impl WidthOrder {
  fn new(at_least: bool, width: Box<str>) -> Self {
    Self { at_least, width }
  }
}

impl Ord for WidthOrder {
  fn cmp(&self, other: &Self) -> Ordering {
    self
      .at_least
      .cmp(&other.at_least)
      .then_with(|| compare_breakpoints(&self.width, &other.width, self.at_least))
  }
}

impl PartialOrd for WidthOrder {
  fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
    Some(self.cmp(other))
  }
}

/// Tailwind's `compareBreakpoints`: by unit, or by function name for a CSS function, then by the
/// number the width starts with, then by text when no number decides.
fn compare_breakpoints(a: &str, z: &str, ascending: bool) -> Ordering {
  if a == z {
    return Ordering::Equal;
  }

  let bucket = |width: &str| match width.find('(') {
    Some(function) => width[..function].to_owned(),
    None => width
      .chars()
      .filter(|character| !character.is_ascii_digit() && *character != '.')
      .collect(),
  };

  bucket(a)
    .cmp(&bucket(z))
    .then_with(|| match leading_number(a).partial_cmp(&leading_number(z)) {
      Some(order) if ascending => order,
      Some(order) => order.reverse(),
      None => a.cmp(z),
    })
}

/// JavaScript's `parseFloat`: the number `text` starts with, `NaN` without one.
fn leading_number(text: &str) -> f64 {
  let end = text
    .char_indices()
    .find(|&(index, character)| {
      !(character.is_ascii_digit()
        || character == '.'
        || (index == 0 && matches!(character, '-' | '+')))
    })
    .map_or(text.len(), |(index, _)| index);

  text[..end].parse().unwrap_or(f64::NAN)
}

impl ContainerVariant {
  /// The container variant `token` names, `@` then an optional `min-` or `max-`, a width, and an
  /// optional `/name`.
  fn parse(token: &str) -> Option<Self> {
    let rest = token.strip_prefix('@')?;
    let (rest, name) = match split_top_level(rest, b'/') {
      Some((_, name)) if split_top_level(name, b'/').is_some() => return None,
      Some(split) => split,
      None => (rest, ""),
    };
    let (at_least, value) = match rest.strip_prefix("max-") {
      Some(value) => (false, value),
      None => (true, rest.strip_prefix("min-").unwrap_or(rest)),
    };
    let width = match extract_arbitrary_value(value) {
      Some(arbitrary) if arbitrary.contains("var(") => return None,
      Some(arbitrary) => arbitrary.into_owned(),
      None => Length::container_width(value)?.to_css_string(),
    };
    let operator = if at_least { ">=" } else { "<" };
    let text = format!("{name} (width {operator} {width})");
    let query = ContainerQuery::parse(&mut Parser::new(&text)).ok()?;

    Some(Self {
      query,
      order: WidthOrder::new(at_least, width.into()),
    })
  }
}

/// A variant Tailwind registers after its container queries: a media query the viewport meets,
/// or a selector the element matches.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum LateCondition {
  Media(&'static str),
  PseudoClass(&'static str),
}

/// The late variants in the order Tailwind registers them.
const LATE: [(&str, LateCondition); 8] = [
  ("portrait", LateCondition::Media("(orientation: portrait)")),
  (
    "landscape",
    LateCondition::Media("(orientation: landscape)"),
  ),
  (
    "ltr",
    LateCondition::PseudoClass(r#":where(:dir(ltr), [dir="ltr"], [dir="ltr"] *)"#),
  ),
  (
    "rtl",
    LateCondition::PseudoClass(r#":where(:dir(rtl), [dir="rtl"], [dir="rtl"] *)"#),
  ),
  ("dark", LateCondition::Media("(prefers-color-scheme: dark)")),
  ("print", LateCondition::Media("print")),
  (
    "forced-colors",
    LateCondition::Media("(forced-colors: active)"),
  ),
  (
    "inverted-colors",
    LateCondition::Media("(inverted-colors: inverted)"),
  ),
];

/// The preference variants in the order Tailwind registers them, ahead of its breakpoints.
const PREFERENCES: [(&str, &str); 4] = [
  ("motion-safe", "(prefers-reduced-motion: no-preference)"),
  ("motion-reduce", "(prefers-reduced-motion: reduce)"),
  ("contrast-more", "(prefers-contrast: more)"),
  ("contrast-less", "(prefers-contrast: less)"),
];

/// The states of an element no static render is in, whose `not-` variants always apply.
const UNREACHED_STATES: [&str; 8] = [
  "autofill",
  "focus-within",
  "hover",
  "focus",
  "focus-visible",
  "active",
  "visited",
  "target",
];

/// One variant of a utility, as Tailwind's `parseVariant` reads it.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Variant {
  Breakpoint(Breakpoint),
  Container(ContainerVariant),
  Late {
    condition: LateCondition,
    rank: u8,
  },
  /// A preference media query, `motion-safe:` or `contrast-more:`.
  Preference {
    query: &'static str,
    rank: u8,
  },
  /// A media query variant under `not-`, `not-dark:` or `not-print:`, which Tailwind's `not`
  /// compiles to `@media not …`.
  NotMedia(String),
  /// A pseudo-class on the element itself, such as `:first-child` for `first:`.
  Structural {
    pseudo_class: String,
    rank: u8,
  },
  /// An arbitrary variant, `[&:nth-child(4)]`, which sorts after every registered one.
  Arbitrary(String),
  PseudoElement(PseudoElement),
}

/// Where Tailwind registers `not-*`, ahead of every other variant this rank orders.
const NOT_RANK: u8 = 0;
/// Where Tailwind registers `before:` and `after:`, after `not-*` and ahead of every structural
/// variant.
const BEFORE_RANK: u8 = 1;
const AFTER_RANK: u8 = 2;

/// The structural and form state variants in the order Tailwind registers them, from
/// `STRUCTURAL_RANK` on, each with the pseudo-class it compiles to. Approximate: `default`,
/// `valid`, `invalid`, and the other states takumi's selectors never match are left out.
const REGISTERED: [(&str, &str); 17] = [
  ("first", ":first-child"),
  ("last", ":last-child"),
  ("only", ":only-child"),
  ("odd", ":nth-child(odd)"),
  ("even", ":nth-child(even)"),
  ("first-of-type", ":first-of-type"),
  ("last-of-type", ":last-of-type"),
  ("only-of-type", ":only-of-type"),
  ("checked", ":checked"),
  ("indeterminate", ":indeterminate"),
  ("placeholder-shown", ":placeholder-shown"),
  ("optional", ":optional"),
  ("required", ":required"),
  ("read-only", ":read-only"),
  ("empty", ":empty"),
  ("enabled", ":enabled"),
  ("disabled", ":disabled"),
];
const STRUCTURAL_RANK: u8 = 3;
const FUNCTIONAL: [&str; 6] = [
  "aria-",
  "data-",
  "nth-",
  "nth-last-",
  "nth-of-type-",
  "nth-last-of-type-",
];

impl Variant {
  /// The variant `token` names, or `None` for one takumi does not support.
  pub(super) fn parse(token: &str) -> Option<Self> {
    match token {
      "before" => return Some(Self::PseudoElement(PseudoElement::Before)),
      "after" => return Some(Self::PseudoElement(PseudoElement::After)),
      _ => {}
    }

    if let Some(rank) = LATE.iter().position(|(name, _)| *name == token) {
      return Some(Self::Late {
        condition: LATE[rank].1,
        rank: rank as u8,
      });
    }

    if let Some(rank) = PREFERENCES.iter().position(|(name, _)| *name == token) {
      return Some(Self::Preference {
        query: PREFERENCES[rank].1,
        rank: rank as u8,
      });
    }

    if token.starts_with('@') {
      return ContainerVariant::parse(token).map(Self::Container);
    }

    if let Some(compound) = arbitrary_compound(token) {
      return Some(Self::Arbitrary(compound));
    }

    if let Some(pseudo_class) = pseudo_class(token) {
      return Some(Self::Structural {
        rank: structural_rank(token),
        pseudo_class,
      });
    }

    let (base, negated) = strip_negations(token);

    if base.len() != token.len() {
      let variant = Self::parse(base)?;

      return if negated {
        variant.negated()
      } else {
        Some(variant)
      };
    }

    Breakpoint::parse(token).map(Self::Breakpoint)
  }

  /// The variant `not-` makes of this at-rule variant, or `None` for one Tailwind's `not` does
  /// not negate here. Approximate: Tailwind also negates a container query and `ltr:` and `rtl:`.
  fn negated(self) -> Option<Self> {
    match self {
      Self::Breakpoint(breakpoint) => Some(Self::Breakpoint(breakpoint.negated())),
      Self::Late {
        condition: LateCondition::Media(query),
        ..
      }
      | Self::Preference { query, .. } => Some(Self::NotMedia(format!("not {query}"))),
      _ => None,
    }
  }
}

/// Where a structural variant sorts. Approximate: values of one variant keep their order.
fn structural_rank(token: &str) -> u8 {
  if token.starts_with("not-") {
    return NOT_RANK;
  }

  if let Some(index) = REGISTERED.iter().position(|(name, _)| *name == token) {
    return STRUCTURAL_RANK + index as u8;
  }

  let functional = FUNCTIONAL
    .iter()
    .rposition(|prefix| token.starts_with(prefix))
    .unwrap_or(FUNCTIONAL.len());

  STRUCTURAL_RANK + REGISTERED.len() as u8 + functional as u8
}

/// The variants of one utility: its breakpoints, at most one pseudo-element, and the
/// pseudo-classes they put on the element.
#[derive(Debug, Default)]
pub(super) struct Variants {
  /// The breakpoints, every one of which must hold.
  pub(super) breakpoint: Vec<Breakpoint>,
  /// The container queries `@` variants ask, every one of which must hold.
  pub(super) container: Vec<ContainerVariant>,
  /// The media queries the late and preference variants require of the viewport.
  pub(super) media: Vec<MediaQueryList>,
  pub(super) pseudo_element: Option<PseudoElement>,
  pseudo_classes: String,
  /// The compounds of the arbitrary variants.
  arbitrary: Vec<String>,
  /// A bit per variant rank, so comparing two sorts them as Tailwind's variant bit sets do.
  ranks: u32,
  /// A bit per late variant, which outranks every other variant.
  late: u8,
  /// A bit per preference variant, which outranks the structural ones but no breakpoint.
  preference: u8,
}

impl Variants {
  /// The variants a token opens with and the utility after them, or `None` when a variant is
  /// unknown or repeats a pseudo-element.
  pub(super) fn parse(mut token: &str) -> Option<(Self, &str)> {
    let mut variants = Self::default();

    while let Some((variant, rest)) = split_variant(token) {
      variants.push(Variant::parse(variant)?)?;
      token = rest;
    }

    Some((variants, token))
  }

  /// Adds `variant`, or `None` for a second pseudo-element.
  pub(super) fn push(&mut self, variant: Variant) -> Option<()> {
    match variant {
      Variant::Breakpoint(breakpoint) => {
        if breakpoint.negated {
          self.ranks |= 1 << NOT_RANK;
        }
        self.breakpoint.push(breakpoint);
      }
      Variant::Container(container) => self.container.push(container),
      Variant::Late { condition, rank } => {
        match condition {
          LateCondition::Media(query) => self
            .media
            .push(MediaQueryList::parse(&mut Parser::new(query)).ok()?),
          LateCondition::PseudoClass(pseudo_class) => self.pseudo_classes.push_str(pseudo_class),
        }
        self.late |= 1 << rank;
      }
      Variant::Preference { query, rank } => {
        self
          .media
          .push(MediaQueryList::parse(&mut Parser::new(query)).ok()?);
        self.preference |= 1 << rank;
      }
      Variant::NotMedia(query) => {
        self
          .media
          .push(MediaQueryList::parse(&mut Parser::new(&query)).ok()?);
        self.ranks |= 1 << NOT_RANK;
      }
      Variant::Structural { pseudo_class, rank } => {
        self.pseudo_classes.push_str(&pseudo_class);
        self.ranks |= 1 << rank;
      }
      Variant::Arbitrary(compound) => {
        self.pseudo_classes.push_str(&compound);
        self.arbitrary.push(compound);
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

  /// The variants' sort key, the latest Tailwind registers first: the arbitrary variants, the late variants, a container
  /// query, a breakpoint, the preference variants, then the structural variant bits.
  pub(super) fn order(&self) -> VariantOrder {
    VariantOrder {
      arbitrary: descending(self.arbitrary.iter().cloned()),
      late: self.late,
      container: descending(
        self
          .container
          .iter()
          .map(|container| container.order.clone()),
      ),
      breakpoint: descending(
        self
          .breakpoint
          .iter()
          .filter(|breakpoint| !breakpoint.negated)
          .map(Breakpoint::order),
      ),
      preference: self.preference,
      ranks: self.ranks,
    }
  }
}

/// `orders`, the greatest first.
fn descending<T: Ord>(orders: impl Iterator<Item = T>) -> Vec<T> {
  let mut orders: Vec<T> = orders.collect();

  orders.sort_unstable_by(|left, right| right.cmp(left));
  orders
}

/// Tailwind's variant order for one utility.
#[derive(Debug, Clone, Default, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct VariantOrder {
  /// Approximate: Tailwind gives each arbitrary variant a bit by its selector's text, where this
  /// compares the compound's text without the `&`.
  arbitrary: Vec<String>,
  late: u8,
  /// Approximate: several container variants sort by their latest first, where Tailwind's variant
  /// bit set gives each group one bit.
  container: Vec<WidthOrder>,
  breakpoint: Vec<BreakpointOrder>,
  preference: u8,
  ranks: u32,
}

/// The variant under a run of `not-` prefixes, and whether the run negates it: Tailwind's `not`
/// compounds with itself, and each pair cancels out.
fn strip_negations(token: &str) -> (&str, bool) {
  let base = token.trim_start_matches("not-");
  let negations = (token.len() - base.len()) / "not-".len();

  (base, negations % 2 == 1)
}

/// The selector a structural, form state, or attribute variant compiles to, under any `not-`
/// prefixes, or `None` for any other variant.
fn pseudo_class(variant: &str) -> Option<String> {
  match strip_negations(variant) {
    (base, true) if UNREACHED_STATES.contains(&base) => Some(format!(":not(:{base})")),
    (base, true) => Some(format!(":not({})", element_pseudo_class(base)?)),
    (base, false) => element_pseudo_class(base),
  }
}

/// The selector a structural, form state, or attribute variant without `not-` compiles to.
fn element_pseudo_class(variant: &str) -> Option<String> {
  if let Some((_, pseudo_class)) = REGISTERED.iter().find(|(name, _)| *name == variant) {
    return Some((*pseudo_class).to_owned());
  }

  if let Some(compound) = arbitrary_compound(variant) {
    return Some(compound);
  }

  if let Some(value) = variant.strip_prefix("aria-") {
    return attribute_selector("aria", value, "=\"true\"");
  }

  if let Some(value) = variant.strip_prefix("data-") {
    return attribute_selector("data", value, "");
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
      None => argument
        .parse::<u32>()
        .ok()
        .filter(|number| number.to_string() == argument)?
        .to_string(),
    };

    return Some(format!(":{function}({argument})"));
  }

  None
}

/// The attribute selector Tailwind's `aria-*` and `data-*` variants write: `[aria-checked="true"]`
/// for a name, which `named_value` follows, and `[data-state="open"]` for `data-[state=open]`.
fn attribute_selector(prefix: &str, value: &str, named_value: &str) -> Option<String> {
  match value
    .strip_prefix('[')
    .and_then(|rest| rest.strip_suffix(']'))
  {
    Some(bracketed) => Some(format!(
      "[{prefix}-{}]",
      quote_attribute_value(&arbitrary(bracketed))
    )),
    None => is_ident(value).then(|| format!("[{prefix}-{value}{named_value}]")),
  }
}

/// Tailwind's `quoteAttributeValue`: an attribute's unquoted value quoted, keeping a trailing `i`
/// or `s` flag outside the quotes.
fn quote_attribute_value(input: &str) -> String {
  let Some((attribute, value)) = input.split_once('=') else {
    return input.to_owned();
  };
  let value = value.trim();

  if value.starts_with(['\'', '"']) {
    return input.to_owned();
  }

  if let [.., b' ', flag @ (b'i' | b'I' | b's' | b'S')] = value.as_bytes() {
    let unflagged = &value[..value.len() - 2];

    return format!("{attribute}=\"{unflagged}\" {}", *flag as char);
  }

  format!("{attribute}=\"{value}\"")
}

/// The compound an arbitrary variant such as `[&:nth-child(3)]` or `[&.active]` adds to the
/// element itself. Approximate: one that reaches past the element, `[&_p]`, and an at-rule one,
/// `[@media(…)]`, are unsupported.
fn arbitrary_compound(variant: &str) -> Option<String> {
  let compound = variant
    .strip_prefix("[&")
    .and_then(|rest| rest.strip_suffix(']'))?;

  compound
    .starts_with([':', '.', '#', '['])
    .then(|| arbitrary(compound))
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

/// Splits a token at the first top-level variant `:`.
fn split_variant(token: &str) -> Option<(&str, &str)> {
  split_top_level(token, b':')
}

/// Splits `token` at the first top-level `separator`, mirroring Tailwind's `segment`: one nested
/// in brackets, quotes, or escaped by `\` is not a separator, so `url('https://…')` stays intact.
fn split_top_level(token: &str, separator: u8) -> Option<(&str, &str)> {
  let bytes = token.as_bytes();
  let mut stack: Vec<u8> = Vec::new();
  let mut index = 0;
  while index < bytes.len() {
    match bytes[index] {
      b'\\' => index += 1, // skip the escaped character
      quote @ (b'\'' | b'"') => {
        index += 1;
        while index < bytes.len() && bytes[index] != quote {
          index += if bytes[index] == b'\\' { 2 } else { 1 };
        }
      }
      b'(' => stack.push(b')'),
      b'[' => stack.push(b']'),
      b'{' => stack.push(b'}'),
      closing @ (b')' | b']' | b'}') => {
        if stack.last() == Some(&closing) {
          stack.pop();
        }
      }
      byte if byte == separator && stack.is_empty() => {
        return Some((&token[..index], &token[index + 1..]));
      }
      _ => {}
    }
    index += 1;
  }
  None
}

#[cfg(test)]
mod tests {
  use super::{Breakpoint, Variant, Variants, condition, pseudo_class, split_variant};
  use crate::{style::selector::PseudoElement, viewport::Viewport};

  #[test]
  fn structural_variants_compile_to_pseudo_classes() {
    for (variant, expected) in [
      ("first", Some(":first-child")),
      ("odd", Some(":nth-child(odd)")),
      ("nth-3", Some(":nth-child(3)")),
      ("nth-last-of-type-2", Some(":nth-last-of-type(2)")),
      ("nth-[2n_+_1]", Some(":nth-child(2n + 1)")),
      ("not-first", Some(":not(:first-child)")),
      ("not-hover", Some(":not(:hover)")),
      ("not-not-first", Some(":first-child")),
      ("not-not-not-first", Some(":not(:first-child)")),
      ("not-focus-within", Some(":not(:focus-within)")),
      ("not-enabled", Some(":not(:enabled)")),
      ("disabled", Some(":disabled")),
      ("checked", Some(":checked")),
      ("indeterminate", Some(":indeterminate")),
      ("placeholder-shown", Some(":placeholder-shown")),
      ("read-only", Some(":read-only")),
      ("required", Some(":required")),
      ("not-optional", Some(":not(:optional)")),
      ("not-autofill", Some(":not(:autofill)")),
      ("autofill", None),
      ("valid", None),
      ("default", None),
      ("read-write", None),
      ("aria-checked", Some("[aria-checked=\"true\"]")),
      ("aria-[sort=ascending]", Some("[aria-sort=\"ascending\"]")),
      ("not-aria-busy", Some(":not([aria-busy=\"true\"])")),
      ("data-active", Some("[data-active]")),
      ("data-[state=open]", Some("[data-state=\"open\"]")),
      ("data-[state='open']", Some("[data-state='open']")),
      ("data-[lang=EN_i]", Some("[data-lang=\"EN\" i]")),
      ("data-[size]", Some("[data-size]")),
      ("data-a.b", None),
      ("[&:nth-child(4)]", Some(":nth-child(4)")),
      ("[&.active]", Some(".active")),
      ("[&[data-x]]", Some("[data-x]")),
      ("[&_p]", None),
      (
        "[&:is([data-state=foo\\_bar])]",
        Some(":is([data-state=foo_bar])"),
      ),
      ("nth-x", None),
      ("nth-007", None),
      ("nth-last-of-type-01", None),
      ("hover", None),
      ("md", None),
    ] {
      assert_eq!(pseudo_class(variant).as_deref(), expected, "{variant}");
    }
  }

  #[test]
  fn variant_names_and_theme_keys_are_case_sensitive() {
    assert_eq!(
      Breakpoint::parse("SM").map(|breakpoint| breakpoint.width),
      Some(None)
    );
    assert!(Variant::parse("@MD").is_none());
    assert!(Variant::parse("@max-LG").is_none());
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
  fn a_utility_takes_several_breakpoints_and_one_pseudo_element() {
    let mut variants = Variants::default();

    assert!(variants.push(Variant::parse("md").unwrap()).is_some());
    assert!(variants.push(Variant::parse("max-lg").unwrap()).is_some());

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
      "only-of-type",
      "checked",
      "read-only",
      "empty",
      "enabled",
      "disabled",
      "aria-checked",
      "data-active",
      "nth-3",
      "nth-last-of-type-2",
      "motion-safe",
      "motion-reduce",
      "contrast-more",
      "contrast-less",
      "sm",
      "md",
      "@max-[100px]",
      "@max-lg",
      "@max-sm",
      "@[10em]",
      "@[400px]",
      "@sm",
      "@min-md",
      "portrait",
      "ltr",
      "rtl",
      "dark",
      "print",
      "forced-colors",
      "inverted-colors",
      "[&:nth-child(4)]",
      "[&:nth-child(5)]",
    ];
    let order = |token: &str| {
      let mut variants = Variants::default();

      variants.push(Variant::parse(token).unwrap()).unwrap();
      variants.order()
    };

    for pair in ordered.windows(2) {
      assert!(order(pair[0]) < order(pair[1]), "{pair:?}");
    }
    assert_eq!(order("@md/sidebar"), order("@min-md"));
  }

  #[test]
  fn breakpoints_order_max_widest_first_then_min_narrowest_first() {
    let order = |token: &str| {
      let mut variants = Variants::default();

      variants.push(Variant::parse(token).unwrap()).unwrap();
      variants.order()
    };

    for pair in [
      "max-[600px]",
      "max-lg",
      "max-md",
      "min-[900px]",
      "min-[1000px]",
      "sm",
      "min-md",
      "lg",
    ]
    .windows(2)
    {
      assert!(order(pair[0]) < order(pair[1]), "{pair:?}");
    }
    assert_eq!(order("min-md"), order("md"));
  }

  #[test]
  fn min_and_max_never_both_match() {
    let viewport = Viewport::new((600, 400));
    let matches = |token: &str| {
      Breakpoint::parse(token)
        .unwrap()
        .matches(viewport, &Default::default())
    };

    assert!(!matches("min-[600.5px]"));
    assert!(matches("max-[600.5px]"));
  }

  #[test]
  fn test_split_variant_ignores_colons_in_arbitrary_values() {
    assert_eq!(split_variant("sm:mt-0"), Some(("sm", "mt-0")));
    assert_eq!(split_variant("sm:hover:mt-0"), Some(("sm", "hover:mt-0")));
    assert_eq!(split_variant("mt-0"), None);
    assert_eq!(
      split_variant("mask-[url('https://example.com/a.svg')]"),
      None
    );
    assert_eq!(split_variant("grid-cols-[repeat(2,minmax(0,1fr))]"), None);
    assert_eq!(split_variant("[mask:foo]"), None);
    assert_eq!(
      split_variant("md:mask-[url('https://example.com/a.svg')]"),
      Some(("md", "mask-[url('https://example.com/a.svg')]"))
    );
    assert_eq!(split_variant(r#"content-['a:b]']"#), None);
    assert_eq!(split_variant(r#"content-["a\":b"]"#), None);
  }
}
