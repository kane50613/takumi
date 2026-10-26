//! The custom properties in scope on an element: their specified values and the
//! `@property` rules that govern them.

use std::{
  collections::{HashMap, HashSet},
  sync::Arc,
};

use crate::style::{CssWideKeyword, FromCssStr, selector::PropertyRule};

/// The `@property` rules in force, by name, shared by every element they reach.
pub(crate) type Registrations = Arc<HashMap<String, PropertyRule>>;

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

  pub(crate) fn values(&self) -> &HashMap<String, String> {
    &self.values
  }

  pub(crate) fn replace_values(&mut self, values: HashMap<String, String>) {
    self.values = Arc::new(values);
  }

  /// Sets `name` as a declaration specifies it. A CSS-wide keyword resolves here, as the cascade
  /// resolves it for a custom property: `inherit` takes `parent`'s value, `initial` the registered
  /// initial value or none, and `unset` either one as the property inherits.
  /// <https://drafts.csswg.org/css-variables-1/#defining-variables>
  pub(crate) fn declare(&mut self, name: String, value: String, parent: Option<&Self>) {
    let Ok(keyword) = CssWideKeyword::from_css_str(&value) else {
      return self.set(name, value);
    };
    let registration = self.registration(&name);
    let inherits = registration.is_none_or(|rule| rule.inherits);
    let inherited = match keyword {
      CssWideKeyword::Inherit => true,
      CssWideKeyword::Unset => inherits,
      CssWideKeyword::Initial => false,
    };
    let resolved = match parent {
      Some(parent) if inherited => parent.get(&name).map(str::to_owned),
      _ => registration.and_then(|rule| rule.initial_value.clone()),
    };

    match resolved {
      Some(value) => self.set(name, value),
      None => self.remove(&name),
    }
  }

  pub(crate) fn remove(&mut self, name: &str) {
    Arc::make_mut(&mut self.values).remove(name);
  }

  /// The `@property` rule governing `name`, if one registered it.
  pub(crate) fn registration(&self, name: &str) -> Option<&PropertyRule> {
    self.registrations.get(name)
  }

  /// Whether any `@property` registration is in scope here.
  pub(crate) fn has_registrations(&self) -> bool {
    !self.registrations.is_empty()
  }

  /// Whether `registrations` are the ones in scope here.
  pub(crate) fn registers(&self, registrations: &Registrations) -> bool {
    Arc::ptr_eq(&self.registrations, registrations) || self.registrations == *registrations
  }

  /// Puts `registrations` in scope, leaving the value each rule puts there: the parent's when it
  /// inherits, the rule's initial value otherwise.
  pub(crate) fn register_in_scope(&mut self, registrations: &Registrations, parent: &Self) {
    self.registrations = Arc::clone(registrations);

    for rule in registrations.values() {
      let inherited = rule.inherits.then(|| parent.get(&rule.name)).flatten();

      match inherited.or(rule.initial_value.as_deref()) {
        Some(value) => self.set(rule.name.clone(), value.to_owned()),
        None => self.remove(&rule.name),
      }
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

  /// What a child starts `name` from: the value itself when it inherits, the
  /// registered initial value when it does not, and nothing when an engine
  /// wrote it as this element's own state.
  fn inherited_value<'v>(&'v self, name: &str, value: &'v str) -> Option<&'v str> {
    if self.element_state.contains(name) {
      return None;
    }

    match self.registration(name) {
      Some(rule) if !rule.inherits => rule.initial_value.as_deref(),
      _ => Some(value),
    }
  }

  /// Whether a child needs a map of its own. Conservative: a non-inheriting
  /// property already holding its initial value still answers yes.
  fn needs_its_own_values(&self) -> bool {
    !self.element_state.is_empty()
      || self
        .registrations
        .values()
        .any(|rule| !rule.inherits && self.values.contains_key(&rule.name))
  }

  /// The properties a child starts from: the registrations as they are, and the
  /// values each rule leaves in reach.
  pub(crate) fn inherited(&self) -> Self {
    let values = if self.needs_its_own_values() {
      Arc::new(
        self
          .values
          .iter()
          .filter_map(|(name, value)| {
            Some((name.clone(), self.inherited_value(name, value)?.to_owned()))
          })
          .collect(),
      )
    } else {
      self.values.clone()
    };

    Self {
      values,
      registrations: self.registrations.clone(),
      element_state: Default::default(),
    }
  }
}
