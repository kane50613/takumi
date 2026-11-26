//! A utility compiled as Tailwind compiles it: one rule of the `tw` layer, which the cascade
//! orders among the others as Tailwind's [`compileCandidates`](https://github.com/tailwindlabs/tailwindcss/blob/main/packages/tailwindcss/src/compile.ts)
//! sorts them.

use crate::sort_key::sort_by_key;
use std::cmp::{Ordering, Reverse};

use selectors::parser::Selector;
use smallvec::SmallVec;
use xxhash_rust::xxh3::xxh3_64;

use super::{
  Breakpoint, BreakpointOverrides,
  builder::TailwindDeclarationBuilder,
  property_order::{property_index, property_indices},
  utility,
  variant::{VariantOrder, Variants},
};
use crate::{
  style::{
    ContentItem, ContentValue, KeyframesRule, PropertyMask, StyleDeclaration,
    StyleDeclarationBlock,
    media_query::{ContainerQuery, MediaQueryList},
    selector::{PseudoElement, SelectorImpl},
  },
  viewport::Viewport,
};

/// Tailwind's sort key for a utility. Approximate: it ignores `--tw-sort`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct TwOrder {
  variants: VariantOrder,
  /// The properties' indices, ascending, and then `u16::MAX`, so a list that runs out sorts after
  /// a longer one sharing its start, as Tailwind reads a missing index as infinite.
  properties: SmallVec<[u16; 4]>,
  count: Reverse<usize>,
  name: NaturalName,
}

impl TwOrder {
  fn new(name: &str, declarations: &StyleDeclarationBlock, variants: VariantOrder) -> Self {
    let mut properties = SmallVec::<[u16; 4]>::new();
    let mut longhands = PropertyMask::default();

    for declaration in declarations.iter() {
      match declaration {
        StyleDeclaration::CustomProperty(name, _) => properties.extend(property_index(name)),
        _ => longhands.union(&declaration.affected_longhands()),
      }
    }

    properties.extend(property_indices(longhands));
    sort_by_key(&mut properties, |&property| property);
    properties.dedup();
    properties.push(u16::MAX);

    Self {
      variants,
      properties,
      count: Reverse(declarations.iter().count()),
      name: NaturalName(name.into()),
    }
  }
}

/// A utility's name, ordered as Tailwind's [`compare`](https://github.com/tailwindlabs/tailwindcss/blob/main/packages/tailwindcss/src/utils/compare.ts)
/// does, with digit runs by value.
#[derive(Debug, Clone, PartialEq, Eq)]
struct NaturalName(Box<str>);

impl Ord for NaturalName {
  fn cmp(&self, other: &Self) -> Ordering {
    let (mut left, mut right) = (self.0.as_bytes(), other.0.as_bytes());

    while let (Some(&a), Some(&z)) = (left.first(), right.first()) {
      if a.is_ascii_digit() && z.is_ascii_digit() {
        let (a_run, a_rest) = split_digits(left);
        let (z_run, z_rest) = split_digits(right);
        let order = compare_digits(a_run, z_run);

        if order != Ordering::Equal {
          return order;
        }

        (left, right) = (a_rest, z_rest);
      } else if a == z {
        (left, right) = (&left[1..], &right[1..]);
      } else {
        return a.cmp(&z);
      }
    }

    left.len().cmp(&right.len())
  }
}

/// Digit runs by value, then by text when only leading zeros differ.
fn compare_digits(a: &[u8], z: &[u8]) -> Ordering {
  let value = |run: &[u8]| {
    run
      .iter()
      .position(|&byte| byte != b'0')
      .map_or(0, |zeros| run.len() - zeros)
  };
  let (a_value, z_value) = (&a[a.len() - value(a)..], &z[z.len() - value(z)..]);

  a_value
    .len()
    .cmp(&z_value.len())
    .then_with(|| a_value.cmp(z_value))
    .then_with(|| a.cmp(z))
}

