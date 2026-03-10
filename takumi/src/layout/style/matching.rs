use std::{collections::HashMap, fmt};

use selectors::matching::{
  MatchingContext, MatchingForInvalidation, MatchingMode, NeedsSelectorFlags, QuirksMode,
  SelectorCaches, early_reject_by_local_name, matches_selector,
};
use selectors::{
  Element, OpaqueElement,
  attr::CaseSensitivity,
  bloom::BloomFilter,
  parser::{AncestorHashes, Component, Selector},
};

use crate::layout::style::StyleDeclarationBlock;
use crate::layout::{
  Viewport,
  node::Node,
  style::selector::{CssRule, StyleSheet, TakumiIdent, TakumiSelectorImpl},
};

#[derive(Default)]
struct SelectorSubjectHint {
  tag_name: Option<String>,
  id: Option<String>,
  classes: Vec<String>,
  has_positive_key: bool,
}

#[derive(Default)]
struct RuleSubjectHint {
  tags: Vec<String>,
  ids: Vec<String>,
  classes: Vec<String>,
  matches_any: bool,
}

#[derive(Default)]
struct RuleCandidateIndex {
  any_rules: Vec<usize>,
  by_tag: HashMap<String, Vec<usize>>,
  by_id: HashMap<String, Vec<usize>>,
  by_class: HashMap<String, Vec<usize>>,
}

fn selector_subject_hint(selector: &Selector<TakumiSelectorImpl>) -> SelectorSubjectHint {
  let mut hint = SelectorSubjectHint::default();

  for component in selector.iter_raw_match_order() {
    match component {
      Component::Combinator(_) => break,
      Component::LocalName(local_name) => {
        hint.tag_name = Some(local_name.lower_name.0.clone());
        hint.has_positive_key = true;
      }
      Component::ID(id) => {
        hint.id = Some(id.0.clone());
        hint.has_positive_key = true;
      }
      Component::Class(class_name) => {
        hint.classes.push(class_name.0.clone());
        hint.has_positive_key = true;
      }
      _ => {}
    }
  }

  hint
}

fn push_unique(vec: &mut Vec<String>, value: &str) {
  if !vec.iter().any(|existing| existing == value) {
    vec.push(value.to_owned());
  }
}

fn build_rule_subject_hint(rule: &CssRule) -> RuleSubjectHint {
  let mut hint = RuleSubjectHint::default();

  for selector in rule.selectors.slice() {
    let selector_hint = selector_subject_hint(selector);
    if !selector_hint.has_positive_key {
      hint.matches_any = true;
      continue;
    }

    if let Some(tag_name) = selector_hint.tag_name.as_deref() {
      push_unique(&mut hint.tags, tag_name);
    }
    if let Some(id) = selector_hint.id.as_deref() {
      push_unique(&mut hint.ids, id);
    }
    for class_name in &selector_hint.classes {
      push_unique(&mut hint.classes, class_name);
    }
  }

  hint
}

fn push_rule_index_entry(map: &mut HashMap<String, Vec<usize>>, key: &str, rule_index: usize) {
  map.entry(key.to_owned()).or_default().push(rule_index);
}

fn build_rule_candidate_index(rule_subject_hints: &[RuleSubjectHint]) -> RuleCandidateIndex {
  let mut index = RuleCandidateIndex::default();

  for (rule_index, hint) in rule_subject_hints.iter().enumerate() {
    if hint.matches_any || (hint.tags.is_empty() && hint.ids.is_empty() && hint.classes.is_empty())
    {
      index.any_rules.push(rule_index);
      continue;
    }

    for tag in &hint.tags {
      push_rule_index_entry(&mut index.by_tag, tag, rule_index);
    }
    for id in &hint.ids {
      push_rule_index_entry(&mut index.by_id, id, rule_index);
    }
    for class_name in &hint.classes {
      push_rule_index_entry(&mut index.by_class, class_name, rule_index);
    }
  }

  index
}

