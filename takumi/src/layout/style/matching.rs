use std::fmt;

use selectors::matching::{
  MatchingContext, MatchingForInvalidation, MatchingMode, NeedsSelectorFlags, QuirksMode,
  SelectorCaches, matches_selector,
};
use selectors::{Element, OpaqueElement, attr::CaseSensitivity, bloom::BloomStorageU8};

use crate::layout::{
  node::Node,
  style::{
    Style,
    selector::{StyleSheet, TakumiIdent, TakumiSelectorImpl},
  },
};

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
    true
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
  fn add_element_unique_hashes(
    &self,
    _filter: &mut selectors::bloom::CountingBloomFilter<BloomStorageU8>,
  ) -> bool {
    false
  }
}

#[derive(Debug, Default, Clone)]
pub(crate) struct MatchedAuthorStyles {
  pub(crate) stylesheet: Style,
}

#[derive(Debug, Default, Clone)]
pub(crate) struct MatchedStyles {
  pub(crate) per_node: Vec<MatchedAuthorStyles>,
}

pub(crate) fn match_stylesheets_for_tree<N: Node<N>>(
  root: &N,
  stylesheets: &[StyleSheet],
) -> MatchedStyles {
  let arena = StyleArena::new(root);
  let mut per_node = vec![MatchedAuthorStyles::default(); arena.nodes.len()];
  if stylesheets.is_empty() {
    return MatchedStyles { per_node };
  }
  let mut matched_rules: Vec<Vec<(u32, usize, &Style)>> = vec![Vec::new(); arena.nodes.len()];

  let mut caches = SelectorCaches::default();
  let mut ctx = MatchingContext::new(
    MatchingMode::Normal,
    None,
    &mut caches,
    QuirksMode::NoQuirks,
    NeedsSelectorFlags::No,
    MatchingForInvalidation::No,
  );

  let mut source_order = 0usize;
  for sheet in stylesheets {
    for rule in &sheet.rules {
      for (i, matched_rule) in matched_rules.iter_mut().enumerate() {
        let element = ArenaElement {
          tree: &arena,
          index: i,
        };

        for selector in rule.selectors.slice().iter() {
          if matches_selector(selector, 0, None, &element, &mut ctx) {
            matched_rule.push((selector.specificity(), source_order, &rule.style));
          }
        }
      }

      source_order += 1;
    }
  }

  for (matched, rules) in per_node.iter_mut().zip(matched_rules.into_iter()) {
    let mut rules = rules;
    rules.sort_by_key(|(specificity, order, _)| (*specificity, *order));
    for (_, _, style) in rules {
      matched.stylesheet.merge_from(style.clone());
    }
  }

  MatchedStyles { per_node }
}
