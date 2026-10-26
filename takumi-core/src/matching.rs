//! Selector matching over the [`Node`] tree.
//!
//! All interaction with the `selectors` crate lives here so it stays out of
//! takumi's public API. Callers pass a node tree and receive back the
//! declaration blocks that apply to each node.

use std::{collections::HashMap, fmt, ops::Deref, sync::Arc};

use rustc_hash::FxHashMap;
use selectors::{
  Element, OpaqueElement, SelectorImpl as SelectorImplTrait,
  attr::{CaseSensitivity, NamespaceConstraint},
  bloom::BloomFilter,
  matching::*,
  parser::{AncestorHashes, Component, Selector},
};
use smallvec::SmallVec;

use crate::{
  layout::node::{Node, NodeKind},
  sort_key::sort_by_key,
  style::{
    BreakpointOverrides, Direction, LonghandId, MediaQueryList, StyleDeclarationBlock, TwRule,
    selector::{CssRule, Ident, PseudoClass, PseudoElement, SelectorImpl, StyleSheet},
  },
  viewport::Viewport,
};

struct StyleArena<'a> {
  nodes: Vec<StyleNode<'a>>,
}
struct StyleNode<'a> {
  node: &'a Node,
  parent: Option<usize>,
  prev_sibling: Option<usize>,
  next_sibling: Option<usize>,
  first_child: Option<usize>,
}
#[derive(Clone, Copy)]
struct ArenaElement<'a> {
  tree: &'a StyleArena<'a>,
  index: usize,
}

impl fmt::Debug for ArenaElement<'_> {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.debug_struct("ArenaElement")
      .field("index", &self.index)
      .finish()
  }
}

impl<'a> StyleArena<'a> {
  fn new(root: &'a Node) -> Self {
    let mut arena = StyleArena { nodes: Vec::new() };
    arena.add_node(root, None, None);
    arena
  }

  fn add_node(
    &mut self,
    node: &'a Node,
    parent: Option<usize>,
    prev_sibling: Option<usize>,
  ) -> usize {
    struct ChildFrame<'a> {
      parent_index: usize,
      children: &'a [Node],
      next_child: usize,
      current_prev: Option<usize>,
    }

    let root_index = self.push_node(node, parent, prev_sibling);
    let mut stack = Vec::new();

    if let Some(children) = node.children() {
      stack.push(ChildFrame {
        parent_index: root_index,
        children,
        next_child: 0,
        current_prev: None,
      });
    }

    while let Some(frame) = stack.last_mut() {
      if frame.next_child >= frame.children.len() {
        stack.pop();
        continue;
      }

      let child = &frame.children[frame.next_child];
      let child_prev = frame.current_prev;
      frame.next_child += 1;

      let child_index = self.push_node(child, Some(frame.parent_index), child_prev);
      if child_prev.is_none() {
        self.nodes[frame.parent_index].first_child = Some(child_index);
      }
      frame.current_prev = Some(child_index);

      if let Some(children) = child.children() {
        stack.push(ChildFrame {
          parent_index: child_index,
          children,
          next_child: 0,
          current_prev: None,
        });
      }
    }

    root_index
  }

  fn push_node(
    &mut self,
    node: &'a Node,
    parent: Option<usize>,
    prev_sibling: Option<usize>,
  ) -> usize {
    let index = self.nodes.len();
    self.nodes.push(StyleNode {
      node,
      parent,
      prev_sibling,
      next_sibling: None,
      first_child: None,
    });

    if let Some(prev) = prev_sibling {
      self.nodes[prev].next_sibling = Some(index);
    }

    index
  }
}

fn hash_ascii_case_insensitive(value: &str) -> u32 {
  let mut hash = 0x811c_9dc5u32;
  for byte in value.as_bytes() {
    hash ^= u32::from(byte.to_ascii_lowercase());
    hash = hash.wrapping_mul(0x0100_0193);
  }
  hash
}

