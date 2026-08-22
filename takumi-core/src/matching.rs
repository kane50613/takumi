//! Selector matching over the [`Node`] tree.
//!
//! All interaction with the `selectors` crate lives here so it stays out of
//! takumi's public API. Callers pass a node tree and receive back the
//! declaration blocks that apply to each node.

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
  layout::node::Node,
  style::{
    StyleDeclarationBlock,
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

fn add_node_unique_hashes_to_filter(node: &Node, filter: &mut BloomFilter) -> bool {
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

fn remove_node_unique_hashes_from_filter(node: &Node, filter: &mut BloomFilter) {
  if let Some(tag) = node.tag_name() {
    filter.remove_hash(hash_ascii_case_insensitive(tag));
  }

  if let Some(id) = node.id() {
    filter.remove_hash(hash_ascii_case_insensitive(id));
  }

  if let Some(classes) = node.class_name() {
    for class_name in classes.split_whitespace() {
      filter.remove_hash(hash_ascii_case_insensitive(class_name));
    }
  }
}

impl Element for ArenaElement<'_> {
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
    pc: &<Self::Impl as SelectorImplTrait>::NonTSPseudoClass,
    _context: &mut MatchingContext<'_, Self::Impl>,
  ) -> bool {
    let PseudoClass::Lang(ranges) = pc else {
      return false;
    };

    // `lang` is inherited: walk up from this element to the nearest ancestor-or-self that
    // has one set, matching HTML's language-determination algorithm. No ancestor has one
    // set means the language is unknown, so `:lang()` never matches.
    let mut current = Some(*self);
    while let Some(element) = current {
      if let Some(lang) = element.tree.nodes[element.index].node.attr("lang") {
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
    add_node_unique_hashes_to_filter(self.tree.nodes[self.index].node, filter)
  }
}

/// Declaration blocks that apply to one element, split by `!important`.
#[derive(Debug, Default, Clone)]
pub(crate) struct MatchedDeclarationsView<'a> {
  normal: SmallVec<[&'a StyleDeclarationBlock; 4]>,
  important: SmallVec<[&'a StyleDeclarationBlock; 4]>,
}

impl<'a> MatchedDeclarationsView<'a> {
  /// Matched declaration blocks without `!important`, in cascade order.
  pub fn normal(&self) -> &[&'a StyleDeclarationBlock] {
    &self.normal
  }

  /// Matched declaration blocks marked `!important`, in cascade order.
  pub(crate) fn important(&self) -> &[&'a StyleDeclarationBlock] {
    &self.important
  }
}

/// Per-node matching result: the element's own declarations plus declarations
/// for any matched `::before` / `::after` pseudo-elements.
#[derive(Debug, Default, Clone)]
pub(crate) struct NodeMatchedDeclarations<'a> {
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
pub(crate) fn match_stylesheets_view<'a>(
  root: &Node,
  stylesheet: &'a StyleSheet,
  viewport: Viewport,
) -> Vec<NodeMatchedDeclarations<'a>> {
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

  let arena = StyleArena::new(root);
  let node_count = arena.nodes.len();
  let mut per_node = vec![NodeMatchedDeclarations::default(); node_count];

  // No rules survive the media-query filter, so every node keeps the default
  // declarations. Skip the per-node match buckets and the matching walk.
  if flattened_rules.is_empty() {
    return per_node;
  }

  let mut matched_element: Vec<Vec<MatchedRule<'a>>> = vec![Vec::new(); node_count];
  let mut matched_before: Vec<Vec<MatchedRule<'a>>> = vec![Vec::new(); node_count];
  let mut matched_after: Vec<Vec<MatchedRule<'a>>> = vec![Vec::new(); node_count];

  let mut ancestor_bloom_filter = BloomFilter::new();
  let mut ancestor_stack: Vec<usize> = Vec::new();
  let mut selector_ancestor_hashes_cache: HashMap<(usize, usize), AncestorHashes> = HashMap::new();

  let mut element_caches = SelectorCaches::default();
  let mut pseudo_caches = SelectorCaches::default();

  // Arena order is DFS preorder, so one counting filter walked along the
  // ancestor chain replaces a per-node filter copy: pop-and-remove until the
  // stack top is this node's parent, match against strict-ancestor hashes
  // only, then push self before descending.
  for i in 0..node_count {
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
    let is_replaced = arena.nodes[i].node.is_replaced();

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

    for (source_order, &rule) in flattened_rules.iter().enumerate() {
      let mut best_element: Option<u32> = None;
      let mut best_before: Option<u32> = None;
      let mut best_after: Option<u32> = None;

      for (selector_index, selector) in rule.selectors().slice().iter().enumerate() {
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

    ancestor_stack.push(i);
    add_node_unique_hashes_to_filter(arena.nodes[i].node, &mut ancestor_bloom_filter);
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
  if !rule.normal_declarations.is_empty() {
    let normal_layer_order = rule.layer_order.unwrap_or(layer_count);
    bucket.push(MatchedRule {
      important: false,
      layer_order: normal_layer_order,
      specificity,
      source_order,
      declarations: &rule.normal_declarations,
    });
  }
  if !rule.important_declarations.is_empty() {
    let important_layer_order = rule.layer_order.map_or(0, |order| layer_count - order);
    bucket.push(MatchedRule {
      important: true,
      layer_order: important_layer_order,
      specificity,
      source_order,
      declarations: &rule.important_declarations,
    });
  }
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

/// RFC 4647 basic filtering, as `:lang()` uses: `range` matches `lang` if they're equal
/// (case-insensitive) or `lang` extends `range` with a `-` boundary (`zh` matches `zh-Hant`).
/// `*` matches any non-empty `lang`.
fn lang_matches(lang: &str, range: &str) -> bool {
  if range == "*" {
    return true;
  }
  lang.eq_ignore_ascii_case(range)
    || lang
      .get(..range.len())
      .is_some_and(|prefix| prefix.eq_ignore_ascii_case(range))
      && lang.as_bytes().get(range.len()) == Some(&b'-')
}
