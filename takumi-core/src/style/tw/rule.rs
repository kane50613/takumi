//! A utility compiled as Tailwind compiles it: one rule of the `tw` layer, which the cascade
//! orders among the others as Tailwind's [`compileCandidates`](https://github.com/tailwindlabs/tailwindcss/blob/main/packages/tailwindcss/src/compile.ts)
//! sorts them.

use std::cmp::Reverse;

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
    ContentItem, ContentValue, PropertyMask, StyleDeclaration, StyleDeclarationBlock,
    selector::{PseudoElement, SelectorImpl},
  },
  viewport::Viewport,
};

/// Tailwind's sort key for a utility: its variants, the properties it sets in Tailwind's order,
/// the one setting more properties first, then its name. Approximate: it ignores the `--tw-sort`
/// declaration that makes Tailwind sort `size-*` as `size` and a gradient stop as its variable.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct TwOrder {
  variants: VariantOrder,
  /// The properties' indices, ascending, and then `u16::MAX`, so a list that runs out sorts after
  /// a longer one sharing its start, as Tailwind reads a missing index as infinite.
  properties: SmallVec<[u16; 4]>,
  count: Reverse<usize>,
  name: Box<str>,
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
    properties.sort_unstable();
    properties.dedup();
    properties.push(u16::MAX);

    Self {
      variants,
      properties,
      count: Reverse(declarations.iter().count()),
      name: name.into(),
    }
  }
}

/// One utility's declarations, all `!important` or none, applying to the box or pseudo-element
/// its variants name while they hold.
#[derive(Debug, PartialEq)]
pub(crate) struct TwRule {
  pub(crate) declarations: StyleDeclarationBlock,
  pub(crate) important: bool,
  pub(crate) breakpoint: Option<Breakpoint>,
  /// The pseudo-classes the utility's structural variants require of the element.
  pub(crate) condition: Option<Selector<SelectorImpl>>,
  pub(crate) pseudo_element: Option<PseudoElement>,
  /// A class's specificity with its variants', as Tailwind's selector for the utility has.
  pub(crate) specificity: u32,
  pub(crate) order: TwOrder,
  /// A digest of the utility's token, which names the rule among the ones a box matched.
  pub(crate) id: u64,
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

    Self::new(source, builder.finish(), important, variants)
  }

  /// The rule `token` compiles to, or `None` when its variants form no selector.
  fn new(
    token: &str,
    declarations: StyleDeclarationBlock,
    important: bool,
    variants: Variants,
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
      condition,
      pseudo_element: variants.pseudo_element,
      specificity,
      id: xxh3_64(token.as_bytes()),
    })
  }

  /// Whether the rule's breakpoint, if any, admits the viewport.
  pub(super) fn is_active(&self, viewport: Viewport, breakpoints: &BreakpointOverrides) -> bool {
    self
      .breakpoint
      .as_ref()
      .is_none_or(|breakpoint| breakpoint.matches(viewport, breakpoints))
  }
}
