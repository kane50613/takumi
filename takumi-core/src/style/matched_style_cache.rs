//! Blink's `MatchedPropertiesCache`: the style a box resolves to from its parent's style and the
//! declarations it matched, which every box matching the same declarations under the same parent
//! shares, as the cells of one table column do.

use std::{cell::RefCell, ptr, rc::Rc};

use rustc_hash::FxHashMap;
use smallvec::SmallVec;

use crate::{
  matching::MatchedDeclarationsView,
  style::{Color, ComputedStyle, SizingContext, Style},
};

/// Past this many entries the cache starts over, as a tree whose boxes share nothing would fill it
/// with entries nothing reads again.
const MAX_ENTRIES: usize = 4096;

/// The parent style and the matched declaration blocks a resolved style is keyed by.
#[derive(Clone, PartialEq, Eq, Hash)]
pub(crate) struct MatchedStyleKey {
  parent: usize,
  declarations: SmallVec<[usize; 8]>,
}

impl MatchedStyleKey {
  /// The key of a box under `parent` that matched `matched`.
  pub(crate) fn new(parent: &Rc<ComputedStyle>, matched: &MatchedDeclarationsView<'_>) -> Self {
    let blocks = matched
      .layered_normal()
      .iter()
      .chain(matched.unlayered_normal())
      .map(|block| ptr::from_ref(*block).addr());
    let important = matched
      .unlayered_important()
      .iter()
      .chain(matched.layered_important())
      .map(|block| ptr::from_ref(*block).addr());
    // A zero no block address takes separates the normal blocks from the important ones.
    let declarations = blocks.chain([0]).chain(important).collect();

    Self {
      parent: Rc::as_ptr(parent).addr(),
      declarations,
    }
  }
}

/// A style resolved once, with what it resolved against beyond its key.
struct MatchedStyle {
  /// Held so its address is not reused while the key names it.
  _parent: Rc<ComputedStyle>,
  preset: Option<Style>,
  parent_sizing: SizingContext,
  parent_color: Color,
  resolved: (Rc<ComputedStyle>, SizingContext, Color),
}

/// The styles boxes resolved, by parent and matched declarations.
#[derive(Default)]
pub(crate) struct MatchedStyleCache {
  entries: RefCell<FxHashMap<MatchedStyleKey, Vec<MatchedStyle>>>,
}

impl MatchedStyleCache {
  /// The style, sizing, and `currentColor` a box with `preset` resolved to under the key, when the
  /// parent's sizing and color were the same.
  pub(crate) fn get(
    &self,
    key: &MatchedStyleKey,
    preset: Option<&Style>,
    parent_sizing: &SizingContext,
    parent_color: Color,
  ) -> Option<(Rc<ComputedStyle>, SizingContext, Color)> {
    self
      .entries
      .borrow()
      .get(key)?
      .iter()
      .find(|entry| {
        entry.preset.as_ref() == preset
          && entry.parent_color == parent_color
          && entry.parent_sizing.resolves_like(parent_sizing)
      })
      .map(|entry| entry.resolved.clone())
  }

  /// Keeps what a box with `preset` resolved to under `parent`.
  pub(crate) fn insert(
    &self,
    key: MatchedStyleKey,
    parent: Rc<ComputedStyle>,
    preset: Option<Style>,
    parent_sizing: SizingContext,
    parent_color: Color,
    resolved: (Rc<ComputedStyle>, SizingContext, Color),
  ) {
    let mut entries = self.entries.borrow_mut();

    if entries.len() >= MAX_ENTRIES {
      entries.clear();
    }
    entries.entry(key).or_default().push(MatchedStyle {
      _parent: parent,
      preset,
      parent_sizing,
      parent_color,
      resolved,
    });
  }
}
