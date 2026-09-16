//! The custom properties in scope on an element: their specified values and the
//! `@property` rules that govern them.

use std::{collections::HashMap, sync::Arc};

use crate::style::selector::PropertyRule;

/// The custom properties an element resolves `var()` against, with the
/// `@property` registrations that decide how each one inherits.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CustomProperties {
  values: Arc<HashMap<String, String>>,
  registrations: Arc<HashMap<String, PropertyRule>>,
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

  /// Every specified value, for the `var()` resolver.
  pub(crate) fn values(&self) -> &HashMap<String, String> {
    &self.values
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
  pub(crate) fn apply_registration(&mut self, rule: &PropertyRule, parent: &Self) {
    self.register(rule.clone());

    let inherited = rule.inherits.then(|| parent.get(&rule.name)).flatten();

    match inherited.or(rule.initial_value.as_deref()) {
      Some(value) => self.set(rule.name.clone(), value.to_owned()),
      None => self.remove(&rule.name),
    }
  }

  /// Whether a child inherits `name` from this element.
  ///
  /// [`Self::apply_registration`] has already left the value a registered
  /// property puts in scope, so a registered name passes through untouched.
  /// That leaves the unregistered ones: `--tw-*` holds per-element composition
  /// state the utility engine writes without registering, and stops here.
  fn inherits(&self, name: &str) -> bool {
    self.registration(name).is_some() || !name.starts_with("--tw-")
  }

  /// The properties a child starts from, carrying the registrations forward and
  /// dropping the values that stop here.
  pub(crate) fn inherited(&self) -> Self {
    let registrations = self.registrations.clone();

    if self.values.keys().all(|name| self.inherits(name)) {
      return Self {
        values: self.values.clone(),
        registrations,
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
    }
  }
}
