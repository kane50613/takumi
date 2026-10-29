//! Blink's [`CountersAttachmentContext`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/css/counters_attachment_context.cc):
//! the CSS counters in scope as the render tree is built in document order, entering each element
//! and pseudo-element that gets a box and leaving it after its descendants.
//!
//! It follows Blink as Chrome ships it, with the experimental `CSSListCounterAccounting` off, so
//! `reversed()` resets a counter to its integer. The page and page-margin paths are left out, as
//! this tree holds neither box.

use std::sync::Arc;

use rustc_hash::FxHashMap;

use crate::{
  layout::{
    list_marker::{ListItemOrdinals, explicit_value},
    node::Node,
  },
  style::{ComputedStyle, Contain, CounterChange, Display},
};

pub(super) const LIST_ITEM: &str = "list-item";

/// Blink's `CountersAttachmentContext::Type`, as a bit mask.
const INCREMENT_TYPE: u8 = 1 << 0;
const RESET_TYPE: u8 = 1 << 1;
const SET_TYPE: u8 = 1 << 2;

/// What the implicit `list-item` counter does on entering an element, Blink's
/// `ElementGeneratesListItemCounter` and the `ListItemOrdinal` it reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ListItemCounter {
  /// The element generates no `list-item` counter.
  None,
  /// An element that is a list item, with an `li`'s `value` attribute, in a list that is
  /// `reversed` or not.
  Item {
    explicit_value: Option<i32>,
    reversed: bool,
  },
  /// An `ol`, with its `InitialCounter`.
  OrderedList {
    initial_counter: i32,
    reversed: bool,
  },
  /// A `ul`, `menu`, or `dir` that is not a list item.
  OtherList,
}

impl ListItemCounter {
  /// The implicit `list-item` counter of `node`, a box with `display`, whose items `ordinals`
  /// number, in the list `enclosing` numbers: Blink's `ElementGeneratesListItemCounter`, then
  /// `ListItemOrdinal::Get`, which a list item alone has.
  pub(super) fn of(
    node: &Node,
    display: Display,
    ordinals: &ListItemOrdinals,
    enclosing: &ListItemOrdinals,
  ) -> Self {
    let Some(tag) = node.tag_name() else {
      return Self::None;
    };
    let is = |name: &str| tag.eq_ignore_ascii_case(name);

    if !["ol", "ul", "li", "menu", "dir"].into_iter().any(is) {
      return Self::None;
    }

    if display == Display::ListItem {
      return Self::Item {
        explicit_value: explicit_value(node),
        reversed: enclosing.reversed(),
      };
    }

    if is("ol")
      && let Some(initial_counter) = ordinals.initial_counter()
    {
      return Self::OrderedList {
        initial_counter,
        reversed: ordinals.reversed(),
      };
    }

    if is("li") {
      Self::None
    } else {
      Self::OtherList
    }
  }
}

/// Blink's `CounterDirectives`: what one element does to one counter.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct CounterDirectives {
  reset_value: Option<i64>,
  is_reset_reversed: bool,
  increment_value: Option<i32>,
  set_value: Option<i32>,
}

impl CounterDirectives {
  /// The directives `style` gives the counter `counter_name`.
  pub(super) fn of(style: &ComputedStyle, counter_name: &str) -> Self {
    counter_directives(style)
      .into_iter()
      .find_map(|(name, directives)| (&*name == counter_name).then_some(directives))
      .unwrap_or_default()
  }

  pub(super) fn has_set(self) -> bool {
    self.set_value.is_some()
  }

  pub(super) fn has_increment(self) -> bool {
    self.increment_value.is_some()
  }

  fn is_reset(self) -> bool {
    self.reset_value.is_some() || self.is_reset_reversed
  }

  fn is_defined(self) -> bool {
    self.is_reset() || self.increment_value.is_some() || self.set_value.is_some()
  }

  pub(super) fn combined_value(self) -> i32 {
    if let Some(set_value) = self.set_value {
      return set_value;
    }

    saturate(self.reset_value.unwrap_or(0) + i64::from(self.increment_value.unwrap_or(0)))
  }

