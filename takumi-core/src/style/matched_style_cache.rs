//! Blink's `MatchedPropertiesCache`: boxes that match the same rules under parents with equal
//! inherited values share one resolved style, as the cells of one table column do.

use std::{
  cell::{Cell, RefCell},
  rc::Rc,
};

use rustc_hash::FxHashMap;
use smallvec::SmallVec;

use crate::{
  layout::node::NodeStyleLayers,
  matching::MatchedDeclarationsView,
  style::{Color, ComputedStyle, SizingContext, TailwindValues, TwMatches},
};

/// Past this many entries the cache starts over, as a tree whose boxes share nothing would fill it
/// with entries nothing reads again.
const MAX_ENTRIES: usize = 4096;

/// The rules a box matched, its `tw` class list and the variants of it its place admits, with the
/// parent's `currentColor` and font size, which Blink's key also hashes to keep a key's entries
/// few.
#[derive(Clone, PartialEq, Eq, Hash)]
pub(crate) struct MatchedStyleKey {
  rules: SmallVec<[(usize, bool); 8]>,
  tw: Option<u64>,
  tw_matches: TwMatches,
  parent_color: [u8; 4],
  parent_font_size: u32,
}

impl MatchedStyleKey {
  /// The key of a box that matched `matched`, styled by `tw` where `tw_matches` admits, under a
  /// parent with `parent_sizing` and `parent_color`.
  pub(crate) fn new(
    matched: &MatchedDeclarationsView<'_>,
    tw: Option<&TailwindValues>,
    tw_matches: &TwMatches,
    parent_sizing: &SizingContext,
    parent_color: Color,
  ) -> Self {
    Self {
      rules: matched.rules().into(),
      tw: tw.map(TailwindValues::digest),
      tw_matches: tw_matches.clone(),
      parent_color: parent_color.0,
      parent_font_size: parent_sizing.font_size.to_bits(),
    }
  }
}

/// A style resolved once, with the parent and the box's own style layers it resolved against.
struct MatchedStyle {
  parent: Rc<ComputedStyle>,
  parent_sizing: SizingContext,
  layers: NodeStyleLayers,
  inherits_explicitly: bool,
  resolved: (Rc<ComputedStyle>, SizingContext, Color),
}

impl MatchedStyle {
  /// Whether a box with `layers` resolves to this under `parent`, as Blink's
  /// `MatchedPropertiesCache::Find` decides.
  fn holds_for(
    &self,
    parent: &ComputedStyle,
    parent_sizing: &SizingContext,
    layers: &NodeStyleLayers,
  ) -> bool {
    self.layers == *layers
      && self.parent_sizing.resolves_like(parent_sizing)
      && self.parent.inherited_equal(parent)
      && (!self.inherits_explicitly || self.parent.non_inherited_equal(parent))
  }
}

/// The styles boxes resolved, by matched declarations.
#[derive(Default)]
pub(crate) struct MatchedStyleCache {
  entries: RefCell<FxHashMap<MatchedStyleKey, Vec<MatchedStyle>>>,
  len: Cell<usize>,
}

impl MatchedStyleCache {
  /// The style, sizing, and `currentColor` a box with `layers` resolved to under the key, when it
  /// resolves the same under `parent`.
  pub(crate) fn get(
    &self,
    key: &MatchedStyleKey,
    parent: &ComputedStyle,
    parent_sizing: &SizingContext,
    layers: &NodeStyleLayers,
  ) -> Option<(Rc<ComputedStyle>, SizingContext, Color)> {
    self
      .entries
      .borrow()
      .get(key)?
      .iter()
      .rev()
      .find(|entry| entry.holds_for(parent, parent_sizing, layers))
      .map(|entry| entry.resolved.clone())
  }

  /// Keeps what a box with `layers` resolved to under `parent`.
  pub(crate) fn insert(
    &self,
    key: MatchedStyleKey,
    parent: Rc<ComputedStyle>,
    parent_sizing: SizingContext,
    layers: NodeStyleLayers,
    inherits_explicitly: bool,
    resolved: (Rc<ComputedStyle>, SizingContext, Color),
  ) {
    let mut entries = self.entries.borrow_mut();

    if self.len.get() >= MAX_ENTRIES {
      entries.clear();
      self.len.set(0);
    }
    entries.entry(key).or_default().push(MatchedStyle {
      parent,
      parent_sizing,
      layers,
      inherits_explicitly,
      resolved,
    });
    self.len.set(self.len.get() + 1);
  }
}
