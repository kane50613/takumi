mod builder;
/// The `--color-*` style prefixes a utility reads its value from.
mod namespace;
/// Parsers for Tailwind utility-class suffixes.
mod parser;
mod property_order;
mod rule;
mod utility;
mod variant;

use std::{
  borrow::Cow,
  convert::Infallible,
  str::FromStr,
  sync::{Arc, LazyLock},
};

pub(crate) use namespace::Namespace;
use quick_cache::{Weighter, sync::Cache};
pub(crate) use rule::TwRule;
use serde::{Deserialize, Deserializer, de::Error as DeError};
pub(crate) use variant::{Breakpoint, BreakpointOverrides};

use crate::{style::*, viewport::Viewport};

/// Tailwind v4 `--spacing` (rem per unit). Prefer [`Length::from_spacing`].
pub(crate) const TW_VAR_SPACING: f32 = 0.25;

/// Expands an `@apply` list into declarations, `None` for an unknown token or a variant.
pub(crate) fn expand_apply(source: &str) -> Option<StyleDeclarationBlock> {
  let rules = source
    .split_whitespace()
    .map(TwRule::parse)
    .collect::<Option<Vec<_>>>()?;

  if rules.iter().any(|rule| rule.breakpoint.is_some()) {
    return None;
  }

  Some(cascaded_block(&rules))
}

/// `rules` in the order the cascade applies them: the normal ones, then the `!important` ones,
/// each in Tailwind's sort.
fn cascade_order<'r>(rules: impl IntoIterator<Item = &'r TwRule>) -> Vec<&'r TwRule> {
  let mut rules: Vec<&TwRule> = rules.into_iter().collect();

  TwRule::sort(&mut rules, |left, right| {
    (left.important, &left.order).cmp(&(right.important, &right.order))
  });
  rules
}

/// The declarations of `rules`, in the order the cascade applies them.
fn cascaded_block<'r>(rules: impl IntoIterator<Item = &'r TwRule>) -> StyleDeclarationBlock {
  let mut block = StyleDeclarationBlock::default();

  for rule in cascade_order(rules) {
    block.extend_element_state(rule.declarations.element_state.iter().map(Box::as_ref));
    for declaration in rule.declarations.iter() {
      block.push(declaration.clone(), rule.important);
    }
  }

  block
}

/// Represents a collection of tailwind properties.
#[derive(Debug, Clone)]
pub struct TailwindValues {
  inner: Arc<Vec<Arc<TwRule>>>,
}

const TAILWIND_CACHE_MAX_BYTES: usize = 1 << 20;

/// Bytes charged for an entry's own bookkeeping on top of its payload.
const ENTRY_OVERHEAD: usize = 64;

#[derive(Clone)]
struct ByBytes;

impl Weighter<String, TailwindValues> for ByBytes {
  fn weight(&self, source: &String, values: &TailwindValues) -> u64 {
    (source.len() + values.estimated_bytes() + ENTRY_OVERHEAD).max(1) as u64
  }
}

static PARSED: LazyLock<Cache<String, TailwindValues, ByBytes>> = LazyLock::new(|| {
  let max_bytes = TAILWIND_CACHE_MAX_BYTES as u64;
  // ~256 bytes per parsed list ⇒ an item-count hint for the budget.
  let estimated_items = (max_bytes / 256).max(1) as usize;

  Cache::with_weighter(estimated_items, max_bytes, ByBytes)
});

impl FromStr for TailwindValues {
  type Err = String;

  fn from_str(source: &str) -> Result<Self, Self::Err> {
    Ok(
      PARSED
        .get_or_insert_with::<str, Infallible>(source, || Ok(Self::parse(source)))
        .unwrap_or_else(|never| match never {}),
    )
  }
}

impl PartialEq for TailwindValues {
  fn eq(&self, other: &Self) -> bool {
    Arc::ptr_eq(&self.inner, &other.inner) || self.inner == other.inner
  }
}

impl TailwindValues {
  /// Approximate retained size in bytes, for cache budgeting. Counts the
  /// compiled rules, not the heap their declarations own.
  fn estimated_bytes(&self) -> usize {
    self.inner.capacity() * size_of::<Arc<TwRule>>() + self.inner.len() * size_of::<TwRule>()
  }

  fn parse(source: &str) -> Self {
    TailwindValues {
      inner: Arc::new(
        source
          .split_whitespace()
          .filter_map(TwRule::parse)
          .map(Arc::new)
          .collect(),
      ),
    }
  }

  /// The utilities' rules, in the order they were written.
  pub(crate) fn rules(&self) -> &[Arc<TwRule>] {
    &self.inner
  }

  /// Collects resource URLs referenced by active Tailwind utilities for the given viewport.
  pub(crate) fn image_urls(
    &self,
    viewport: Viewport,
    breakpoints: &BreakpointOverrides,
  ) -> impl Iterator<Item = &str> {
    self
      .inner
      .iter()
      .filter(|rule| rule.is_active(viewport, breakpoints))
      .flat_map(|rule| rule.declarations.image_urls())
      .collect::<Vec<_>>()
      .into_iter()
  }