/// The hashes of the node's tag, id and classes, as the ancestor bloom filter holds them.
fn node_unique_hashes(node: &Node) -> impl Iterator<Item = u32> {
  node
    .tag_name()
    .into_iter()
    .chain(node.id())
    .chain(
      node
        .class_name()
        .into_iter()
        .flat_map(str::split_whitespace),
    )
    .map(hash_ascii_case_insensitive)
}

fn add_node_unique_hashes_to_filter(node: &Node, filter: &mut BloomFilter) -> bool {
  let mut added = false;

  for hash in node_unique_hashes(node) {
    filter.insert_hash(hash);
    added = true;
  }

  added
}

fn remove_node_unique_hashes_from_filter(node: &Node, filter: &mut BloomFilter) {
  for hash in node_unique_hashes(node) {
    filter.remove_hash(hash);
  }
}

impl<'a> ArenaElement<'a> {
  fn style_node(&self) -> &'a StyleNode<'a> {
    &self.tree.nodes[self.index]
  }

  /// HTML's directionality: the `dir` of the nearest element that sets one, left to right when
  /// none does.
  /// <https://html.spec.whatwg.org/multipage/dom.html#the-directionality>
  fn directionality(&self) -> Direction {
    let mut current = Some(*self);

    while let Some(element) = current {
      if let Some(direction) = element.style_node().node.metadata.dir {
        return direction;
      }

      current = element.parent_element();
    }

    Direction::Ltr
  }

  fn at(&self, index: Option<usize>) -> Option<Self> {
    index.map(|index| ArenaElement {
      tree: self.tree,
      index,
    })
  }

  /// The first element from `index` on, following `next` past text nodes, which selectors do
  /// not count as siblings or children.
  fn element_from(
    &self,
    mut index: Option<usize>,
    next: impl Fn(&StyleNode<'_>) -> Option<usize>,
  ) -> Option<Self> {
    while let Some(current) = index {
      let style_node = &self.tree.nodes[current];

      if text_of(style_node.node).is_none() {
        return self.at(Some(current));
      }
      index = next(style_node);
    }

    None
  }
}

/// The text of a text node, which selectors do not count as an element; `None` for an element.
fn text_of(node: &Node) -> Option<&str> {
  match &node.kind {
    NodeKind::Text(data) if node.tag_name().is_none() => Some(&data.text),
    _ => None,
  }
}

impl Element for ArenaElement<'_> {
  type Impl = SelectorImpl;

  fn opaque(&self) -> OpaqueElement {
    OpaqueElement::new(self.style_node().node)
  }

  fn parent_element(&self) -> Option<Self> {
    self.at(self.style_node().parent)
  }

  fn parent_node_is_shadow_root(&self) -> bool {
    false
  }

  fn containing_shadow_host(&self) -> Option<Self> {
    None
  }

  fn is_pseudo_element(&self) -> bool {
    false
  }

  fn prev_sibling_element(&self) -> Option<Self> {
    self.element_from(self.style_node().prev_sibling, |node| node.prev_sibling)
  }

  fn next_sibling_element(&self) -> Option<Self> {
    self.element_from(self.style_node().next_sibling, |node| node.next_sibling)
  }

  fn first_element_child(&self) -> Option<Self> {
    self.element_from(self.style_node().first_child, |node| node.next_sibling)
  }

  fn is_html_element_in_html_document(&self) -> bool {
    true
  }

  fn has_local_name(&self, local_name: &Ident) -> bool {
    self
      .style_node()
      .node
      .tag_name()
      .is_some_and(|tag| tag.eq_ignore_ascii_case(local_name))
  }

  fn has_namespace(&self, _ns: &Ident) -> bool {
    false
  }

  fn is_same_type(&self, other: &Self) -> bool {
    match (
      self.style_node().node.tag_name(),
      other.style_node().node.tag_name(),
    ) {
      (Some(a), Some(b)) => a.eq_ignore_ascii_case(b),
      (a, b) => a == b,
    }
  }

  fn has_id(&self, id: &Ident, _case_sensitivity: CaseSensitivity) -> bool {
    self.style_node().node.id() == Some(&**id)
  }