  /// Blink's `DetermineCounterTypeAndValue` for an element or a `::before` or `::after`, whose
  /// pseudo type counters allow.
  fn type_and_value(self) -> Option<(u8, i32)> {
    if !self.is_defined() {
      return None;
    }

    let mut type_mask = 0;

    if self.increment_value.is_some() {
      type_mask |= INCREMENT_TYPE;
    }
    if self.is_reset() {
      type_mask |= RESET_TYPE;
    }
    if self.set_value.is_some() {
      type_mask |= SET_TYPE;
    }

    Some((type_mask, self.combined_value()))
  }
}

/// Blink's `CounterDirectiveMap`, as `StyleBuilderConverter` fills it from `counter-reset`,
/// `counter-increment`, and `counter-set`: increments of one name add up, a later reset or set
/// replaces an earlier one.
fn counter_directives(style: &ComputedStyle) -> Vec<(Arc<str>, CounterDirectives)> {
  let misc1 = &style.misc1_data;
  let mut directives: Vec<(Arc<str>, CounterDirectives)> = Vec::new();

  for change in misc1.counter_reset.iter() {
    let directive = directive_for(&mut directives, change);

    directive.reset_value = Some(i64::from(change.value));
    directive.is_reset_reversed = change.reversed;
  }
  for change in misc1.counter_increment.iter() {
    let directive = directive_for(&mut directives, change);

    directive.increment_value = Some(
      directive
        .increment_value
        .unwrap_or(0)
        .saturating_add(change.value),
    );
  }
  for change in misc1.counter_set.iter() {
    directive_for(&mut directives, change).set_value = Some(change.value);
  }

  directives
}

/// The directives for the counter `change` names, added when `directives` has none yet.
fn directive_for<'a>(
  directives: &'a mut Vec<(Arc<str>, CounterDirectives)>,
  change: &CounterChange,
) -> &'a mut CounterDirectives {
  let index = match directives.iter().position(|(name, _)| *name == change.name) {
    Some(index) => index,
    None => {
      directives.push((Arc::clone(&change.name), CounterDirectives::default()));
      directives.len() - 1
    }
  };

  &mut directives[index].1
}

fn is_reset(counter_type: u8) -> bool {
  counter_type & RESET_TYPE != 0
}

fn is_set_or_reset(counter_type: u8) -> bool {
  counter_type & (RESET_TYPE | SET_TYPE) != 0
}

fn calculate_counter_value(
  counter_type: u8,
  counter_value: i32,
  counter_current_value: i32,
) -> i32 {
  if is_set_or_reset(counter_type) {
    return counter_value;
  }

  counter_current_value
    .checked_add(counter_value)
    .unwrap_or(counter_current_value)
}

pub(super) fn saturate(value: i64) -> i32 {
  i32::try_from(value).unwrap_or(if value < 0 { i32::MIN } else { i32::MAX })
}

/// A box the counters see, numbered in the order the builder enters it.
type ObjectId = usize;

/// Blink's `CounterEntry`: a counter, the box that created it, and that box's parent.
#[derive(Clone, Copy, Debug)]
struct CounterEntry {
  layout_object: ObjectId,
  parent: Option<ObjectId>,
  value: i32,
}

/// Each name's counters from the outermost in, `None` marking a style containment boundary.
type CounterStack = Vec<Option<CounterEntry>>;

/// The counters in scope while a render tree is built, and the boxes entered but not yet left.
#[derive(Default)]
pub(super) struct CountersAttachmentContext {
  counter_inheritance_table: FxHashMap<Arc<str>, CounterStack>,
  path: Vec<ObjectId>,
  next_object: ObjectId,
}

