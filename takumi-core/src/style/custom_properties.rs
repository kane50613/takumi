//! The custom properties in scope on an element: their specified values and the
//! `@property` rules that govern them.

use std::{
  collections::{HashMap, HashSet},
  sync::Arc,
};

use crate::style::selector::PropertyRule;

/// The custom properties an element resolves `var()` against, with the
/// `@property` registrations that decide how each one inherits.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CustomProperties {
  values: Arc<HashMap<String, String>>,
  registrations: Arc<HashMap<String, PropertyRule>>,
  /// Names a utility engine wrote as this element's own composition state.
  /// Unlike a registration, this does not reach the children.
  element_state: Arc<HashSet<String>>,
}

impl CustomProperties {
  /// The specified value of `name`, before `var()` substitution.
  pub fn get(&self, name: &str) -> Option<&str> {
    self.values.get(name).map(String::as_str)
  }

  /// Whether `name` has a specified value.
  pub fn contains(&self, name: &str) -> bool {
    self.values.contains_key(name)
  }

  pub(crate) fn set(&mut self, name: String, value: String) {
    Arc::make_mut(&mut self.values).insert(name, value);
  }

  pub(crate) fn remove(&mut self, name: &str) {
    Arc::make_mut(&mut self.values).remove(name);
  }

  /// The `@property` rule governing `name`, if one registered it.
  pub(crate) fn registration(&self, name: &str) -> Option<&PropertyRule> {
    self.registrations.get(name)
  }

  pub(crate) fn register(&mut self, rule: PropertyRule) {
    Arc::make_mut(&mut self.registrations).insert(rule.name.clone(), rule);
  }

  /// Applies one `@property` registration, leaving the value the rule puts in
  /// scope: the parent's when it inherits, the rule's initial value otherwise.
  pub(crate) fn register_in_scope(&mut self, rule: &PropertyRule, parent: &Self) {
    self.register(rule.clone());

    let inherited = rule.inherits.then(|| parent.get(&rule.name)).flatten();

    match inherited.or(rule.initial_value.as_deref()) {
      Some(value) => self.set(rule.name.clone(), value.to_owned()),
      None => self.remove(&rule.name),
    }
  }

  /// Records state a utility engine wrote for this element alone. Tailwind's
  /// own stylesheet says so with an `@property` rule; the engine has no
  /// stylesheet, so it says so here. An author's rule for the name wins.
  pub(crate) fn register_element_state(&mut self, name: &str) {
    if self.registration(name).is_some() {
      return;
    }

    Arc::make_mut(&mut self.element_state).insert(name.to_owned());
  }

  /// Whether a child inherits `name`. A registered property keeps whatever
  /// [`Self::register_in_scope`] left it; only an engine's element state stops
  /// here.
  fn inherits(&self, name: &str) -> bool {
    !self.element_state.contains(name)
  }

  /// The properties a child starts from, carrying the registrations forward and
  /// dropping the values that stop here.
  pub(crate) fn inherited(&self) -> Self {
    let registrations = self.registrations.clone();

    if self.element_state.is_empty() {
      return Self {
        values: self.values.clone(),
        registrations,
        element_state: Default::default(),
      };
    }

    Self {
      values: Arc::new(
        self
          .values
          .iter()
          .filter(|(name, _)| self.inherits(name))
          .map(|(name, value)| (name.clone(), value.clone()))
          .collect(),
      ),
      registrations,
      element_state: Default::default(),
    }
  }
}
