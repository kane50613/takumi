//! The custom properties in scope on an element: their specified values and the
//! `@property` rules that govern them.

use std::{
  collections::{HashMap, HashSet},
  sync::Arc,
};

use super::hash_trie::HashTrie;
use crate::style::{
  CssWideKeyword, FromCssStr, PropertySyntax, SizingContext, contains_var_references,
  selector::PropertyRule,
};

/// The `@property` rules in force, by name, shared by every element they reach.
pub(crate) type Registrations = Arc<HashMap<String, PropertyRule>>;

/// The custom properties an element resolves `var()` against, with the `@property` registrations
/// that decide how each one inherits. As Blink's `StyleInheritedVariables` and
/// `StyleNonInheritedVariables`, the values a child inherits sit apart from the ones that stop here,
/// so a child shares the first and starts the second empty, each in a trie a change copies only a
/// path of. A registered property without a value
/// takes its initial value, as Blink's `StyleInitialData` gives it.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CustomProperties {
  inherited: HashTrie<String, String>,
  /// Registered properties that do not inherit, and the element state below.
  non_inherited: HashTrie<String, String>,
  registrations: Arc<HashMap<String, PropertyRule>>,
  /// Names a utility engine wrote as this element's own composition state.
  /// Unlike a registration, this does not reach the children.
  element_state: Arc<HashSet<String>>,
}

impl CustomProperties {
  /// The specified value of `name`, before `var()` substitution.
  pub fn get(&self, name: &str) -> Option<&str> {
    self
      .own(name)
      .or_else(|| self.registration(name)?.initial_value.as_deref())
  }

  /// The value this element holds for `name`, not a registered initial value.
  fn own(&self, name: &str) -> Option<&str> {
    self.values_of(name).get(name).map(String::as_str)
  }

  /// Whether `name` has a specified value.
  pub fn contains(&self, name: &str) -> bool {
    self.get(name).is_some()
  }

  pub(crate) fn set(&mut self, name: String, value: String) {
    self.values_of_mut(&name).insert(name, value);
  }

  /// Whether `name`'s value stops at this element instead of reaching its children.
  fn stops_here(&self, name: &str) -> bool {
    self.element_state.contains(name) || self.registration(name).is_some_and(|rule| !rule.inherits)
  }

  fn values_of(&self, name: &str) -> &HashTrie<String, String> {
    if self.stops_here(name) {
      &self.non_inherited
    } else {
      &self.inherited
    }
  }

  fn values_of_mut(&mut self, name: &str) -> &mut HashTrie<String, String> {
    if self.stops_here(name) {
      &mut self.non_inherited
    } else {
      &mut self.inherited
    }
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
      CssWideKeyword::Unset
      | CssWideKeyword::Revert
      | CssWideKeyword::RevertLayer
      | CssWideKeyword::RevertRule => inherits,
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
    if self.own(name).is_some() {
      self.values_of_mut(name).remove(name);
    }
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

  /// Puts `registrations` in scope. A registered property reads the value already here when it
  /// inherits, and its initial value otherwise.
  pub(crate) fn register_in_scope(&mut self, registrations: &Registrations) {
    self.registrations = Arc::clone(registrations);
  }

  /// A copy with each registered value computed against `sizing`, if any changes: a `<length>` to
  /// px, an `<angle>` to deg, a `<color>` to its color, and so on. A child takes the ones that do
  /// not inherit through `inherit` alone, as Blink's `ApplyInherit` copies the computed value.
  /// <https://drafts.css-houdini.org/css-properties-values-api-1/#calculation-of-computed-values>
  pub(crate) fn with_computed_values(&self, sizing: &SizingContext) -> Option<Self> {
    let mut computed: Option<Self> = None;

    for rule in self
      .registrations
      .values()
      .filter(|rule| rule.syntax != PropertySyntax::Universal)
    {
      let Some(value) = self.get(&rule.name) else {
        continue;
      };
      let Some(computed_value) = rule.syntax.compute(value, Some(sizing)) else {
        continue;
      };

      if computed_value != value {
        computed
          .get_or_insert_with(|| self.clone())
          .set(rule.name.clone(), computed_value);
      }
    }
    computed
  }

  /// Computes each of the `declared` names its registered syntax takes, as far as the element
  /// needs no sizing to, and makes each it does not take invalid at computed-value time: `unset`,
  /// so the parent's value when it inherits and the initial value otherwise. A value `var()` has
  /// yet to substitute into waits.
  /// <https://drafts.csswg.org/css-variables-1/#invalid-at-computed-value-time>
  pub(crate) fn enforce_syntax(&mut self, declared: &[String], parent: &Self) {
    let registrations = Arc::clone(&self.registrations);

    for name in declared {
      let Some(rule) = registrations.get(name) else {
        continue;
      };
      let value = self.own(name);

      if value.is_some_and(contains_var_references) {
        continue;
      }
      if let Some(computed) = value.and_then(|value| rule.syntax.compute(value, None)) {
        self.set(name.clone(), computed);
        continue;
      }

      match rule
        .inherits
        .then(|| parent.get(name))
        .flatten()
        .or(rule.initial_value.as_deref())
      {
        Some(value) => self.set(name.clone(), value.to_owned()),
        None => self.remove(name),
      }
    }
  }

  /// Records state a utility engine wrote for this element alone, as Tailwind's `@property` does.
  pub(crate) fn register_element_state(&mut self, name: &str) {
    if self.registration(name).is_some() || self.element_state.contains(name) {
      return;
    }

    let inherited = self.inherited.remove(name);

    Arc::make_mut(&mut self.element_state).insert(name.to_owned());
    if let Some(value) = inherited {
      self.set(name.to_owned(), value);
    }
  }

  /// The properties a child starts from: the values that inherit, shared, and none of the ones
  /// that stop here.
  pub(crate) fn inherited(&self) -> Self {
    Self {
      inherited: self.inherited.clone(),
      non_inherited: Default::default(),
      registrations: self.registrations.clone(),
      element_state: Default::default(),
    }
  }
}
