//! Selector matching over an abstract node tree.
//!
//! All interaction with the `selectors` crate lives here so it stays out of
//! takumi's public API. Callers implement [`MatchableNode`] for their node type
//! and receive back the declaration blocks that apply to each node.

use std::{collections::HashMap, fmt};

use selectors::{
  Element, OpaqueElement, SelectorImpl as SelectorImplTrait,
  attr::{CaseSensitivity, NamespaceConstraint},
  bloom::BloomFilter,
  matching::*,
  parser::{AncestorHashes, Selector},
};
use smallvec::SmallVec;

use crate::{
  Viewport,
  style::{
    StyleDeclarationBlock,
    selector::{CssRule, Ident, PseudoElement, SelectorImpl, StyleSheet},
  },
};

/// A node the cascade can match selectors against.
///
/// Implemented by the caller's node type; the matcher only reads the queries
/// below, so the caller does not depend on the `selectors` crate.
pub trait MatchableNode {
  /// The element's tag name, if any.
  fn tag_name(&self) -> Option<&str>;
  /// The element's `id`, if any.
  fn id(&self) -> Option<&str>;
  /// The element's whitespace-separated class list, if any.
  fn class_name(&self) -> Option<&str>;
  /// The value of an attribute by name.
  fn attr(&self, name: &str) -> Option<&str>;
  /// Whether this is a replaced element (`<img>`-like), which suppresses
  /// `::before`/`::after`.
  fn is_replaced(&self) -> bool;
  /// The element's children in source order.
  fn children(&self) -> Option<&[Self]>
  where
    Self: Sized;
}

struct StyleArena<'a, N> {
  nodes: Vec<StyleNode<'a, N>>,
}
struct StyleNode<'a, N> {
  node: &'a N,
  parent: Option<usize>,
  prev_sibling: Option<usize>,
  next_sibling: Option<usize>,
  first_child: Option<usize>,
}
struct ArenaElement<'a, N> {
  tree: &'a StyleArena<'a, N>,
  index: usize,
}

impl<N> Clone for ArenaElement<'_, N> {
  fn clone(&self) -> Self {
    *self
  }
}
impl<N> Copy for ArenaElement<'_, N> {}

impl<N> fmt::Debug for ArenaElement<'_, N> {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.debug_struct("ArenaElement")
      .field("index", &self.index)
      .finish()
  }
}

impl<'a, N: MatchableNode> StyleArena<'a, N> {
  fn new(root: &'a N) -> Self {
    let mut arena = StyleArena { nodes: Vec::new() };
    arena.add_node(root, None, None);
    arena
  }

