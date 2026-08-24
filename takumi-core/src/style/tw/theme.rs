use std::collections::HashMap;

/// A theme namespace, spelled as the CSS custom-property prefix it reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TwNamespace {
  /// `--color-*`
  Color,
  /// `--spacing-*`
  Spacing,
  /// `--container-*`
  Container,
  /// `--text-*`
  Text,
  /// `--font-*`
  Font,
  /// `--font-weight-*`
  FontWeight,
  /// `--tracking-*`
  Tracking,
  /// `--leading-*`
  Leading,
  /// `--radius-*`
  Radius,
  /// `--shadow-*`
  Shadow,
  /// `--drop-shadow-*`
  DropShadow,
  /// `--text-shadow-*`
  TextShadow,
  /// `--blur-*`
  Blur,
  /// `--aspect-*`
  Aspect,
  /// `--animate-*`
  Animate,
}

impl TwNamespace {
  const fn prefix(self) -> &'static str {
    match self {
      TwNamespace::Color => "--color-",
      TwNamespace::Spacing => "--spacing-",
      TwNamespace::Container => "--container-",
      TwNamespace::Text => "--text-",
      TwNamespace::Font => "--font-",
      TwNamespace::FontWeight => "--font-weight-",
      TwNamespace::Tracking => "--tracking-",
      TwNamespace::Leading => "--leading-",
      TwNamespace::Radius => "--radius-",
      TwNamespace::Shadow => "--shadow-",
      TwNamespace::DropShadow => "--drop-shadow-",
      TwNamespace::TextShadow => "--text-shadow-",
      TwNamespace::Blur => "--blur-",
      TwNamespace::Aspect => "--aspect-",
      TwNamespace::Animate => "--animate-",
    }
  }
}

/// Namespaces ordered longest-prefix-first, so `--font-weight-bold` never lands
/// in `--font-*`. Upstream spells the same rule as an explicit ignore list.
const NAMESPACES: &[TwNamespace] = &[
  TwNamespace::FontWeight,
  TwNamespace::TextShadow,
  TwNamespace::DropShadow,
  TwNamespace::Container,
  TwNamespace::Tracking,
  TwNamespace::Spacing,
  TwNamespace::Leading,
  TwNamespace::Animate,
  TwNamespace::Aspect,
  TwNamespace::Radius,
  TwNamespace::Shadow,
  TwNamespace::Color,
  TwNamespace::Text,
  TwNamespace::Font,
  TwNamespace::Blur,
];

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

  /// Builds a theme from entries whose order carries no meaning, such as a JS
  /// object. Namespace resets are applied first so they cannot clear a token
  /// the same batch defines.
  pub fn from_unordered<I: IntoIterator<Item = (String, String)>>(entries: I) -> Self {
    let entries = entries.into_iter().collect::<Vec<_>>();
    let mut theme = Theme::default();

    for (key, value) in entries.iter().filter(|(key, _)| key.ends_with("-*")) {
      theme.insert(key, value);
    }

    for (key, value) in entries.iter().filter(|(key, _)| !key.ends_with("-*")) {
      theme.insert(key, value);
    }

    theme
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

#[cfg(test)]
mod tests {
  use super::{NAMESPACES, Theme, ThemeLookup, TwNamespace};

  #[test]
  fn test_namespaces_are_ordered_longest_prefix_first() {
    let lengths = NAMESPACES
      .iter()
      .map(|namespace| namespace.prefix().len())
      .collect::<Vec<_>>();

    assert!(
      lengths.windows(2).all(|pair| pair[0] >= pair[1]),
      "a shorter prefix placed first would swallow a longer one: {lengths:?}"
    );
  }

  #[test]
  fn test_longest_prefix_wins() {
    let mut theme = Theme::default();

    theme.insert("--font-weight-bold", "700");

    assert!(matches!(
      theme.lookup(TwNamespace::FontWeight, "bold"),
      ThemeLookup::Value("700")
    ));
    assert!(matches!(
      theme.lookup(TwNamespace::Font, "weight-bold"),
      ThemeLookup::Missing
    ));
  }
}