  fn has_class(&self, name: &Ident, _case_sensitivity: CaseSensitivity) -> bool {
    self
      .style_node()
      .node
      .class_name()
      .is_some_and(|classes| classes.split_whitespace().any(|c| c == *name))
  }

  fn imported_part(&self, _name: &Ident) -> Option<Ident> {
    None
  }

  fn is_part(&self, _name: &Ident) -> bool {
    false
  }

  /// Blink's `:empty`: no element child, and no text with a character in it.
  fn is_empty(&self) -> bool {
    let mut index = self.style_node().first_child;

    while let Some(current) = index {
      let style_node = &self.tree.nodes[current];

      if text_of(style_node.node).is_none_or(|text| !text.is_empty()) {
        return false;
      }
      index = style_node.next_sibling;
    }

    true
  }

  fn is_root(&self) -> bool {
    self.style_node().parent.is_none()
  }

  fn has_custom_state(&self, _name: &Ident) -> bool {
    false
  }

  fn attr_matches(
    &self,
    ns: &NamespaceConstraint<&Ident>,
    local_name: &Ident,
    operation: &selectors::attr::AttrSelectorOperation<&Ident>,
  ) -> bool {
    let namespace_supported = match ns {
      NamespaceConstraint::Any => true,
      NamespaceConstraint::Specific(url) => url.is_empty(),
    };
    if !namespace_supported {
      return false;
    }

    self
      .style_node()
      .node
      .attribute(local_name)
      .is_some_and(|value| operation.eval_str(value))
  }
  fn match_non_ts_pseudo_class(
    &self,
    pc: &<Self::Impl as SelectorImplTrait>::NonTSPseudoClass,
    _context: &mut MatchingContext<'_, Self::Impl>,
  ) -> bool {
    let ranges = match pc {
      PseudoClass::Lang(ranges) => ranges,
      PseudoClass::Dir(direction) => {
        return *direction
          == match self.directionality() {
            Direction::Ltr => "ltr",
            Direction::Rtl => "rtl",
          };
      }
      PseudoClass::Other(_) => return false,
    };

    // `lang` is inherited: walk up from this element to the nearest ancestor-or-self that
    // has one set, matching HTML's language-determination algorithm. No ancestor has one
    // set means the language is unknown, so `:lang()` never matches.
    let mut current = Some(*self);
    while let Some(element) = current {
      if let Some(lang) = element.style_node().node.attribute("lang") {
        return ranges.iter().any(|range| lang_matches(lang, range));
      }
      current = element.parent_element();
    }
    false
  }
  fn match_pseudo_element(
    &self,
    _pe: &<Self::Impl as SelectorImplTrait>::PseudoElement,
    _context: &mut MatchingContext<'_, Self::Impl>,
  ) -> bool {
    false
  }

  fn apply_selector_flags(&self, _flags: ElementSelectorFlags) {}
  fn is_link(&self) -> bool {
    false
  }
  fn is_html_slot_element(&self) -> bool {
    false
  }
  fn add_element_unique_hashes(&self, filter: &mut BloomFilter) -> bool {
    add_node_unique_hashes_to_filter(self.style_node().node, filter)
  }
}

/// A matched declaration block: a stylesheet rule's, or the rule a `tw` utility compiles to.
#[derive(Debug, Clone)]
pub(crate) enum MatchedBlock<'a> {
  Sheet(&'a StyleDeclarationBlock),
  Tw(Arc<TwRule>),
}

impl Deref for MatchedBlock<'_> {
  type Target = StyleDeclarationBlock;

  fn deref(&self) -> &StyleDeclarationBlock {
    match self {
      Self::Sheet(declarations) => declarations,
      Self::Tw(rule) => &rule.declarations,
    }
  }
}

/// Names a matched rule the same on every element of a render: a stylesheet rule by its position,
/// a `tw` rule by its utility.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum RuleId {
  Sheet(usize),
  Tw(u64),
}