  fn add_node(&mut self, node: &'a N, parent: Option<usize>, prev_sibling: Option<usize>) -> usize {
    struct ChildFrame<'a, N> {
      parent_index: usize,
      children: &'a [N],
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
    node: &'a N,
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

fn add_node_unique_hashes_to_filter<N: MatchableNode>(node: &N, filter: &mut BloomFilter) -> bool {
  let mut added = false;

  if let Some(tag) = node.tag_name() {
    filter.insert_hash(hash_ascii_case_insensitive(tag));
    added = true;
  }

  if let Some(id) = node.id() {
    filter.insert_hash(hash_ascii_case_insensitive(id));
    added = true;
  }

  if let Some(classes) = node.class_name() {
    for class_name in classes.split_whitespace() {
      filter.insert_hash(hash_ascii_case_insensitive(class_name));
      added = true;
    }
  }

  added
}

impl<'a, N: MatchableNode> Element for ArenaElement<'a, N> {
  type Impl = SelectorImpl;

  fn opaque(&self) -> OpaqueElement {
    OpaqueElement::new(self.tree.nodes[self.index].node)
  }

  fn parent_element(&self) -> Option<Self> {
    self.tree.nodes[self.index]
      .parent
      .map(|index| ArenaElement {
        tree: self.tree,
        index,
      })
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
    self.tree.nodes[self.index]
      .prev_sibling
      .map(|index| ArenaElement {
        tree: self.tree,
        index,
      })
  }

  fn next_sibling_element(&self) -> Option<Self> {
    self.tree.nodes[self.index]
      .next_sibling
      .map(|index| ArenaElement {
        tree: self.tree,
        index,
      })
  }

  fn first_element_child(&self) -> Option<Self> {
    self.tree.nodes[self.index]
      .first_child
      .map(|index| ArenaElement {
        tree: self.tree,
        index,
      })
  }

  fn is_html_element_in_html_document(&self) -> bool {
    true
  }

  fn has_local_name(&self, local_name: &Ident) -> bool {
    let node = self.tree.nodes[self.index].node;
    if let Some(tag) = node.tag_name() {
      tag.eq_ignore_ascii_case(local_name)
    } else {
      false
    }
  }

  fn has_namespace(&self, _ns: &Ident) -> bool {
    false
  }

  fn is_same_type(&self, other: &Self) -> bool {
    match (
      self.tree.nodes[self.index].node.tag_name(),
      other.tree.nodes[other.index].node.tag_name(),
    ) {
      (Some(a), Some(b)) => a.eq_ignore_ascii_case(b),
      (a, b) => a == b,
    }
  }

  fn has_id(&self, id: &Ident, _case_sensitivity: CaseSensitivity) -> bool {
    let node = self.tree.nodes[self.index].node;
    node.id() == Some(&**id)
  }

  fn has_class(&self, name: &Ident, _case_sensitivity: CaseSensitivity) -> bool {
    let node = self.tree.nodes[self.index].node;
    if let Some(classes) = node.class_name() {
      classes.split_whitespace().any(|c| c == *name)
    } else {
      false
    }
  }

  fn imported_part(&self, _name: &Ident) -> Option<Ident> {
    None
  }

  fn is_part(&self, _name: &Ident) -> bool {
    false
  }

  fn is_empty(&self) -> bool {
    self.tree.nodes[self.index].first_child.is_none()
  }

  fn is_root(&self) -> bool {
    self.tree.nodes[self.index].parent.is_none()
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

    self.tree.nodes[self.index]
      .node
      .attr(local_name)
      .is_some_and(|value| operation.eval_str(value))
  }
  fn match_non_ts_pseudo_class(
    &self,
    _pc: &<Self::Impl as SelectorImplTrait>::NonTSPseudoClass,
    _context: &mut MatchingContext<'_, Self::Impl>,
  ) -> bool {
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
    add_node_unique_hashes_to_filter(self.tree.nodes[self.index].node, filter)
  }
}

/// Declaration blocks that apply to one element, split by `!important`.
#[derive(Debug, Default, Clone)]
pub struct MatchedDeclarationsView<'a> {
  normal: SmallVec<[&'a StyleDeclarationBlock; 4]>,
  important: SmallVec<[&'a StyleDeclarationBlock; 4]>,
}

impl<'a> MatchedDeclarationsView<'a> {
  /// Matched declaration blocks without `!important`, in cascade order.
  pub fn normal(&self) -> &[&'a StyleDeclarationBlock] {
    &self.normal
  }

  /// Matched declaration blocks marked `!important`, in cascade order.
  pub fn important(&self) -> &[&'a StyleDeclarationBlock] {
    &self.important
  }
}

/// Per-node matching result: the element's own declarations plus declarations
/// for any matched `::before` / `::after` pseudo-elements.
#[derive(Debug, Default, Clone)]
pub struct NodeMatchedDeclarations<'a> {
  element: MatchedDeclarationsView<'a>,
  before: Option<MatchedDeclarationsView<'a>>,
  after: Option<MatchedDeclarationsView<'a>>,
}

impl<'a> NodeMatchedDeclarations<'a> {
  /// Declarations matching the element itself.
  pub fn element(&self) -> &MatchedDeclarationsView<'a> {
    &self.element
  }

  /// Declarations matching the element's `::before`, if any rule targeted it.
  pub fn before(&self) -> Option<&MatchedDeclarationsView<'a>> {
    self.before.as_ref()
  }

  /// Declarations matching the element's `::after`, if any rule targeted it.
  pub fn after(&self) -> Option<&MatchedDeclarationsView<'a>> {
    self.after.as_ref()
  }
}

#[derive(Debug, Clone, Copy)]
struct MatchedRule<'a> {
  important: bool,
  layer_order: usize,
  specificity: u32,
  source_order: usize,
  declarations: &'a StyleDeclarationBlock,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SelectorTarget {
  Element,
  Before,
  After,
}

