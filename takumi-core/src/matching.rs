//! Selector matching over the [`Node`] tree, keeping the `selectors` crate out of the public API.

use std::{cell::OnceCell, collections::HashMap, fmt, iter::successors, ops::Deref, sync::Arc};

use cssparser::match_ignore_ascii_case;
use rustc_hash::FxHashMap;
use selectors::{
  Element, OpaqueElement, SelectorImpl as SelectorImplTrait,
  attr::{CaseSensitivity, NamespaceConstraint},
  bloom::BloomFilter,
  matching::*,
  parser::{AncestorHashes, Combinator, Component, Selector},
};
use smallvec::SmallVec;

use crate::{
  form_control::{
    InputType, has_attribute, is_disableable_form_control, is_element, is_mutable_text_control,
    is_placeholder_shown, required_state,
  },
  layout::node::{Node, NodeKind},
  sort_key::sort_by_key,
  style::{
    BreakpointOverrides, Direction, LonghandId, MediaQueryList, StyleDeclarationBlock, TwRule,
    selector::{CssRule, FormState, Ident, PseudoClass, PseudoElement, SelectorImpl, StyleSheet},
  },
  viewport::Viewport,
};

struct StyleArena<'a> {
  nodes: Vec<StyleNode<'a>>,
  /// Built on the first form state query, so a match pass reads the tree for it once.
  form_controls: OnceCell<FormControls>,
}

/// Checkedness and selectedness the whole tree decides, by arena index.
#[derive(Default)]
struct FormControls {
  /// The group of each radio button.
  radio_groups: FxHashMap<usize, usize>,
  /// The radio button each group keeps checked, the last one the markup checks.
  checked_radios: Vec<Option<usize>>,
  /// The option each single-selection `<select>` shows selected.
  selected_options: FxHashMap<usize, Option<usize>>,
  /// The first `<legend>` child of each `<fieldset>`.
  first_legends: FxHashMap<usize, usize>,
}

impl FormControls {
  fn new(arena: &StyleArena<'_>) -> Self {
    let mut controls = Self::default();
    let mut form_owners: Vec<Option<usize>> = Vec::with_capacity(arena.nodes.len());
    let mut named_groups: FxHashMap<(Option<usize>, &str), usize> = FxHashMap::default();

    for (index, style_node) in arena.nodes.iter().enumerate() {
      // Tree order puts every parent before its children.
      let form_owner = style_node.parent.and_then(|parent| {
        if is_element(arena.nodes[parent].node, "form") {
          Some(parent)
        } else {
          form_owners[parent]
        }
      });
      let node = style_node.node;

      form_owners.push(form_owner);

      if let Some(parent) = style_node.parent
        && is_element(node, "legend")
        && is_element(arena.nodes[parent].node, "fieldset")
      {
        controls.first_legends.entry(parent).or_insert(index);
      }

      if InputType::of(node) == Some(InputType::Radio) {
        let next_group = controls.checked_radios.len();
        let group = match node.attribute("name").filter(|name| !name.is_empty()) {
          Some(name) => *named_groups.entry((form_owner, name)).or_insert(next_group),
          None => next_group,
        };

        if group == next_group {
          controls.checked_radios.push(None);
        }

        if has_attribute(node, "checked") {
          controls.checked_radios[group] = Some(index);
        }

        controls.radio_groups.insert(index, group);
      } else if is_element(node, "select") && !has_attribute(node, "multiple") {
        let select = ArenaElement { tree: arena, index };

        controls
          .selected_options
          .insert(index, select.selected_option());
      }
    }

    controls
  }