/// Declaration blocks that apply to one element, split by `!important`.
#[derive(Debug, Default, Clone)]
pub(crate) struct MatchedDeclarationsView<'a> {
  normal: SmallVec<[MatchedBlock<'a>; 4]>,
  /// A `Vec`, as `!important` is rare and every node of the tree holds this.
  important: Vec<MatchedBlock<'a>>,
  /// Where unlayered blocks begin in `normal`, after every named `@layer` and then the `tw` layer,
  /// which is declared last.
  unlayered_start: usize,
  /// Where layered blocks begin in `important`, which reverses layer order: the `tw` layer first,
  /// then every named `@layer`.
  layered_important_start: usize,
  /// The rule each block came from and whether it is the `!important` block, in cascade order.
  rules: SmallVec<[(RuleId, bool); 4]>,
}

impl<'a> MatchedDeclarationsView<'a> {
  /// Matched blocks without `!important` from named layers and the `tw` layer, in cascade order.
  pub fn layered_normal(&self) -> &[MatchedBlock<'a>] {
    &self.normal[..self.unlayered_start]
  }

  /// Matched unlayered blocks without `!important`, in cascade order.
  pub fn unlayered_normal(&self) -> &[MatchedBlock<'a>] {
    &self.normal[self.unlayered_start..]
  }

  /// Matched declaration blocks marked `!important`, in cascade order.
  #[cfg(test)]
  pub(crate) fn important(&self) -> &[MatchedBlock<'a>] {
    &self.important
  }

  /// Matched unlayered blocks marked `!important`, in cascade order.
  pub(crate) fn unlayered_important(&self) -> &[MatchedBlock<'a>] {
    &self.important[..self.layered_important_start]
  }

  /// Matched blocks marked `!important` from the `tw` layer and named layers, in cascade order.
  pub(crate) fn layered_important(&self) -> &[MatchedBlock<'a>] {
    &self.important[self.layered_important_start..]
  }

  /// The rules the blocks came from, which name the same blocks on every element of the render.
  pub(crate) fn rules(&self) -> &[(RuleId, bool)] {
    &self.rules
  }

  /// Whether a matched declaration may copy a non-inherited value from the parent.
  pub(crate) fn inherits_explicitly(&self) -> bool {
    self
      .normal
      .iter()
      .chain(&self.important)
      .any(|block| block.inherits_explicitly())
  }
}

/// Per-node matching result: the element's own declarations plus declarations
/// for any matched `::before` / `::after` pseudo-elements.
#[derive(Debug, Default, Clone)]
pub(crate) struct NodeMatchedDeclarations<'a> {
  element: MatchedDeclarationsView<'a>,
  /// Boxed, as few elements have one and every node of the tree holds this.
  before: Option<Box<MatchedDeclarationsView<'a>>>,
  after: Option<Box<MatchedDeclarationsView<'a>>>,
}

impl<'a> NodeMatchedDeclarations<'a> {
  /// Declarations matching the element itself.
  pub fn element(&self) -> &MatchedDeclarationsView<'a> {
    &self.element
  }

  /// Declarations matching the element's `::before`, if any rule targeted it.
  pub fn before(&self) -> Option<&MatchedDeclarationsView<'a>> {
    self.before.as_deref()
  }

  /// Declarations matching the element's `::after`, if any rule targeted it.
  pub fn after(&self) -> Option<&MatchedDeclarationsView<'a>> {
    self.after.as_deref()
  }
}

#[derive(Debug, Clone)]
struct MatchedRule<'a> {
  important: bool,
  layer_order: usize,
  specificity: u32,
  /// A stylesheet rule's position, or a `tw` rule's place in Tailwind's sort of this render's
  /// utilities.
  source_order: usize,
  id: RuleId,
  declarations: MatchedBlock<'a>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SelectorTarget {
  Element,
  Before,
  After,
}

/// What a selector's rightmost compound requires of the element it matches.
///
/// A rule whose rightmost compound names an id, class or tag can only match an
/// element carrying that name, so other nodes skip it without running the
/// matcher. Anything else, a bare `*` or an attribute selector, has to be
/// considered for every node.
enum SelectorKey {
  Id(String),
  Class(String),
  /// Lowercased: tag names match case-insensitively.
  Tag(String),
  Any,
}