/// Matches every rule in `stylesheet` against the tree rooted at `root`,
/// returning the declaration blocks that apply to each node in tree order.
pub fn match_stylesheets_view<'a, N: MatchableNode>(
  root: &N,
  stylesheet: &'a StyleSheet,
  viewport: Viewport,
) -> Vec<NodeMatchedDeclarations<'a>> {
  let arena = StyleArena::new(root);
  let node_count = arena.nodes.len();
  let mut per_node = vec![NodeMatchedDeclarations::default(); node_count];

  let mut matched_element: Vec<Vec<MatchedRule<'a>>> = vec![Vec::new(); node_count];
  let mut matched_before: Vec<Vec<MatchedRule<'a>>> = vec![Vec::new(); node_count];
  let mut matched_after: Vec<Vec<MatchedRule<'a>>> = vec![Vec::new(); node_count];

  let mut ancestor_bloom_filters = vec![BloomFilter::new(); node_count];
  let mut selector_ancestor_hashes_cache: HashMap<usize, AncestorHashes> = HashMap::new();
  let flattened_rules: Vec<&CssRule> = stylesheet
    .rules
    .iter()
    .filter(|rule| {
      rule
        .media_queries
        .iter()
        .all(|media_queries| media_queries.matches(viewport))
    })
    .collect();

  for i in 0..node_count {
    let Some(parent) = arena.nodes[i].parent else {
      continue;
    };
    ancestor_bloom_filters[i] = ancestor_bloom_filters[parent].clone();
    add_node_unique_hashes_to_filter(arena.nodes[parent].node, &mut ancestor_bloom_filters[i]);
  }

  let mut element_caches = SelectorCaches::default();
  let mut pseudo_caches = SelectorCaches::default();

  for i in 0..node_count {
    let element = ArenaElement {
      tree: &arena,
      index: i,
    };
    let is_replaced = arena.nodes[i].node.is_replaced();

    let mut element_ctx = MatchingContext::new(
      MatchingMode::Normal,
      Some(&ancestor_bloom_filters[i]),
      &mut element_caches,
      QuirksMode::NoQuirks,
      NeedsSelectorFlags::No,
      MatchingForInvalidation::No,
    );
    let mut pseudo_ctx = MatchingContext::new(
      MatchingMode::ForStatelessPseudoElement,
      Some(&ancestor_bloom_filters[i]),
      &mut pseudo_caches,
      QuirksMode::NoQuirks,
      NeedsSelectorFlags::No,
      MatchingForInvalidation::No,
    );

    for (source_order, &rule) in flattened_rules.iter().enumerate() {
      let mut best_element: Option<u32> = None;
      let mut best_before: Option<u32> = None;
      let mut best_after: Option<u32> = None;

      for selector in rule.selectors().slice() {
        let Some(target) = selector_target(selector) else {
          continue;
        };
        if is_replaced && target != SelectorTarget::Element {
          continue;
        }

        let selector_key = selector as *const _ as usize;
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
          let slot = match target {
            SelectorTarget::Element => &mut best_element,
            SelectorTarget::Before => &mut best_before,
            SelectorTarget::After => &mut best_after,
          };
          *slot = Some(slot.map_or(specificity, |best| best.max(specificity)));
        }
      }

      record_matches(
        rule,
        source_order,
        stylesheet.layer_count,
        best_element,
        &mut matched_element[i],
      );
      record_matches(
        rule,
        source_order,
        stylesheet.layer_count,
        best_before,
        &mut matched_before[i],
      );
      record_matches(
        rule,
        source_order,
        stylesheet.layer_count,
        best_after,
        &mut matched_after[i],
      );
    }
  }

  for (i, matched) in per_node.iter_mut().enumerate() {
    finalize_bucket(&mut matched_element[i], &mut matched.element);
    matched.before = take_pseudo_bucket(&mut matched_before[i]);
    matched.after = take_pseudo_bucket(&mut matched_after[i]);
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

fn record_matches<'a>(
  rule: &'a CssRule,
  source_order: usize,
  layer_count: usize,
  best_specificity: Option<u32>,
  bucket: &mut Vec<MatchedRule<'a>>,
) {
  let Some(specificity) = best_specificity else {
    return;
  };
  let normal_layer_order = rule.layer_order.map_or(layer_count, |order| order);
  bucket.push(MatchedRule {
    important: false,
    layer_order: normal_layer_order,
    specificity,
    source_order,
    declarations: &rule.normal_declarations,
  });
  let important_layer_order = rule.layer_order.map_or(0, |order| layer_count - order);
  bucket.push(MatchedRule {
    important: true,
    layer_order: important_layer_order,
    specificity,
    source_order,
    declarations: &rule.important_declarations,
  });
}

fn finalize_bucket<'a>(
  rules: &mut Vec<MatchedRule<'a>>,
  matched: &mut MatchedDeclarationsView<'a>,
) {
  rules.sort_by_key(|rule| {
    (
      rule.important,
      rule.layer_order,
      rule.specificity,
      rule.source_order,
    )
  });
  for rule in rules.drain(..) {
    if rule.important {
      matched.important.push(rule.declarations);
    } else {
      matched.normal.push(rule.declarations);
    }
  }
}

fn take_pseudo_bucket<'a>(rules: &mut Vec<MatchedRule<'a>>) -> Option<MatchedDeclarationsView<'a>> {
  if rules.is_empty() {
    return None;
  }
  let mut view = MatchedDeclarationsView::default();
  finalize_bucket(rules, &mut view);
  Some(view)
}
