//! A utility compiled as Tailwind compiles it: one rule of the `tw` layer, which the cascade
//! orders among the others as Tailwind's [`compileCandidates`](https://github.com/tailwindlabs/tailwindcss/blob/main/packages/tailwindcss/src/compile.ts)
//! sorts them.

use std::{cmp::Reverse, sync::Arc};

use smallvec::SmallVec;
use xxhash_rust::xxh3::xxh3_64;

use super::{Breakpoint, property_order::property_index};
use crate::style::{Length, StyleDeclaration, StyleDeclarationBlock};

/// Tailwind's sort key for a utility: its variants, the properties it sets in Tailwind's order,
/// the one setting more properties first, then its name. Approximate: a breakpoint sorts by its
/// built-in width, where Tailwind compares the widths a theme gives.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct TwOrder {
  breakpoint: Option<(u32, Arc<str>)>,
  /// The properties' indices, ascending, and then `u16::MAX`, so a list that runs out sorts after
  /// a longer one sharing its start, as Tailwind reads a missing index as infinite.
  properties: SmallVec<[u16; 4]>,
  count: Reverse<usize>,
  name: Box<str>,
}

impl TwOrder {
  fn new(
    name: &str,
    declarations: &StyleDeclarationBlock,
    breakpoint: Option<&Breakpoint>,
  ) -> Self {
    let mut properties = SmallVec::<[u16; 4]>::new();

    for declaration in declarations.iter() {
      if matches!(declaration, StyleDeclaration::CustomProperty(..)) {
        continue;
      }

      let longhands = declaration.affected_longhands();

      properties.extend(
        longhands
          .iter()
          .filter_map(|longhand| property_index(longhand.css_name())),
      );
    }

    properties.sort_unstable();
    properties.dedup();
    properties.push(u16::MAX);

    Self {
      breakpoint: breakpoint.map(|breakpoint| {
        let width = match breakpoint.width {
          Some(Length::Rem(rem)) => rem,
          Some(Length::Px(px)) => px / 16.0,
          _ => f32::INFINITY,
        };

        (width.to_bits(), Arc::clone(&breakpoint.token))
      }),
      properties,
      count: Reverse(declarations.iter().count()),
      name: name.into(),
    }
  }
}

/// One utility's declarations, all `!important` or none, applying while its breakpoint admits the
/// viewport.
#[derive(Debug, PartialEq)]
pub(crate) struct TwRule {
  pub(crate) declarations: StyleDeclarationBlock,
  pub(crate) important: bool,
  pub(crate) breakpoint: Option<Breakpoint>,
  pub(crate) order: TwOrder,
  /// A digest of the utility's token, which names the rule among the ones a box matched.
  pub(crate) id: u64,
}

impl TwRule {
  pub(crate) fn new(
    token: &str,
    declarations: StyleDeclarationBlock,
    important: bool,
    breakpoint: Option<Breakpoint>,
  ) -> Self {
    Self {
      order: TwOrder::new(token, &declarations, breakpoint.as_ref()),
      declarations,
      important,
      breakpoint,
      id: xxh3_64(token.as_bytes()),
    }
  }
}
