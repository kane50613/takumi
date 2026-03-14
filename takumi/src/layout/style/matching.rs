use std::{collections::HashMap, fmt};

use selectors::{
  Element, OpaqueElement, SelectorImpl,
  attr::CaseSensitivity,
  bloom::BloomFilter,
  matching::{
    MatchingContext, MatchingForInvalidation, MatchingMode, NeedsSelectorFlags, QuirksMode,
    SelectorCaches, early_reject_by_local_name, matches_selector,
  },
  parser::AncestorHashes,
};

use crate::layout::{
  Viewport,
  node::Node,
  style::{
    StyleDeclarationBlock,
    selector::{CssRule, StyleSheet, TakumiIdent, TakumiSelectorImpl},
  },
};

pub(crate) struct StyleArena<'a, N: Node<N>> {
  pub nodes: Vec<StyleNode<'a, N>>,
}
pub(crate) struct StyleNode<'a, N: Node<N>> {
  pub node: &'a N,
  pub parent: Option<usize>,
  pub prev_sibling: Option<usize>,
  pub next_sibling: Option<usize>,
  pub first_child: Option<usize>,
}
#[derive(Clone, Copy)]
pub(crate) struct ArenaElement<'a, N: Node<N>> {
  pub tree: &'a StyleArena<'a, N>,
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
      tag.eq_ignore_ascii_case(local_name)
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
    node.id() == Some(id)
  }

  fn has_class(&self, name: &TakumiIdent, _case_sensitivity: CaseSensitivity) -> bool {
    let node = self.tree.nodes[self.index].node;
    if let Some(classes) = node.class_name() {
      classes.split_whitespace().any(|c| c == *name)
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
    pc: &<Self::Impl as SelectorImpl>::NonTSPseudoClass,
    _context: &mut MatchingContext<'_, Self::Impl>,
  ) -> bool {
    match *pc {}
  }
  fn match_pseudo_element(
    &self,
    pe: &<Self::Impl as SelectorImpl>::PseudoElement,
    _context: &mut MatchingContext<'_, Self::Impl>,
  ) -> bool {
    match *pe {}
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
  stylesheet: &StyleSheet,
  viewport: Viewport,
) -> Vec<MatchedDeclarations> {
  let arena = StyleArena::new(root);
  let mut per_node = vec![MatchedDeclarations::default(); arena.nodes.len()];

  let mut matched_rules: Vec<Vec<MatchedRule<'_>>> = vec![Vec::new(); arena.nodes.len()];
  let mut ancestor_bloom_filters = vec![BloomFilter::new(); arena.nodes.len()];
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
  let layer_count = flattened_rules
    .iter()
    .filter_map(|rule| rule.layer_order)
    .max()
    .map_or(0, |max_order| max_order + 1);

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

    for (source_order, rule) in flattened_rules.iter().copied().enumerate() {
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
  use crate::layout::style::StyleSheet;
  use crate::layout::{
    Viewport,
    node::{Node, NodeMetadata, NodeStyleLayers},
    style::{ComputedStyle, Length, Style},
  };

  #[derive(Clone, Default)]
  struct TestNode {
    metadata: NodeMetadata,
    children: Vec<TestNode>,
  }

  impl TestNode {
    fn with_children(mut self, children: Vec<TestNode>) -> Self {
      self.children = children;
      self
    }
  }

  impl Node<TestNode> for TestNode {
    fn metadata(&self) -> &NodeMetadata {
      &self.metadata
    }

    fn metadata_mut(&mut self) -> &mut NodeMetadata {
      &mut self.metadata
    }

    fn children_ref(&self) -> Option<&[TestNode]> {
      Some(&self.children)
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

  fn computed_height_from_matches(matches: &super::MatchedDeclarations) -> Length {
    let mut style = Style::default();
    for declaration in matches.normal.iter() {
      declaration.merge_into_ref(&mut style);
    }
    for declaration in matches.important.iter() {
      declaration.merge_into_ref(&mut style);
    }
    style.inherit(&ComputedStyle::default()).height
  }

  fn parse_stylesheet(css: &str) -> StyleSheet {
    let result = StyleSheet::parse(css);
    assert!(result.is_ok(), "expected stylesheet to parse: {result:?}");
    let Ok(stylesheet) = result else {
      unreachable!();
    };
    stylesheet
  }

  fn parse_stylesheet_list<I, S>(stylesheets: I) -> StyleSheet
  where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
  {
    let result = StyleSheet::parse_list(stylesheets);
    assert!(
      result.is_ok(),
      "expected stylesheet list to parse: {result:?}"
    );
    let Ok(stylesheet) = result else {
      unreachable!();
    };
    stylesheet
  }

  #[test]
  fn layered_rules_outrank_source_order() {
    let root = TestNode::default().with_class_name("card");
    let stylesheet = parse_stylesheet(
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

    let matched = match_stylesheets(&root, &stylesheet, Viewport::new(None, None));
    assert_eq!(matched.len(), 1);
    assert_eq!(computed_width_from_matches(&matched[0]), Length::Px(10.0));
  }

  #[test]
  fn nested_selector_uses_parent_list_specificity() {
    let root = TestNode::default()
      .with_class_name("card notice")
      .with_children(vec![TestNode::default().with_class_name("title")]);

    let stylesheet = parse_stylesheet(
      r#"
        .card, #panel {
          .title { width: 10px; }
        }

        .notice .title { width: 20px; }
      "#,
    );

    let matched = match_stylesheets(&root, &stylesheet, Viewport::new(None, None));
    assert_eq!(matched.len(), 2);
    assert_eq!(computed_width_from_matches(&matched[1]), Length::Px(10.0));
  }

  #[test]
  fn important_layered_rules_outrank_unlayered_important() {
    let root = TestNode::default().with_class_name("card");
    let stylesheet = parse_stylesheet(
      r#"
        @layer theme, base;
        .card { width: 5px !important; }
        @layer base {
          .card { width: 10px !important; }
        }
        @layer theme {
          .card { width: 20px !important; }
        }
      "#,
    );

    let matched = match_stylesheets(&root, &stylesheet, Viewport::new(None, None));
    assert_eq!(matched.len(), 1);
    assert_eq!(computed_width_from_matches(&matched[0]), Length::Px(20.0));
  }

  #[test]
  fn later_stylesheet_rules_outrank_earlier_stylesheets_on_ties() {
    let root = TestNode::default().with_class_name("card");
    let stylesheet = parse_stylesheet(".card { width: 10px; } .card { width: 20px; }");

    let matched = match_stylesheets(&root, &stylesheet, Viewport::new(None, None));
    assert_eq!(matched.len(), 1);
    assert_eq!(computed_width_from_matches(&matched[0]), Length::Px(20.0));
  }

  #[test]
  fn parse_list_preserves_cross_stylesheet_layer_order() {
    let root = TestNode::default().with_class_name("card");
    let stylesheet = parse_stylesheet_list([
      r#"
        @layer theme, base;
        @layer base {
          .card { width: 10px; }
        }
      "#,
      r#"
        @layer theme {
          .card { width: 20px; }
        }
      "#,
    ]);

    let matched = match_stylesheets(&root, &stylesheet, Viewport::new(None, None));
    assert_eq!(matched.len(), 1);
    assert_eq!(computed_width_from_matches(&matched[0]), Length::Px(10.0));
  }

  #[test]
  fn root_selector_list_with_host_keeps_matching_root() {
    let root = TestNode::default();
    let stylesheet = parse_stylesheet(
      r#"
        :root, :host {
          width: 10px;
        }
      "#,
    );

    let matched = match_stylesheets(&root, &stylesheet, Viewport::new(None, None));
    assert_eq!(matched.len(), 1);
    assert_eq!(computed_width_from_matches(&matched[0]), Length::Px(10.0));
  }

  #[test]
  fn sibling_combinators_only_match_the_correct_siblings() {
    let root = TestNode {
      children: vec![
        TestNode::default().with_class_name("lead"),
        TestNode::default().with_class_name("title"),
        TestNode::default().with_class_name("spacer"),
        TestNode::default().with_class_name("title"),
      ],
      ..TestNode::default().with_class_name("container")
    };
    let stylesheet = parse_stylesheet(
      r#"
        .container .title { width: 20px; }
        .lead + .title { width: 10px; }
        .lead ~ .title { height: 30px; }
      "#,
    );

    let matched = match_stylesheets(&root, &stylesheet, Viewport::new(None, None));
    assert_eq!(matched.len(), 5);
    assert_eq!(computed_width_from_matches(&matched[2]), Length::Px(10.0));
    assert_eq!(computed_height_from_matches(&matched[2]), Length::Px(30.0));
    assert_eq!(computed_width_from_matches(&matched[4]), Length::Px(20.0));
    assert_eq!(computed_height_from_matches(&matched[4]), Length::Px(30.0));
  }
}