/// The leading digits of `bytes` and what follows them.
fn split_digits(bytes: &[u8]) -> (&[u8], &[u8]) {
  bytes.split_at(
    bytes
      .iter()
      .take_while(|byte| byte.is_ascii_digit())
      .count(),
  )
}

impl PartialOrd for NaturalName {
  fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
    Some(self.cmp(other))
  }
}

/// One utility's declarations, all `!important` or none, applying to the box or pseudo-element
/// its variants name while they hold.
#[derive(Debug, PartialEq)]
pub(crate) struct TwRule {
  pub(crate) declarations: StyleDeclarationBlock,
  pub(crate) important: bool,
  pub(crate) breakpoint: Vec<Breakpoint>,
  /// The container queries `@` variants ask of the element's containers.
  pub(crate) container: Vec<ContainerQuery>,
  /// The media queries the late variants require of the viewport.
  pub(crate) media: Vec<MediaQueryList>,
  /// The pseudo-classes the utility's structural and direction variants require of the element.
  pub(crate) condition: Option<Selector<SelectorImpl>>,
  pub(crate) pseudo_element: Option<PseudoElement>,
  /// A class's specificity with its variants', as Tailwind's selector for the utility has.
  pub(crate) specificity: u32,
  pub(crate) order: TwOrder,
  /// A digest of the utility's token, which names the rule among the ones a box matched.
  pub(crate) id: u64,
  /// The theme `@keyframes` Tailwind emits once the utility is used.
  pub(crate) theme_keyframes: Option<&'static KeyframesRule>,
}

impl TwRule {
  /// The rule a class compiles to: its variants, then the utility, `!` marking it important at
  /// either end, as Tailwind's `parseCandidate` reads one.
  pub(super) fn parse(source: &str) -> Option<Self> {
    let (variants, token) = Variants::parse(source)?;
    let (token, important) = match token.strip_prefix('!').or_else(|| token.strip_suffix('!')) {
      Some(stripped) => (stripped, true),
      None => (token, false),
    };
    let mut builder = TailwindDeclarationBuilder::with_capacity(1);

    // Tailwind's `before:` and `after:` open each rule with `content: var(--tw-content)`, which
    // `@property` starts as the empty string, so the box exists without a `content-*` utility.
    if variants.pseudo_element.is_some() {
      builder.push(StyleDeclaration::content(ContentValue::Items(
        [ContentItem::Text("".into())].into(),
      )));
    }

    utility::emit(token, &mut builder)?;

    let theme_keyframes = builder.theme_keyframes;

    Self::new(
      source,
      builder.finish(),
      important,
      variants,
      theme_keyframes,
    )
  }

  /// The rule `token` compiles to, or `None` when its variants form no selector.
  fn new(
    token: &str,
    declarations: StyleDeclarationBlock,
    important: bool,
    variants: Variants,
    theme_keyframes: Option<&'static KeyframesRule>,
  ) -> Option<Self> {
    const CLASS_SPECIFICITY: u32 = 1 << 10;

    let condition = variants.condition()?;
    let specificity = CLASS_SPECIFICITY
      + condition.as_ref().map_or(0, Selector::specificity)
      + u32::from(variants.pseudo_element.is_some());

    Some(Self {
      order: TwOrder::new(token, &declarations, variants.order()),
      declarations,
      important,
      breakpoint: variants.breakpoint,
      container: variants
        .container
        .into_iter()
        .map(|container| container.query)
        .collect(),
      media: variants.media,
      condition,
      pseudo_element: variants.pseudo_element,
      specificity,
      id: xxh3_64(token.as_bytes()),
      theme_keyframes,
    })
  }

  /// Whether the rule's breakpoint and media queries, if any, admit the viewport.
  pub(crate) fn is_active(&self, viewport: Viewport, breakpoints: &BreakpointOverrides) -> bool {
    self
      .breakpoint
      .iter()
      .all(|breakpoint| breakpoint.matches(viewport, breakpoints))
      && self.media.iter().all(|media| media.matches(viewport))
  }
}