fn selector_key(selector: &Selector<SelectorImpl>) -> SelectorKey {
  let mut key = SelectorKey::Any;

  // `iter` walks the rightmost compound and stops at the first combinator.
  for component in selector.iter() {
    match component {
      Component::ID(id) => return SelectorKey::Id(id.to_string()),
      Component::Class(class) => key = SelectorKey::Class(class.to_string()),
      Component::LocalName(name) if matches!(key, SelectorKey::Any) => {
        key = SelectorKey::Tag(name.lower_name.to_ascii_lowercase());
      }
      _ => {}
    }
  }

  key
}

/// A stylesheet's rules, by position, grouped by what their rightmost compound
/// requires, so a node visits only the rules that could match it.
#[derive(Debug, Default)]
pub(crate) struct RuleIndex {
  by_id: HashMap<String, Vec<usize>>,
  by_class: HashMap<String, Vec<usize>>,
  by_tag: HashMap<String, Vec<usize>>,
  unindexed: Vec<usize>,
  /// Each distinct `media_queries` list among the rules.
  media: Vec<Vec<MediaQueryList>>,
  /// The position in `media` of each rule's `media_queries`.
  rule_media: Vec<usize>,
}

impl RuleIndex {
  pub(crate) fn build(rules: &[CssRule]) -> Self {
    let mut index = Self {
      rule_media: Vec::with_capacity(rules.len()),
      ..Self::default()
    };

    for (position, rule) in rules.iter().enumerate() {
      let media = index.media_id(&rule.media_queries);
      index.rule_media.push(media);

      let keys: SmallVec<[SelectorKey; 4]> =
        rule.selectors.slice().iter().map(selector_key).collect();

      // A rule with one unindexable selector has to be visited by every node,
      // so it goes wholly into the unindexed bucket.
      if keys.iter().any(|key| matches!(key, SelectorKey::Any)) {
        index.unindexed.push(position);
        continue;
      }

      for key in keys {
        let bucket = match key {
          SelectorKey::Id(name) => index.by_id.entry(name),
          SelectorKey::Class(name) => index.by_class.entry(name),
          SelectorKey::Tag(name) => index.by_tag.entry(name),
          SelectorKey::Any => unreachable!("handled above"),
        }
        .or_default();

        if bucket.last() != Some(&position) {
          bucket.push(position);
        }
      }
    }

    index
  }

  /// The position of `media_queries` in `media`, adding it if it is new.
  fn media_id(&mut self, media_queries: &[MediaQueryList]) -> usize {
    // Rules from one `@media` block are adjacent, so search from the latest list.
    if let Some(id) = self.media.iter().rposition(|media| media == media_queries) {
      return id;
    }

    self.media.push(media_queries.to_vec());
    self.media.len() - 1
  }

  /// Whether each distinct media query list in `media` matches `viewport`.
  fn matching_media(&self, viewport: Viewport) -> Vec<bool> {
    self
      .media
      .iter()
      .map(|media| media.iter().all(|list| list.matches(viewport)))
      .collect()
  }

  /// Rules that could match `node` and whose media queries match, in source order.
  fn candidates(&self, node: &Node, matching_media: &[bool], out: &mut Vec<usize>) {
    out.clear();
    out.extend_from_slice(&self.unindexed);

    if let Some(id) = node.id()
      && let Some(rules) = self.by_id.get(id)
    {
      out.extend_from_slice(rules);
    }

    if let Some(classes) = node.class_name() {
      for class in classes.split_ascii_whitespace() {
        if let Some(rules) = self.by_class.get(class) {
          out.extend_from_slice(rules);
        }
      }
    }

    if let Some(tag) = node.tag_name()
      && let Some(rules) = self.by_tag.get(&tag.to_ascii_lowercase())
    {
      out.extend_from_slice(rules);
    }

    out.retain(|&rule| matching_media[self.rule_media[rule]]);
    sort_by_key(out, |&index| index);
    out.dedup();
  }
}