fn collect_candidate_rule_indices_for_node<N: Node<N>>(
  node: &N,
  index: &RuleCandidateIndex,
  candidate_rule_indices: &mut Vec<usize>,
  seen_rule_marks: &mut [u32],
  seen_generation: u32,
) {
  let mut push_if_new = |rule_index: usize| {
    let mark = &mut seen_rule_marks[rule_index];
    if *mark == seen_generation {
      return;
    }
    *mark = seen_generation;
    candidate_rule_indices.push(rule_index);
  };

  for &rule_index in &index.any_rules {
    push_if_new(rule_index);
  }

  if let Some(tag) = node.tag_name() {
    let tag = tag.to_ascii_lowercase();
    if let Some(rule_indices) = index.by_tag.get(&tag) {
      for &rule_index in rule_indices {
        push_if_new(rule_index);
      }
    }
  }

  if let Some(id) = node.id()
    && let Some(rule_indices) = index.by_id.get(id)
  {
    for &rule_index in rule_indices {
      push_if_new(rule_index);
    }
  }

  if let Some(classes) = node.class_name() {
    for class_name in classes.split_whitespace() {
      if let Some(rule_indices) = index.by_class.get(class_name) {
        for &rule_index in rule_indices {
          push_if_new(rule_index);
        }
      }
    }
  }
}

/// A transient arena for CSS matching.
/// It flattens the node tree into a vector of nodes and stores indices to parents, siblings, and children.
pub(crate) struct StyleArena<'a, N: Node<N>> {
  /// The flattened nodes in the arena.
  pub nodes: Vec<StyleNode<'a, N>>,
}
/// Represents a single node inside the `StyleArena`.
pub(crate) struct StyleNode<'a, N: Node<N>> {
  /// The actual node reference.
  pub node: &'a N,
  /// The index of the parent node, if any.
  pub parent: Option<usize>,
  /// The index of the previous sibling node, if any.
  pub prev_sibling: Option<usize>,
  /// The index of the next sibling node, if any.
  pub next_sibling: Option<usize>,
  /// The index of the first child node, if any.
  pub first_child: Option<usize>,
}
/// An element inside the `StyleArena` that can be matched against CSS selectors.
#[derive(Clone, Copy)]
pub(crate) struct ArenaElement<'a, N: Node<N>> {
  /// A reference to the parent arena.
  pub tree: &'a StyleArena<'a, N>,
  /// The index of this element in the arena.
  pub index: usize,
}

impl<N: Node<N>> fmt::Debug for ArenaElement<'_, N> {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.debug_struct("ArenaElement")
      .field("index", &self.index)
      .finish()
  }
}

impl<'a, N: Node<N>> StyleArena<'a, N> {
  /// Creates a new `StyleArena` from a given root node.
  pub fn new(root: &'a N) -> Self {
    let mut arena = StyleArena { nodes: Vec::new() };
    arena.add_node(root, None, None);
    arena
  }

  fn add_node(&mut self, node: &'a N, parent: Option<usize>, prev_sibling: Option<usize>) -> usize {
    struct ChildFrame<'a, N: Node<N>> {
      parent_index: usize,
      children: &'a [N],
      next_child: usize,
      current_prev: Option<usize>,
    }

    let root_index = self.push_node(node, parent, prev_sibling);
    let mut stack = Vec::new();