  /// The radio button its group keeps checked, if any.
  fn checked_radio(&self, radio: usize) -> Option<usize> {
    self
      .radio_groups
      .get(&radio)
      .and_then(|&group| self.checked_radios[group])
  }
}
struct StyleNode<'a> {
  node: &'a Node,
  /// The node's classes, split once for every selector that reads them.
  classes: Box<[&'a str]>,
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
    let mut arena = StyleArena {
      nodes: Vec::new(),
      form_controls: OnceCell::new(),
    };
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
      classes: node.classes().collect(),
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
    .chain(node.classes())
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

  fn node(&self) -> &'a Node {
    self.style_node().node
  }

  fn matches_form_state(&self, state: FormState) -> bool {
    let node = self.node();

    match state {
      FormState::Disabled => self.is_actually_disabled(),
      FormState::Enabled => {
        (is_disableable_form_control(node)
          || ["fieldset", "optgroup", "option"]
            .iter()
            .any(|tag| is_element(node, tag)))
          && !self.is_actually_disabled()
      }
      FormState::Checked => self.is_checked(),
      FormState::Indeterminate => match InputType::of(node) {
        Some(InputType::Radio) => self.form_controls().checked_radio(self.index).is_none(),
        _ => is_element(node, "progress") && !has_attribute(node, "value"),
      },
      FormState::PlaceholderShown => is_placeholder_shown(node),
      FormState::ReadWrite => self.is_read_write(),
      FormState::ReadOnly => !self.is_read_write(),
      FormState::Required => required_state(node) == Some(true),
      FormState::Optional => required_state(node) == Some(false),
    }
  }

  /// HTML's "actually disabled".
  /// <https://html.spec.whatwg.org/multipage/semantics-other.html#concept-element-disabled>
  fn is_actually_disabled(&self) -> bool {
    let node = self.node();

    if is_disableable_form_control(node) || is_element(node, "fieldset") {
      return has_attribute(node, "disabled") || self.is_in_disabled_fieldset();
    }

    if is_element(node, "optgroup") {
      return has_attribute(node, "disabled");
    }

    is_element(node, "option")
      && (has_attribute(node, "disabled")
        || self.parent_element().is_some_and(|parent| {
          is_element(parent.node(), "optgroup") && has_attribute(parent.node(), "disabled")
        }))
  }

  /// Whether a `<fieldset disabled>` holds it outside that fieldset's first `<legend>` child.
  fn is_in_disabled_fieldset(&self) -> bool {
    let first_legends = &self.form_controls().first_legends;
    let mut child = *self;

    while let Some(parent) = child.parent_element() {
      if is_element(parent.node(), "fieldset")
        && has_attribute(parent.node(), "disabled")
        && first_legends.get(&parent.index) != Some(&child.index)
      {
        return true;
      }

      child = parent;
    }

    false
  }

  /// A checkbox or radio button's checkedness, or an option's selectedness, as parsing the markup
  /// leaves them.
  fn is_checked(&self) -> bool {
    let node = self.node();

    match InputType::of(node) {
      Some(InputType::Checkbox) => has_attribute(node, "checked"),
      Some(InputType::Radio) => self.form_controls().checked_radio(self.index) == Some(self.index),
      _ => is_element(node, "option") && self.is_selected_option(),
    }
  }

  fn form_controls(&self) -> &'a FormControls {
    self
      .tree
      .form_controls
      .get_or_init(|| FormControls::new(self.tree))
  }

  /// HTML's selectedness of an `<option>`, from `selected` attributes.
  fn is_selected_option(&self) -> bool {
    match self.owner_select() {
      Some(select) if !has_attribute(select.node(), "multiple") => {
        self.form_controls().selected_options.get(&select.index) == Some(&Some(self.index))
      }
      _ => has_attribute(self.node(), "selected"),
    }
  }

  /// The option a single-selection `<select>` shows selected: the last one with `selected`, or
  /// its first enabled option when none has one and it shows a single row.
  /// <https://html.spec.whatwg.org/multipage/form-elements.html#selectedness-setting-algorithm>
  fn selected_option(&self) -> Option<usize> {
    let options: SmallVec<[ArenaElement<'a>; 8]> = self.options().collect();

    if let Some(last_selected) = options
      .iter()
      .rev()
      .find(|option| has_attribute(option.node(), "selected"))
    {
      return Some(last_selected.index);
    }

    let display_size = self
      .node()
      .attribute("size")
      .and_then(|size| size.trim().parse::<u32>().ok())
      .filter(|&size| size > 0)
      .unwrap_or(1);

    options
      .iter()
      .find(|option| display_size == 1 && !option.is_actually_disabled())
      .map(|option| option.index)
  }

  /// The `<select>` whose list of options holds this `<option>`.
  fn owner_select(&self) -> Option<Self> {
    let parent = self.parent_element()?;

    if is_element(parent.node(), "select") {
      return Some(parent);
    }

    let grandparent = parent.parent_element()?;

    (is_element(parent.node(), "optgroup") && is_element(grandparent.node(), "select"))
      .then_some(grandparent)
  }

  /// A `<select>`'s list of options: its `<option>` children and those of its `<optgroup>`
  /// children, in tree order.
  fn options(&self) -> impl Iterator<Item = ArenaElement<'a>> {
    self.element_children().flat_map(|child| {
      let options: SmallVec<[ArenaElement<'a>; 8]> = if is_element(child.node(), "optgroup") {
        child
          .element_children()
          .filter(|option| is_element(option.node(), "option"))
          .collect()
      } else {
        SmallVec::from_iter(is_element(child.node(), "option").then_some(child))
      };

      options
    })
  }

  fn element_children(&self) -> impl Iterator<Item = ArenaElement<'a>> {
    successors(self.first_element_child(), |child| {
      child.next_sibling_element()
    })
  }

  /// HTML's `:read-write`: a mutable text control, or an editable element other than one.
  fn is_read_write(&self) -> bool {
    let node = self.node();

    if InputType::of(node).is_some() || is_element(node, "textarea") {
      return is_mutable_text_control(node, self.is_actually_disabled());
    }

    let mut current = Some(*self);

    while let Some(element) = current {
      match element.node().attribute("contenteditable") {
        Some(value)
          if value.is_empty()
            || value.eq_ignore_ascii_case("true")
            || value.eq_ignore_ascii_case("plaintext-only") =>
        {
          return true;
        }
        Some(value) if value.eq_ignore_ascii_case("false") => return false,
        _ => current = element.parent_element(),
      }
    }

    false
  }

  /// HTML's [directionality](https://html.spec.whatwg.org/multipage/dom.html#the-directionality)
  /// from `dir` attributes. Approximate: `dir="auto"` does not read the text.
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
      .classes
      .iter()
      .any(|class| *class == &**name)
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
      PseudoClass::FormState(state) => return self.matches_form_state(*state),
      PseudoClass::Other(_) => return false,
    };

    // The nearest `lang`, as HTML determines the language. With none, `:lang()` never matches.
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