/// A stylesheet among those a tree matches, with where its rules and named layers sit among them.
struct SheetPlacement<'a> {
  sheet: &'a StyleSheet,
  matching_media: Vec<bool>,
  rule_offset: usize,
  layer_offset: usize,
}

/// Matches every rule in `stylesheets`, which cascade in order, against the tree rooted at
/// `root`, returning the declaration blocks that apply to each node in tree order. Approximate:
/// the named layers of different sheets stay apart where CSS merges those with the same name.
pub(crate) fn match_stylesheets_view<'a>(
  root: &Node,
  stylesheets: &[&'a StyleSheet],
  viewport: Viewport,
) -> Vec<NodeMatchedDeclarations<'a>> {
  let mut placements = Vec::with_capacity(stylesheets.len());
  let (mut rule_offset, mut layer_offset) = (0, 0);

  for &sheet in stylesheets {
    placements.push(SheetPlacement {
      sheet,
      matching_media: sheet.rule_index().matching_media(viewport),
      rule_offset,
      layer_offset,
    });
    rule_offset += sheet.rules().len();
    layer_offset += sheet.layer_count;
  }

  let no_breakpoints = BreakpointOverrides::default();
  let breakpoints = stylesheets
    .last()
    .map_or(&no_breakpoints, |sheet| &sheet.breakpoints);

  let arena = StyleArena::new(root);
  let node_count = arena.nodes.len();
  let mut per_node = vec![NodeMatchedDeclarations::default(); node_count];
  let tw_order = tw_rule_order(&arena);
  // The named layers, then the `tw` layer, declared last.
  let layers = layer_offset + 1;

  // Every distinct media query list gates some rule, so none matching means no
  // stylesheet rule applies. Without utilities either, skip the matching walk.
  if placements
    .iter()
    .all(|placement| !placement.matching_media.contains(&true))
    && tw_order.is_empty()
  {
    return per_node;
  }

  // Indexed by `SelectorTarget`, each emptied into the node's view once it is matched.
  let mut node_rules: [Vec<MatchedRule<'a>>; 3] = Default::default();

  let mut ancestor_bloom_filter = BloomFilter::new();
  let mut ancestor_stack: Vec<usize> = Vec::new();
  let mut selector_ancestor_hashes_cache: HashMap<(usize, usize), AncestorHashes> = HashMap::new();

  let mut candidates = Vec::new();

  let mut element_caches = SelectorCaches::default();
  let mut pseudo_caches = SelectorCaches::default();

  // Arena order is DFS preorder, so one counting filter walked along the
  // ancestor chain replaces a per-node filter copy: pop-and-remove until the
  // stack top is this node's parent, match against strict-ancestor hashes
  // only, then push self before descending.
  for (i, matched) in per_node.iter_mut().enumerate() {
    while ancestor_stack.last().copied() != arena.nodes[i].parent {
      let Some(left) = ancestor_stack.pop() else {
        break;
      };
      remove_node_unique_hashes_from_filter(arena.nodes[left].node, &mut ancestor_bloom_filter);
    }

    let element = ArenaElement {
      tree: &arena,
      index: i,
    };
    let is_replaced = arena.nodes[i].node.is_replaced_element();

    let mut element_ctx = MatchingContext::new(
      MatchingMode::Normal,
      Some(&ancestor_bloom_filter),
      &mut element_caches,
      QuirksMode::NoQuirks,
      NeedsSelectorFlags::No,
      MatchingForInvalidation::No,
    );
    let mut pseudo_ctx = MatchingContext::new(
      MatchingMode::ForStatelessPseudoElement,
      Some(&ancestor_bloom_filter),
      &mut pseudo_caches,
      QuirksMode::NoQuirks,
      NeedsSelectorFlags::No,
      MatchingForInvalidation::No,
    );

    for placement in &placements {
      placement.sheet.rule_index().candidates(
        arena.nodes[i].node,
        &placement.matching_media,
        &mut candidates,
      );

      for &rule_index in &candidates {
        let rule = &placement.sheet.rules()[rule_index];
        let source_order = placement.rule_offset + rule_index;
        let mut best_specificities: [Option<u32>; 3] = [None; 3];

        for (selector_index, selector) in rule.selectors.slice().iter().enumerate() {
          let Some(target) = selector_target(selector) else {
            continue;
          };
          if is_replaced && target != SelectorTarget::Element {
            continue;
          }

          let selector_key = (source_order, selector_index);
          let ancestor_hashes = selector_ancestor_hashes_cache
            .entry(selector_key)
            .or_insert_with(|| AncestorHashes::new(selector, QuirksMode::NoQuirks));

          if early_reject_by_local_name(selector, 0, &element) {
            continue;
          }

          let ctx = if target == SelectorTarget::Element {
            &mut element_ctx
          } else {
            &mut pseudo_ctx
          };

          if matches_selector(selector, 0, Some(ancestor_hashes), &element, ctx) {
            let specificity = selector.specificity();
            let slot = &mut best_specificities[target as usize];

            *slot = Some(slot.map_or(specificity, |best| best.max(specificity)));
          }
        }

        for (best_specificity, bucket) in best_specificities.into_iter().zip(node_rules.iter_mut())
        {
          record_matches(
            rule,
            source_order,
            placement.layer_offset,
            layers,
            best_specificity,
            bucket,
          );
        }
      }
    }

    for rule in arena.nodes[i].node.tw_rules() {
      let target = match &rule.pseudo_element {
        None => SelectorTarget::Element,
        Some(PseudoElement::Before) => SelectorTarget::Before,
        Some(_) => SelectorTarget::After,
      };
      let applies = (!is_replaced || target == SelectorTarget::Element)
        && rule
          .breakpoint
          .as_ref()
          .is_none_or(|breakpoint| breakpoint.matches(viewport, breakpoints))
        && rule
          .condition
          .as_ref()
          .is_none_or(|condition| matches_selector(condition, 0, None, &element, &mut element_ctx));

      if applies {
        record_tw_match(
          rule,
          tw_order[&rule.id],
          layers,
          &mut node_rules[target as usize],
        );
      }
    }

    let [element_rules, before, after] = &mut node_rules;

    finalize_bucket(element_rules, layers, &mut matched.element);
    matched.before = take_pseudo_bucket(before, layers).map(Box::new);
    matched.after = take_pseudo_bucket(after, layers).map(Box::new);
    ancestor_stack.push(i);
    add_node_unique_hashes_to_filter(arena.nodes[i].node, &mut ancestor_bloom_filter);
  }

  per_node
}