    if let Some(children) = node.children_ref() {
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

      if let Some(children) = child.children_ref() {
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

fn add_node_unique_hashes_to_filter<N: Node<N>>(node: &N, filter: &mut BloomFilter) -> bool {
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

impl<'a, N: Node<N>> Element for ArenaElement<'a, N> {
  type Impl = TakumiSelectorImpl;

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

  fn has_local_name(&self, local_name: &TakumiIdent) -> bool {
    let node = self.tree.nodes[self.index].node;
    if let Some(tag) = node.tag_name() {
      tag.eq_ignore_ascii_case(&local_name.0)
    } else {
      false
    }
  }

  fn has_namespace(&self, _ns: &TakumiIdent) -> bool {
    false
  }

  fn is_same_type(&self, other: &Self) -> bool {
    let my_tag = self.tree.nodes[self.index].node.tag_name();
    let other_tag = other.tree.nodes[other.index].node.tag_name();
    my_tag == other_tag
  }

  fn has_id(&self, id: &TakumiIdent, _case_sensitivity: CaseSensitivity) -> bool {
    let node = self.tree.nodes[self.index].node;
    node.id() == Some(id.0.as_str())
  }

  fn has_class(&self, name: &TakumiIdent, _case_sensitivity: CaseSensitivity) -> bool {
    let node = self.tree.nodes[self.index].node;
    if let Some(classes) = node.class_name() {
      classes.split_whitespace().any(|c| c == name.0.as_str())
    } else {
      false
    }
  }

  fn imported_part(&self, _name: &TakumiIdent) -> Option<TakumiIdent> {
    None
  }

  fn is_part(&self, _name: &TakumiIdent) -> bool {
    false
  }

  fn is_empty(&self) -> bool {
    self.tree.nodes[self.index].first_child.is_none()
  }

  fn is_root(&self) -> bool {
    self.tree.nodes[self.index].parent.is_none()
  }

  fn has_custom_state(&self, _name: &TakumiIdent) -> bool {
    false
  }

  fn attr_matches(
    &self,
    _ns: &selectors::attr::NamespaceConstraint<&TakumiIdent>,
    _local_name: &TakumiIdent,
    _operation: &selectors::attr::AttrSelectorOperation<&TakumiIdent>,
  ) -> bool {
    // TODO(#attr-selectors): implement CSS attribute selector matching.
    false
  }
  fn match_non_ts_pseudo_class(
    &self,
    _pc: &<Self::Impl as selectors::SelectorImpl>::NonTSPseudoClass,
    _context: &mut MatchingContext<'_, Self::Impl>,
  ) -> bool {
    false
  }
  fn match_pseudo_element(
    &self,
    _pe: &<Self::Impl as selectors::SelectorImpl>::PseudoElement,
    _context: &mut MatchingContext<'_, Self::Impl>,
  ) -> bool {
    false
  }

  fn apply_selector_flags(&self, _flags: selectors::matching::ElementSelectorFlags) {}
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

#[derive(Debug, Default, Clone)]
pub(crate) struct MatchedDeclarations {
  pub(crate) normal: StyleDeclarationBlock,
  pub(crate) important: StyleDeclarationBlock,
}

#[derive(Debug, Clone, Copy)]
struct MatchedRule<'a> {
  important: bool,
  layer_order: usize,
  specificity: u32,
  source_order: usize,
  declarations: &'a StyleDeclarationBlock,
}

pub(crate) fn match_stylesheets<N: Node<N>>(
  root: &N,
  stylesheets: &[StyleSheet],
  viewport: Viewport,
) -> Vec<MatchedDeclarations> {
  let arena = StyleArena::new(root);
  let mut per_node = vec![MatchedDeclarations::default(); arena.nodes.len()];

  if stylesheets.is_empty() {
    return per_node;
  }

  let mut matched_rules: Vec<Vec<MatchedRule<'_>>> = vec![Vec::new(); arena.nodes.len()];
  let mut ancestor_bloom_filters = vec![BloomFilter::new(); arena.nodes.len()];
  let mut selector_ancestor_hashes_cache: HashMap<usize, AncestorHashes> = HashMap::new();
  let flattened_rules: Vec<&CssRule> = stylesheets
    .iter()
    .flat_map(|sheet| sheet.rules.iter())
    .filter(|rule| {
      rule
        .media_queries
        .iter()
        .all(|media_queries| media_queries.matches(viewport))
    })
    .collect();
  let layer_count = flattened_rules
    .iter()
    .filter_map(|rule| rule.layer_order)
    .max()
    .map_or(0, |max_order| max_order + 1);
  let rule_subject_hints: Vec<RuleSubjectHint> = flattened_rules
    .iter()
    .map(|rule| build_rule_subject_hint(rule))
    .collect();
  let rule_candidate_index = build_rule_candidate_index(&rule_subject_hints);
  let mut candidate_rule_indices = Vec::new();
  let mut seen_rule_marks = vec![0u32; flattened_rules.len()];
  let mut seen_generation = 1u32;

  for i in 0..arena.nodes.len() {
    let Some(parent) = arena.nodes[i].parent else {
      continue;
    };
    ancestor_bloom_filters[i] = ancestor_bloom_filters[parent].clone();
    add_node_unique_hashes_to_filter(arena.nodes[parent].node, &mut ancestor_bloom_filters[i]);
  }

  let mut caches = SelectorCaches::default();

  for (i, matched_rule) in matched_rules.iter_mut().enumerate() {
    let element = ArenaElement {
      tree: &arena,
      index: i,
    };
    let mut ctx = MatchingContext::new(
      MatchingMode::Normal,
      Some(&ancestor_bloom_filters[i]),
      &mut caches,
      QuirksMode::NoQuirks,
      NeedsSelectorFlags::No,
      MatchingForInvalidation::No,
    );
    collect_candidate_rule_indices_for_node(
      arena.nodes[i].node,
      &rule_candidate_index,
      &mut candidate_rule_indices,
      &mut seen_rule_marks,
      seen_generation,
    );
    candidate_rule_indices.sort_unstable();

    for &source_order in &candidate_rule_indices {
      let rule = flattened_rules[source_order];
      let mut best_specificity: Option<u32> = None;
      for selector in rule.selectors.slice() {
        let selector_key = selector as *const _ as usize;
        let ancestor_hashes = selector_ancestor_hashes_cache
          .entry(selector_key)
          .or_insert_with(|| AncestorHashes::new(selector, QuirksMode::NoQuirks));
        let is_match = if early_reject_by_local_name(selector, 0, &element) {
          false
        } else {
          matches_selector(selector, 0, Some(ancestor_hashes), &element, &mut ctx)
        };

        if is_match {
          let specificity = selector.specificity();
          best_specificity =
            Some(best_specificity.map_or(specificity, |best| best.max(specificity)));
        }
      }

      if let Some(specificity) = best_specificity {
        let normal_layer_order = rule.layer_order.map_or(layer_count, |order| order);
        matched_rule.push(MatchedRule {
          important: false,
          layer_order: normal_layer_order,
          specificity,
          source_order,
          declarations: &rule.normal_declarations,
        });
        let important_layer_order = rule.layer_order.map_or(0, |order| layer_count - order);
        matched_rule.push(MatchedRule {
          important: true,
          layer_order: important_layer_order,
          specificity,
          source_order,
          declarations: &rule.important_declarations,
        });
      }
    }

    candidate_rule_indices.clear();
    seen_generation = seen_generation.wrapping_add(1);
    if seen_generation == 0 {
      seen_rule_marks.fill(0);
      seen_generation = 1;
    }
  }

  for (matched, mut rules) in per_node.iter_mut().zip(matched_rules.into_iter()) {
    rules.sort_by_key(|rule| {
      (
        rule.important,
        rule.layer_order,
        rule.specificity,
        rule.source_order,
      )
    });

    for rule in rules {
      if rule.important {
        matched.important.append(rule.declarations.clone());
      } else {
        matched.normal.append(rule.declarations.clone());
      }
    }
  }

  per_node
}

#[cfg(test)]
mod tests {
  use super::match_stylesheets;
  use crate::layout::style::selector::StyleSheet;
  use crate::layout::{
    Viewport,
    node::{Node, NodeStyleLayers},
    style::{ComputedStyle, Length, Style},
  };

  #[derive(Clone, Default)]
  struct TestNode {
    class_name: Option<&'static str>,
    id: Option<&'static str>,
    children: Vec<TestNode>,
    style: Style,
  }

  impl Node<TestNode> for TestNode {
    fn class_name(&self) -> Option<&str> {
      self.class_name
    }

    fn id(&self) -> Option<&str> {
      self.id
    }

    fn children_ref(&self) -> Option<&[TestNode]> {
      Some(&self.children)
    }

    fn get_style(&self) -> Option<&Style> {
      Some(&self.style)
    }

    fn take_style_layers(&mut self) -> NodeStyleLayers {
      NodeStyleLayers::default()
    }
  }

  fn computed_width_from_matches(matches: &super::MatchedDeclarations) -> Length {
    let mut style = Style::default();
    for declaration in matches.normal.iter() {
      declaration.merge_into_ref(&mut style);
    }
    for declaration in matches.important.iter() {
      declaration.merge_into_ref(&mut style);
    }
    style.inherit(&ComputedStyle::default()).width
  }

  #[test]
  fn layered_rules_outrank_source_order() {
    let root = TestNode {
      class_name: Some("card"),
      ..TestNode::default()
    };
    let stylesheet = StyleSheet::parse(
      r#"
        @layer theme, base;
        @layer base {
          .card { width: 10px; }
        }
        @layer theme {
          .card { width: 20px; }
        }
      "#,
    );

    let matched = match_stylesheets(&root, &[stylesheet], Viewport::new(None, None));
    assert_eq!(matched.len(), 1);
    assert_eq!(computed_width_from_matches(&matched[0]), Length::Px(10.0));
  }

  #[test]
  fn nested_selector_uses_parent_list_specificity() {
    let root = TestNode {
      class_name: Some("card notice"),
      children: vec![TestNode {
        class_name: Some("title"),
        ..TestNode::default()
      }],
      ..TestNode::default()
    };
    let stylesheet = StyleSheet::parse(
      r#"
        .card, #panel {
          .title { width: 10px; }
        }

        .notice .title { width: 20px; }
      "#,
    );

    let matched = match_stylesheets(&root, &[stylesheet], Viewport::new(None, None));
    assert_eq!(matched.len(), 2);
    assert_eq!(computed_width_from_matches(&matched[1]), Length::Px(10.0));
  }
}