/// What a selector's subject compound requires, so other nodes skip the rule. Mirrors the
/// bucket order of Blink's `RuleSet::FindBestRuleSetAndAdd`: id, class, attribute, then tag.
enum SelectorKey {
  Id(String),
  Class(String),
  /// Lowercased: attribute names match case-insensitively.
  Attribute(String),
  /// Lowercased: tag names match case-insensitively.
  Tag(String),
  Any,
}

impl SelectorKey {
  fn rank(&self) -> u8 {
    match self {
      Self::Any => 0,
      Self::Tag(_) => 1,
      Self::Attribute(_) => 2,
      Self::Class(_) => 3,
      Self::Id(_) => 4,
    }
  }
}

fn selector_key(selector: &Selector<SelectorImpl>) -> SelectorKey {
  let mut key = SelectorKey::Any;
  let mut keep = |candidate: SelectorKey| {
    if candidate.rank() > key.rank() {
      key = candidate;
    }
  };
  // `iter` stops at each combinator. A pseudo-element sits behind one, and the element's own
  // compound comes after it.
  let mut iter = selector.iter();

  loop {
    for component in &mut iter {
      match component {
        Component::ID(id) => keep(SelectorKey::Id(id.to_string())),
        Component::Class(class) => keep(SelectorKey::Class(class.to_string())),
        Component::AttributeInNoNamespaceExists {
          local_name_lower, ..
        } => keep(attribute_key(local_name_lower)),
        Component::AttributeInNoNamespace { local_name, .. } => keep(attribute_key(local_name)),
        Component::LocalName(name) => {
          keep(SelectorKey::Tag(name.lower_name.to_ascii_lowercase()));
        }
        _ => {}
      }
    }

    if iter.next_sequence() != Some(Combinator::PseudoElement) {
      return key;
    }
  }
}

/// `id`, `class`, and `lang` live outside a node's attribute map, so they key no bucket.
fn attribute_key(name: &str) -> SelectorKey {
  match_ignore_ascii_case! { name,
    "id" | "class" | "lang" => SelectorKey::Any,
    _ => SelectorKey::Attribute(name.to_ascii_lowercase()),
  }
}

/// A stylesheet's rules, by position, grouped by what their rightmost compound
/// requires, so a node visits only the rules that could match it.
#[derive(Debug, Default)]
pub(crate) struct RuleIndex {
  by_id: HashMap<String, Vec<usize>>,
  by_class: HashMap<String, Vec<usize>>,
  by_attribute: HashMap<String, Vec<usize>>,
  by_tag: HashMap<String, Vec<usize>>,
  unindexed: Vec<usize>,
  /// The bloom filter hashes of each rule's selectors, as Blink's `RuleData` keeps them.
  ancestor_hashes: Vec<Box<[AncestorHashes]>>,
  /// Each distinct `media_queries` list among the rules.
  media: Vec<Vec<MediaQueryList>>,
  /// The position in `media` of each rule's `media_queries`.
  rule_media: Vec<usize>,
}