impl CountersAttachmentContext {
  /// Enters the box of an element or a `::before` or `::after` whose style is `style`, after its
  /// parent and before its children.
  pub(super) fn enter_object(&mut self, style: &ComputedStyle, list_item: ListItemCounter) {
    self.path.push(self.next_object);
    self.next_object += 1;

    let counter_directives = counter_directives(style);

    for (counter_name, directives) in &counter_directives {
      let Some((counter_type, value_argument)) = directives.type_and_value() else {
        continue;
      };

      self.process_counter(counter_name, counter_type, value_argument);
    }

    // If there were no explicit counter related property set for `list-item` counter, maybe we
    // need to create implicit one.
    if list_item != ListItemCounter::None && !has_counter(&counter_directives, LIST_ITEM) {
      self.maybe_create_list_item_counter(list_item);
    }

    // The element itself is not in its style containment scope.
    if contains_style(style) {
      self.enter_style_containment_scope();
    }
  }

  /// Leaves the box [`Self::enter_object`] entered last, after its descendants.
  pub(super) fn leave_object(&mut self, style: &ComputedStyle, list_item: ListItemCounter) {
    if contains_style(style) {
      self.leave_style_containment_scope();
    }

    let counter_directives = counter_directives(style);

    for (counter_name, directives) in &counter_directives {
      let Some((counter_type, _)) = directives.type_and_value() else {
        continue;
      };

      if !is_reset(counter_type) {
        continue;
      }

      // Remove self from stack if previous counter on stack is ancestor to self, as every later
      // element would inherit the ancestor's instead.
      self.remove_counter_if_ancestor_exists(counter_name);
    }

    if list_item != ListItemCounter::None && !has_counter(&counter_directives, LIST_ITEM) {
      self.remove_counter_if_ancestor_exists(LIST_ITEM);
    }

    self.path.pop();
  }

  /// The values `counter()`, when `only_last` is set, or `counters()` reads for `counter_name` in
  /// the box entered last, the innermost first.
  pub(super) fn get_counter_values(&mut self, counter_name: &str, only_last: bool) -> Vec<i32> {
    self.remove_stale_counters(counter_name);

    let Some(counter_stack) = self.counter_inheritance_table.get(counter_name) else {
      return vec![0];
    };
    // counter() and counters() can cross style containment boundaries.
    let mut result: Vec<i32> = counter_stack
      .iter()
      .rev()
      .flatten()
      .map(|entry| entry.value)
      .take(if only_last { 1 } else { usize::MAX })
      .collect();

    if result.is_empty() {
      result.push(0);
    }

    result
  }

  fn maybe_create_list_item_counter(&mut self, list_item: ListItemCounter) {
    self.remove_stale_counters(LIST_ITEM);

    match list_item {
      ListItemCounter::None => {}
      ListItemCounter::Item {
        explicit_value: Some(value),
        ..
      } => self.create_counter(LIST_ITEM, value),
      ListItemCounter::Item { reversed, .. } => {
        self.update_counter_value(LIST_ITEM, INCREMENT_TYPE, if reversed { -1 } else { 1 });
      }
      ListItemCounter::OrderedList {
        initial_counter,
        reversed,
      } => {
        self.create_counter(
          LIST_ITEM,
          initial_counter.saturating_add(if reversed { 1 } else { -1 }),
        );
      }
      ListItemCounter::OtherList => self.create_counter(LIST_ITEM, 0),
    }
  }

  fn process_counter(&mut self, counter_name: &str, counter_type: u8, value_argument: i32) {
    // First, there might be some counters on stack that are stale, remove those.
    self.remove_stale_counters(counter_name);

    // Reset counter always creates counter.
    if is_reset(counter_type) {
      self.create_counter(counter_name, value_argument);
      return;
    }

    // Otherwise, get the value of last counter from stack and update its value. This can create
    // a counter, if there are no counters on stack.
    self.update_counter_value(counter_name, counter_type, value_argument);
  }

  /// Pushes a counter, after removing the innermost one when its box is a sibling of the box
  /// entered last, per <https://drafts.csswg.org/css-lists/#instantiating-counters>.
  fn create_counter(&mut self, counter_name: &str, value: i32) {
    let new_entry = CounterEntry {
      layout_object: self.current(),
      parent: self.current_parent(),
      value,
    };
    let Some(counter_stack) = self.counter_inheritance_table.get_mut(counter_name) else {
      self
        .counter_inheritance_table
        .insert(counter_name.into(), vec![Some(new_entry)]);
      return;
    };

    if let Some(Some(last)) = counter_stack.last()
      && last.parent == new_entry.parent
    {
      counter_stack.pop();
    }

    counter_stack.push(Some(new_entry));
  }