fn selector_target(selector: &Selector<SelectorImpl>) -> Option<SelectorTarget> {
  match selector.pseudo_element() {
    None => Some(SelectorTarget::Element),
    Some(PseudoElement::Before) => Some(SelectorTarget::Before),
    Some(PseudoElement::After) => Some(SelectorTarget::After),
    Some(PseudoElement::Other(_)) => None,
  }
}

/// Records `rule`'s blocks in `bucket` when one of its selectors matched, at `best_specificity`;
/// its sheet's named layers start at `layer_offset`, and `layers` counts every named layer and the
/// `tw` layer after them.
fn record_matches<'a>(
  rule: &'a CssRule,
  source_order: usize,
  layer_offset: usize,
  layers: usize,
  best_specificity: Option<u32>,
  bucket: &mut Vec<MatchedRule<'a>>,
) {
  let Some(specificity) = best_specificity else {
    return;
  };
  let layer = rule.layer_order.map(|layer| layer_offset + layer);

  if !rule.normal_declarations.is_empty() {
    bucket.push(MatchedRule {
      important: false,
      layer_order: layer_order(layer, false, layers),
      specificity,
      source_order,
      id: RuleId::Sheet(source_order),
      declarations: MatchedBlock::Sheet(&rule.normal_declarations),
    });
  }
  if !rule.important_declarations.is_empty() {
    bucket.push(MatchedRule {
      important: true,
      layer_order: layer_order(layer, true, layers),
      specificity,
      source_order,
      id: RuleId::Sheet(source_order),
      declarations: MatchedBlock::Sheet(&rule.important_declarations),
    });
  }
}

