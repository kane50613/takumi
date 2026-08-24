use std::collections::HashMap;

/// A theme namespace, spelled as the CSS custom-property prefix it reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TwNamespace {
  /// `--color-*`
  Color,
  /// `--spacing-*`
  Spacing,
}

impl TwNamespace {
  const fn prefix(self) -> &'static str {
    match self {
      TwNamespace::Color => "--color-",
      TwNamespace::Spacing => "--spacing-",
    }
  }
}

/// Namespaces ordered longest-prefix-first, so `--font-weight-bold` never lands
/// in `--font-*`. Upstream spells the same rule as an explicit ignore list.
const NAMESPACES: &[TwNamespace] = &[TwNamespace::Spacing, TwNamespace::Color];

/// The outcome of a theme lookup, where a removed token must not fall back to
/// the built-in scale.
pub(crate) enum ThemeLookup<'a> {
  /// The token resolves to this CSS value.
  Value(&'a str),
  /// The token was removed with `initial`.
  Removed,
  /// The theme says nothing about this token.
  Missing,
}

/// One namespace's tokens. `reset` records a `--namespace-*: initial`, which
/// drops the built-in scale as well, matching how upstream ships its own scales
/// as theme tokens.
#[derive(Debug, Default, Clone, PartialEq)]
struct Namespace {
  entries: HashMap<Box<str>, Option<Box<str>>>,
  reset: bool,
}

/// Design tokens that override or extend the built-in Tailwind scales.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct Theme {
  values: HashMap<TwNamespace, Namespace>,
}

impl Theme {
  /// Records one `--namespace-key: value` entry, ignoring keys outside a known
  /// namespace. `initial` removes a token, and a `*` key clears the namespace.
  pub fn insert(&mut self, key: &str, value: &str) {
    let Some((namespace, token)) = NAMESPACES
      .iter()
      .find_map(|namespace| Some((*namespace, key.strip_prefix(namespace.prefix())?)))
    else {
      return;
    };

    let entry = self.values.entry(namespace).or_default();

    if token == "*" {
      entry.entries.clear();
      entry.reset = true;
      return;
    }

    let value = (value != "initial").then(|| value.into());

    entry.entries.insert(token.into(), value);
  }

  /// Whether no token is defined, which keeps utility parsing on the built-in path.
  pub fn is_empty(&self) -> bool {
    self
      .values
      .values()
      .all(|namespace| namespace.entries.is_empty() && !namespace.reset)
  }

  pub(crate) fn lookup(&self, namespace: TwNamespace, token: &str) -> ThemeLookup<'_> {
    let Some(namespace) = self.values.get(&namespace) else {
      return ThemeLookup::Missing;
    };

    match namespace.entries.get(token) {
      Some(Some(value)) => ThemeLookup::Value(value),
      Some(None) => ThemeLookup::Removed,
      None if namespace.reset => ThemeLookup::Removed,
      None => ThemeLookup::Missing,
    }
  }
}