  /// Removes the counters whose box's parent is not an ancestor of the box entered last: the
  /// builder has left their scope.
  fn remove_stale_counters(&mut self, counter_name: &str) {
    let Some(counter_stack) = self.counter_inheritance_table.get(counter_name) else {
      return;
    };
    let stale = counter_stack
      .iter()
      .rev()
      .take_while(|entry| {
        // A style containment boundary stops the removal.
        entry.is_some_and(|entry| entry.parent.is_some_and(|parent| !self.is_ancestor(parent)))
      })
      .count();

    if let Some(counter_stack) = self.counter_inheritance_table.get_mut(counter_name) {
      counter_stack.truncate(counter_stack.len() - stale);
    }
  }

  /// When leaving the box that created the last counter, removes that counter if the one before
  /// it belongs to an ancestor, or to an earlier sibling of an ancestor: it would never be
  /// inherited, as an ancestor's counter always comes first.
  fn remove_counter_if_ancestor_exists(&mut self, counter_name: &str) {
    let current = self.current();
    let current_parent = self.current_parent();
    let Some(counter_stack) = self.counter_inheritance_table.get(counter_name) else {
      return;
    };
    // Don't remove the last on stack counter or style containment boundary.
    let [.., Some(previous_entry), Some(last_entry)] = counter_stack.as_slice() else {
      return;
    };

    if last_entry.layout_object != current {
      return;
    }

    let removes = self.is_ancestor(previous_entry.layout_object)
      || previous_entry
        .parent
        .is_some_and(|parent| self.is_ancestor(parent) && Some(parent) != current_parent);

    if removes && let Some(counter_stack) = self.counter_inheritance_table.get_mut(counter_name) {
      counter_stack.pop();
    }
  }

  /// Updates the value of the last counter on the stack, or creates one when there is none.
  fn update_counter_value(&mut self, counter_name: &str, counter_type: u8, counter_value: i32) {
    let default_counter_value = calculate_counter_value(counter_type, counter_value, 0);

    match self
      .counter_inheritance_table
      .get_mut(counter_name)
      .and_then(|counter_stack| counter_stack.last_mut())
    {
      // Otherwise take the value of last counter on stack and update it.
      Some(Some(current)) => {
        current.value = calculate_counter_value(counter_type, counter_value, current.value);
      }
      // No counter of the name, or a style containment boundary last on the stack.
      _ => self.create_counter(counter_name, default_counter_value),
    }
  }

  /// Pushes a style containment boundary onto every stack. A counter of a name first seen later
  /// is still removed on leaving, as its stack empties first.
  fn enter_style_containment_scope(&mut self) {
    for counter_stack in self.counter_inheritance_table.values_mut() {
      counter_stack.push(None);
    }
  }

  /// Pops each stack to its style containment boundary, then the boundary.
  fn leave_style_containment_scope(&mut self) {
    for counter_stack in self.counter_inheritance_table.values_mut() {
      while counter_stack.last().is_some_and(Option::is_some) {
        counter_stack.pop();
      }

      counter_stack.pop();
    }
  }

  fn current(&self) -> ObjectId {
    self.path.last().copied().unwrap_or_default()
  }

  fn current_parent(&self) -> Option<ObjectId> {
    self
      .path
      .len()
      .checked_sub(2)
      .and_then(|index| self.path.get(index).copied())
  }

  /// Whether `object` is a strict ancestor of the box entered last.
  fn is_ancestor(&self, object: ObjectId) -> bool {
    self
      .path
      .split_last()
      .is_some_and(|(_, ancestors)| ancestors.contains(&object))
  }
}

fn has_counter(counter_directives: &[(Arc<str>, CounterDirectives)], counter_name: &str) -> bool {
  counter_directives
    .iter()
    .any(|(name, _)| &**name == counter_name)
}

pub(super) fn contains_style(style: &ComputedStyle) -> bool {
  style.misc_data.contain.contains(Contain::STYLE)
}