  /// The declarations of the utilities active at the viewport, in the order the cascade applies
  /// them.
  #[cfg(test)]
  pub(crate) fn into_declaration_block(
    self,
    viewport: Viewport,
    breakpoints: &BreakpointOverrides,
  ) -> StyleDeclarationBlock {
    cascaded_block(
      self
        .inner
        .iter()
        .map(Arc::as_ref)
        .filter(|rule| rule.is_active(viewport, breakpoints)),
    )
  }
}

impl<'de> Deserialize<'de> for TailwindValues {
  fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
  where
    D: Deserializer<'de>,
  {
    let string = String::deserialize(deserializer)?;

    TailwindValues::from_str(&string).map_err(D::Error::custom)
  }
}

/// The percentage a `/50` colour modifier spells, within `0..=100`.
pub(crate) fn parse_opacity_modifier(modifier: &str) -> Option<f32> {
  modifier
    .parse::<f32>()
    .ok()
    .filter(|percentage| (0.0..=100.0).contains(percentage))
}

/// `color` mixed with transparent the way Tailwind's opacity modifier compiles.
pub(crate) fn with_opacity(color: &str, percentage: f32) -> String {
  format!("color-mix(in oklab, {color} {percentage}%, transparent)")
}

pub(crate) fn extract_arbitrary_value(suffix: &str) -> Option<Cow<'_, str>> {
  let value = suffix.strip_prefix('[')?.strip_suffix(']')?;
  Some(decode_arbitrary_value(value))
}

enum FnKind {
  Url,
  VarTheme,
  Other,
}

impl FnKind {
  fn from_name(name: &str) -> FnKind {
    if name == "url" || name.ends_with("_url") {
      FnKind::Url
    } else if matches!(name, "var" | "theme") || name.ends_with("_var") || name.ends_with("_theme")
    {
      FnKind::VarTheme
    } else {
      FnKind::Other
    }
  }
}

fn is_ident_byte(byte: u8) -> bool {
  byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_'
}

/// Whether `token` is a non-empty run of identifier bytes.
pub(crate) fn is_ident(token: &str) -> bool {
  !token.is_empty() && token.bytes().all(is_ident_byte)
}

/// Tailwind's `decodeArbitraryValue`.
fn decode_arbitrary_value(value: &str) -> Cow<'_, str> {
  if !value.contains('_') {
    return Cow::Borrowed(value);
  }

  let bytes = value.as_bytes();
  let mut out = String::with_capacity(value.len());
  let mut stack: Vec<(FnKind, bool)> = Vec::new();
  let mut ident_start = 0;
  let mut index = 0;
  while index < bytes.len() {
    let byte = bytes[index];
    if byte == b'\\' && bytes.get(index + 1) == Some(&b'_') {
      out.push('_');
      index += 2;
      ident_start = index;
    } else if byte == b'_' {
      let preserved = stack.iter().any(|(kind, _)| matches!(kind, FnKind::Url))
        || matches!(stack.last(), Some((FnKind::VarTheme, true)));
      out.push(if preserved { '_' } else { ' ' });
      index += 1;
    } else if byte == b'(' {
      stack.push((FnKind::from_name(&value[ident_start..index]), true));
      out.push('(');
      index += 1;
      ident_start = index;
    } else if byte == b')' {
      stack.pop();
      out.push(')');
      index += 1;
      ident_start = index;
    } else if byte == b',' {
      if let Some((_, first_arg)) = stack.last_mut() {
        *first_arg = false;
      }
      out.push(',');
      index += 1;
      ident_start = index;
    } else if is_ident_byte(byte) {
      out.push(byte as char);
      index += 1;
    } else {
      let char_len = value[index..].chars().next().map_or(1, char::len_utf8);
      out.push_str(&value[index..index + char_len]);
      index += char_len;
      ident_start = index;
    }
  }
  Cow::Owned(out)
}

/// A trait for parsing tailwind properties.
pub(crate) trait TailwindPropertyParser: Sized + for<'i> FromCss<'i> {
  /// Parse a tailwind property from a token. Defaults to the type's `FromCss`
  /// parser; override for keywords Tailwind spells differently from CSS.
  fn parse_tw(token: &str) -> Option<Self> {
    Self::from_css_str(token).ok()
  }

  /// The variable namespaces this value type reads, in order.
  const NAMESPACES: &'static [Namespace] = &[];

  /// Parse a tailwind property from a token, with support for arbitrary values.
  fn parse_tw_with_arbitrary(token: &str) -> Option<Self> {
    if let Some(value) = extract_arbitrary_value(token) {
      return Self::from_css_str(&value).ok();
    }

    Self::parse_tw(token)
  }
}

#[cfg(test)]
#[allow(clippy::panic, clippy::unwrap_used)]
mod tests;
