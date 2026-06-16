use std::{collections::HashMap, fmt};

use selectors::{
  Element, OpaqueElement, SelectorImpl as SelectorImplTrait,
  attr::CaseSensitivity,
  bloom::BloomFilter,
  matching::{
    MatchingContext, MatchingForInvalidation, MatchingMode, NeedsSelectorFlags, QuirksMode,
    SelectorCaches, early_reject_by_local_name, matches_selector,
  },
  parser::AncestorHashes,
};
use smallvec::SmallVec;

use crate::layout::{
  Viewport,
  node::Node,
  style::{
    StyleDeclarationBlock,
    selector::{CssRule, Ident, SelectorImpl, StyleSheet},
  },
};

pub struct StyleArena<'a> {
  pub nodes: Vec<StyleNode<'a>>,
}
pub struct StyleNode<'a> {
  pub node: &'a Node,
  pub parent: Option<usize>,
  pub prev_sibling: Option<usize>,
  pub next_sibling: Option<usize>,
  pub first_child: Option<usize>,
}
#[derive(Clone, Copy)]
pub struct ArenaElement<'a> {
  pub(crate) tree: &'a StyleArena<'a>,
  pub(crate) index: usize,
}

impl fmt::Debug for ArenaElement<'_> {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.debug_struct("ArenaElement")
      .field("index", &self.index)
      .finish()
  }
}

impl<'a> StyleArena<'a> {
  pub fn new(root: &'a Node) -> Self {
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

  if let Some(tag) = node.metadata.tag_name.as_deref() {
    filter.insert_hash(hash_ascii_case_insensitive(tag));
    added = true;
  }

  if let Some(id) = node.metadata.id.as_deref() {
    filter.insert_hash(hash_ascii_case_insensitive(id));
    added = true;
  }

  if let Some(classes) = node.metadata.class_name.as_deref() {
    for class_name in classes.split_whitespace() {
      filter.insert_hash(hash_ascii_case_insensitive(class_name));
      added = true;
    }
  }

  added
}

impl<'a> Element for ArenaElement<'a> {
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
    if let Some(tag) = node.metadata.tag_name.as_deref() {
      tag.eq_ignore_ascii_case(local_name)
    } else {
      false
    }
  }

  fn has_namespace(&self, _ns: &Ident) -> bool {
    false
  }

  fn is_same_type(&self, other: &Self) -> bool {
    let my_tag = self.tree.nodes[self.index]
      .node
      .metadata
      .tag_name
      .as_deref();
    let other_tag = other.tree.nodes[other.index]
      .node
      .metadata
      .tag_name
      .as_deref();
    my_tag == other_tag
  }

  fn has_id(&self, id: &Ident, _case_sensitivity: CaseSensitivity) -> bool {
    let node = self.tree.nodes[self.index].node;
    node.metadata.id.as_deref() == Some(&**id)
  }