impl RuleIndex {
  pub(crate) fn build(rules: &[CssRule]) -> Self {
    let mut index = Self {
      rule_media: Vec::with_capacity(rules.len()),
      ancestor_hashes: Vec::with_capacity(rules.len()),
      ..Self::default()
    };

    for (position, rule) in rules.iter().enumerate() {
      let media = index.media_id(&rule.media_queries);
      index.rule_media.push(media);
      index.ancestor_hashes.push(
        rule
          .selectors
          .slice()
          .iter()
          .map(|selector| AncestorHashes::new(selector, QuirksMode::NoQuirks))
          .collect(),
      );

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
          SelectorKey::Attribute(name) => index.by_attribute.entry(name),
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

    for class in node.classes() {
      if let Some(rules) = self.by_class.get(class) {
        out.extend_from_slice(rules);
      }
    }

    if !self.by_attribute.is_empty() {
      for name in node.attribute_names() {
        let rules = if name.bytes().any(|byte| byte.is_ascii_uppercase()) {
          self.by_attribute.get(&name.to_ascii_lowercase())
        } else {
          self.by_attribute.get(name)
        };

        if let Some(rules) = rules {
          out.extend_from_slice(rules);
        }
      }
    }

    if let Some(tag) = node.tag_name()
      && let Some(rules) = if tag.bytes().any(|byte| byte.is_ascii_uppercase()) {
        self.by_tag.get(&tag.to_ascii_lowercase())
      } else {
        self.by_tag.get(tag)
      }
    {
      out.extend_from_slice(rules);
    }

    out.retain(|&rule| matching_media[self.rule_media[rule]]);
    out.sort_unstable();
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

/// The declaration blocks of `stylesheets` that apply to each node under `root`, in tree order.
/// Approximate: same-named layers of different sheets stay apart.
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

  let mut candidates = Vec::new();

  let mut element_caches = SelectorCaches::default();
  let mut pseudo_caches = SelectorCaches::default();

  // In preorder, one counting filter along the ancestor chain stands in for a filter per node.
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
      let rule_index = placement.sheet.rule_index();

      rule_index.candidates(
        arena.nodes[i].node,
        &placement.matching_media,
        &mut candidates,
      );

      for &position in &candidates {
        let rule = &placement.sheet.rules()[position];
        let source_order = placement.rule_offset + position;
        let ancestor_hashes = &rule_index.ancestor_hashes[position];
        let mut best_specificities: [Option<u32>; 3] = [None; 3];

        for (selector_index, selector) in rule.selectors.slice().iter().enumerate() {
          let Some(target) = selector_target(selector) else {
            continue;
          };
          if is_replaced && target != SelectorTarget::Element {
            continue;
          }

          if early_reject_by_local_name(selector, 0, &element) {
            continue;
          }

          let ctx = if target == SelectorTarget::Element {
            &mut element_ctx
          } else {
            &mut pseudo_ctx
          };

          if matches_selector(
            selector,
            0,
            Some(&ancestor_hashes[selector_index]),
            &element,
            ctx,
          ) {
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

/// Records `rule`'s blocks in `bucket` at `best_specificity` when one of its selectors matched.
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

/// RFC 4647 basic filtering, as `:lang()` uses.
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

#[cfg(test)]
mod tests {
  use std::{collections::BTreeMap, sync::Arc};

  use crate::{
    context::RenderContext,
    layout::{node::Node, tree::RenderNode},
    resources::font::Fonts,
    style::{SizingContext, StyleSheet},
    viewport::Viewport,
  };

  #[test]
  fn a_class_splits_on_ascii_whitespace_only() {
    let stylesheet = StyleSheet::parse(".a > .c { display: none }").expect("stylesheet parses");
    let context = RenderContext::builder()
      .fonts(Fonts::default().snapshot())
      .sizing(
        SizingContext::builder()
          .viewport(Viewport::default())
          .build(),
      )
      .stylesheet(Arc::new(stylesheet))
      .build();
    let parent = |class_name: &str| {
      Node::container([Node::container([]).with_class_name("c")]).with_class_name(class_name)
    };
    let root = RenderNode::from_node(
      &context,
      Node::container([parent("x\ta\n"), parent("a\u{a0}x")]),
    );
    let kept: Vec<_> = root
      .children
      .as_deref()
      .expect("children")
      .iter()
      .map(|parent| parent.children.as_deref().map_or(0, <[_]>::len))
      .collect();

    // U+00A0 is not ASCII whitespace, so `a\u{a0}x` is one class and its child stays.
    assert_eq!(kept, [0, 1]);
  }

  fn candidates(css: &str, node: &Node) -> Vec<usize> {
    let stylesheet = StyleSheet::parse(css).expect("stylesheet parses");
    let index = stylesheet.rule_index();
    let mut out = Vec::new();

    index.candidates(node, &index.matching_media(Viewport::default()), &mut out);
    out
  }

  #[test]
  fn a_pseudo_element_rule_waits_for_its_host_class() {
    let css = ".a::after { content: 'x' }";

    assert_eq!(candidates(css, &Node::container([])), [0; 0]);
    assert_eq!(
      candidates(css, &Node::container([]).with_class_name("a")),
      [0]
    );
  }

  #[test]
  fn an_attribute_rule_reaches_nodes_with_the_attribute_in_any_case() {
    let css = "[DATA-X] { color: red } [data-y='1'] { color: red } [id] { color: red }";
    let node = Node::container([]).with_attributes(BTreeMap::from([("data-X".into(), "".into())]));

    assert_eq!(candidates(css, &node), [0, 2]);
    assert_eq!(candidates(css, &Node::container([])), [2]);
  }
}