/// Where a block of `layer`, or of no layer, sorts among `layers`: layers in declaration order
/// and unlayered last, or for `!important` blocks the reverse.
fn layer_order(layer: Option<usize>, important: bool, layers: usize) -> usize {
  match (layer, important) {
    (Some(layer), false) => layer,
    (None, false) => layers,
    (Some(layer), true) => layers - layer,
    (None, true) => 0,
  }
}

/// Records the rule `tw_rule` compiles to in `bucket`, in the `tw` layer, the last of `layers`.
fn record_tw_match<'a>(
  tw_rule: &Arc<TwRule>,
  source_order: usize,
  layers: usize,
  bucket: &mut Vec<MatchedRule<'a>>,
) {
  bucket.push(MatchedRule {
    important: tw_rule.important,
    layer_order: layer_order(Some(layers - 1), tw_rule.important, layers),
    specificity: tw_rule.specificity,
    source_order,
    id: RuleId::Tw(tw_rule.id),
    declarations: MatchedBlock::Tw(Arc::clone(tw_rule)),
  });
}

fn finalize_bucket<'a>(
  rules: &mut Vec<MatchedRule<'a>>,
  layers: usize,
  matched: &mut MatchedDeclarationsView<'a>,
) {
  sort_by_key(rules, |rule| {
    (
      rule.important,
      rule.layer_order,
      rule.specificity,
      rule.source_order,
    )
  });
  for rule in rules.drain(..) {
    matched.rules.push((rule.id, rule.important));

    if rule.important {
      if rule.layer_order == 0 {
        matched.layered_important_start += 1;
      }
      matched.important.push(rule.declarations);
    } else {
      if rule.layer_order < layers {
        matched.unlayered_start += 1;
      }
      matched.normal.push(rule.declarations);
    }
  }
}

fn take_pseudo_bucket<'a>(
  rules: &mut Vec<MatchedRule<'a>>,
  layers: usize,
) -> Option<MatchedDeclarationsView<'a>> {
  // `content` does not inherit, and its initial `normal` computes to `none` on these pseudo-elements.
  let sets_content = rules.iter().any(|rule| {
    rule.declarations.iter().any(|declaration| {
      declaration
        .affected_longhands()
        .contains(&LonghandId::Content)
    })
  });

  if !sets_content {
    rules.clear();
    return None;
  }
  let mut view = MatchedDeclarationsView::default();
  finalize_bucket(rules, layers, &mut view);
  Some(view)
}

/// The places of the rules `tw` utilities in `arena` compile to, in Tailwind's sort of them: the
/// stylesheet a render's utilities would compile to.
fn tw_rule_order(arena: &StyleArena<'_>) -> FxHashMap<u64, usize> {
  let mut rules: FxHashMap<u64, &TwRule> = FxHashMap::default();

  for style_node in &arena.nodes {
    for rule in style_node.node.tw_rules() {
      rules.entry(rule.id).or_insert(rule);
    }
  }

  let mut sorted: Vec<&TwRule> = rules.into_values().collect();

  sorted.sort_unstable_by(|left, right| left.order.cmp(&right.order));
  sorted
    .into_iter()
    .enumerate()
    .map(|(place, rule)| (rule.id, place))
    .collect()
}

/// RFC 4647 basic filtering, as `:lang()` uses: `range` matches `lang` if they're equal
/// (case-insensitive) or `lang` extends `range` with a `-` boundary (`zh` matches `zh-Hant`).
/// `*` matches any non-empty `lang`.
pub(crate) fn lang_matches(lang: &str, range: &str) -> bool {
  if range == "*" {
    return true;
  }
  lang.eq_ignore_ascii_case(range)
    || lang
      .get(..range.len())
      .is_some_and(|prefix| prefix.eq_ignore_ascii_case(range))
      && lang.as_bytes().get(range.len()) == Some(&b'-')
}