  fn has_class(&self, name: &Ident, _case_sensitivity: CaseSensitivity) -> bool {
    let node = self.tree.nodes[self.index].node;
    if let Some(classes) = node.metadata.class_name.as_deref() {
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
    ns: &selectors::attr::NamespaceConstraint<&Ident>,
    local_name: &Ident,
    operation: &selectors::attr::AttrSelectorOperation<&Ident>,
  ) -> bool {
    let namespace_supported = match ns {
      selectors::attr::NamespaceConstraint::Any => true,
      selectors::attr::NamespaceConstraint::Specific(url) => url.is_empty(),
    };
    if !namespace_supported {
      return false;
    }

    self.tree.nodes[self.index]
      .node
      .attribute(local_name)
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
pub struct MatchedDeclarationsView<'a> {
  pub normal: SmallVec<[&'a StyleDeclarationBlock; 4]>,
  pub important: SmallVec<[&'a StyleDeclarationBlock; 4]>,
}

/// Per-node matching result: the element's own declarations plus declarations
/// for any matched `::before` / `::after` pseudo-elements.
#[derive(Debug, Default, Clone)]
pub struct NodeMatchedDeclarations<'a> {
  pub element: MatchedDeclarationsView<'a>,
  pub before: Option<MatchedDeclarationsView<'a>>,
  pub after: Option<MatchedDeclarationsView<'a>>,
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

pub fn match_stylesheets_view<'a>(
  root: &Node,
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
    let is_replaced = arena.nodes[i].node.is_replaced_element();

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

      for selector in rule.selectors.slice() {
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

fn selector_target(selector: &selectors::parser::Selector<SelectorImpl>) -> Option<SelectorTarget> {
  match selector.pseudo_element() {
    None => Some(SelectorTarget::Element),
    Some(crate::layout::style::selector::PseudoElement::Before) => Some(SelectorTarget::Before),
    Some(crate::layout::style::selector::PseudoElement::After) => Some(SelectorTarget::After),
    Some(crate::layout::style::selector::PseudoElement::Other(_)) => None,
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

#[cfg(test)]
mod tests {
  use std::collections::BTreeMap;

  use super::{MatchedDeclarationsView, match_stylesheets_view};
  use crate::layout::{
    Viewport,
    node::Node,
    style::{ComputedStyle, Length, Style, StyleSheet},
  };

  fn container_with_class(class_name: &str) -> Node {
    Node::container([]).with_class_name(class_name)
  }

  fn computed_width_from_matches(matches: &MatchedDeclarationsView<'_>) -> Length {
    let mut style = Style::default();
    for &declarations in &matches.normal {
      for declaration in declarations.iter() {
        declaration.merge_into_ref(&mut style);
      }
    }
    for &declarations in &matches.important {
      for declaration in declarations.iter() {
        declaration.merge_into_ref(&mut style);
      }
    }
    style.inherit(&ComputedStyle::default()).width
  }

  fn computed_height_from_matches(matches: &MatchedDeclarationsView<'_>) -> Length {
    let mut style = Style::default();
    for &declarations in &matches.normal {
      for declaration in declarations.iter() {
        declaration.merge_into_ref(&mut style);
      }
    }
    for &declarations in &matches.important {
      for declaration in declarations.iter() {
        declaration.merge_into_ref(&mut style);
      }
    }
    style.inherit(&ComputedStyle::default()).height
  }

  fn parse_stylesheet(css: &str) -> StyleSheet {
    let result = StyleSheet::parse(css);
    assert!(result.is_ok(), "expected stylesheet to parse: {result:?}");
    result.unwrap_or_default()
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
    result.unwrap_or_default()
  }

  #[test]
  fn layered_rules_outrank_source_order() {
    let root = container_with_class("card");
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

    let matched = match_stylesheets_view(&root, &stylesheet, Viewport::default());
    assert_eq!(matched.len(), 1);
    assert_eq!(
      computed_width_from_matches(&matched[0].element),
      Length::Px(10.0)
    );
  }

  #[test]
  fn nested_selector_uses_parent_list_specificity() {
    let root = Node::container([container_with_class("title")]).with_class_name("card notice");

    let stylesheet = parse_stylesheet(
      r#"
        .card, #panel {
          .title { width: 10px; }
        }

        .notice .title { width: 20px; }
      "#,
    );

    let matched = match_stylesheets_view(&root, &stylesheet, Viewport::default());
    assert_eq!(matched.len(), 2);
    assert_eq!(
      computed_width_from_matches(&matched[1].element),
      Length::Px(10.0)
    );
  }

  #[test]
  fn important_layered_rules_outrank_unlayered_important() {
    let root = container_with_class("card");
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

    let matched = match_stylesheets_view(&root, &stylesheet, Viewport::default());
    assert_eq!(matched.len(), 1);
    assert_eq!(
      computed_width_from_matches(&matched[0].element),
      Length::Px(20.0)
    );
  }

  #[test]
  fn later_stylesheet_rules_outrank_earlier_stylesheets_on_ties() {
    let root = container_with_class("card");
    let stylesheet = parse_stylesheet(".card { width: 10px; } .card { width: 20px; }");

    let matched = match_stylesheets_view(&root, &stylesheet, Viewport::default());
    assert_eq!(matched.len(), 1);
    assert_eq!(
      computed_width_from_matches(&matched[0].element),
      Length::Px(20.0)
    );
  }

  #[test]
  fn parse_list_preserves_cross_stylesheet_layer_order() {
    let root = container_with_class("card");
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

    let matched = match_stylesheets_view(&root, &stylesheet, Viewport::default());
    assert_eq!(matched.len(), 1);
    assert_eq!(
      computed_width_from_matches(&matched[0].element),
      Length::Px(10.0)
    );
  }

  #[test]
  fn root_selector_list_with_host_keeps_matching_root() {
    let root = Node::default();
    let stylesheet = parse_stylesheet(
      r#"
        :root, :host {
          width: 10px;
        }
      "#,
    );

    let matched = match_stylesheets_view(&root, &stylesheet, Viewport::default());
    assert_eq!(matched.len(), 1);
    assert_eq!(
      computed_width_from_matches(&matched[0].element),
      Length::Px(10.0)
    );
  }

  #[test]
  fn sibling_combinators_only_match_the_correct_siblings() {
    let root = Node::container([
      container_with_class("lead"),
      container_with_class("title"),
      container_with_class("spacer"),
      container_with_class("title"),
    ])
    .with_class_name("container");
    let stylesheet = parse_stylesheet(
      r#"
        .container .title { width: 20px; }
        .lead + .title { width: 10px; }
        .lead ~ .title { height: 30px; }
      "#,
    );

    let matched = match_stylesheets_view(&root, &stylesheet, Viewport::default());
    assert_eq!(matched.len(), 5);
    assert_eq!(
      computed_width_from_matches(&matched[2].element),
      Length::Px(10.0)
    );
    assert_eq!(
      computed_height_from_matches(&matched[2].element),
      Length::Px(30.0)
    );
    assert_eq!(
      computed_width_from_matches(&matched[4].element),
      Length::Px(20.0)
    );
    assert_eq!(
      computed_height_from_matches(&matched[4].element),
      Length::Px(30.0)
    );
  }

  #[test]
  fn attribute_selectors_match_node_metadata_and_attributes() {
    let root = Node::container([Node::container([])
      .with_id("hero")
      .with_class_name("card featured")
      .with_attributes(BTreeMap::from([
        (Box::<str>::from("data-kind"), Box::<str>::from("promo")),
        (
          Box::<str>::from("data-state"),
          Box::<str>::from("ready now"),
        ),
      ]))]);
    let stylesheet = parse_stylesheet(
      r#"
        [id="hero"] { width: 10px; }
        [class~="featured"] { height: 20px; }
        [data-kind="promo"] { width: 30px; }
        [data-state~="ready"] { height: 40px; }
      "#,
    );

    let matched = match_stylesheets_view(&root, &stylesheet, Viewport::default());
    assert_eq!(matched.len(), 2);
    assert_eq!(
      computed_width_from_matches(&matched[1].element),
      Length::Px(30.0)
    );
    assert_eq!(
      computed_height_from_matches(&matched[1].element),
      Length::Px(40.0)
    );
  }

  #[test]
  fn test_repro_mixed_importance_bug() {
    let stylesheet = parse_stylesheet(".test { width: 10px; height: 20px !important; }");
    let root = Node::container([]).with_class_name("test");
    let matched = match_stylesheets_view(&root, &stylesheet, Viewport::default());

    // Matched normal: should have width: 10px.
    // Matched important: should have height: 20px.

    assert_eq!(matched[0].element.normal[0].declarations.len(), 1);
    assert!(matched[0].element.normal[0].importance.is_empty());

    assert_eq!(matched[0].element.important[0].declarations.len(), 1);
    assert!(!matched[0].element.important[0].importance.is_empty());
  }

  #[test]
  fn pseudo_element_rules_land_in_before_after_buckets() {
    let root = Node::container([]).with_class_name("card");
    let stylesheet = parse_stylesheet(
      r#"
        .card::before { content: "x"; }
        .card::after  { content: "y"; }
        .card         { width: 10px; }
      "#,
    );

    let matched = match_stylesheets_view(&root, &stylesheet, Viewport::default());
    assert_eq!(matched.len(), 1);

    assert!(!matched[0].element.normal.is_empty());
    assert!(matched[0].before.is_some());
    assert!(matched[0].after.is_some());
  }

  #[test]
  fn pseudo_element_rules_are_skipped_on_replaced_elements() {
    let root = Node::image("data:image/png;base64,iVBOR").with_class_name("logo");
    let stylesheet = parse_stylesheet(r#".logo::before { content: "x"; }"#);

    let matched = match_stylesheets_view(&root, &stylesheet, Viewport::default());
    assert_eq!(matched.len(), 1);
    assert!(matched[0].before.is_none());
    assert!(matched[0].after.is_none());
  }

  #[test]
  fn unsupported_pseudo_element_does_not_create_bucket() {
    let root = Node::container([]).with_class_name("card");
    let stylesheet = parse_stylesheet(r#".card::placeholder { color: red; }"#);

    let matched = match_stylesheets_view(&root, &stylesheet, Viewport::default());
    assert_eq!(matched.len(), 1);
    assert!(matched[0].before.is_none());
    assert!(matched[0].after.is_none());
  }
}
