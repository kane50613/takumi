//! The last source text a renderer parsed, so a render that follows `needs` on the same HTML or
//! JSON reads it once.

use std::sync::{Arc, Mutex};

use takumi_core::layout::node::Node;

/// What a source text holds: the node tree, and the CSS of its `<style>` elements.
pub type ParsedSource = (Node, Vec<String>);

/// How a source text reads, which keeps the same text read two ways apart.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SourceKind {
  /// HTML markup.
  Html,
  /// A node tree as JSON.
  Json,
}

/// A source text, how it reads, and what it parsed to.
struct CachedSource {
  kind: SourceKind,
  text: Box<str>,
  parsed: Arc<ParsedSource>,
}

/// The one source text a renderer parsed last.
#[derive(Default)]
pub struct SourceCache(Mutex<Option<CachedSource>>);

impl SourceCache {
  /// `text` read as `kind`, parsed with `parse` unless it is the text parsed last.
  pub fn parse<E>(
    &self,
    kind: SourceKind,
    text: &str,
    parse: impl FnOnce(&str) -> Result<ParsedSource, E>,
  ) -> Result<ParsedSource, E> {
    let cached = self.0.lock().ok().and_then(|last| {
      last
        .as_ref()
        .filter(|held| held.kind == kind && *held.text == *text)
        .map(|held| Arc::clone(&held.parsed))
    });

    if let Some(parsed) = cached {
      return Ok(ParsedSource::clone(&parsed));
    }

    let parsed = parse(text)?;

    if let Ok(mut last) = self.0.lock() {
      *last = Some(CachedSource {
        kind,
        text: text.into(),
        parsed: Arc::new(parsed.clone()),
      });
    }

    Ok(parsed)
  }
}

#[cfg(test)]
mod tests {
  use std::cell::Cell;

  use super::*;

  #[test]
  fn parses_the_same_text_once() {
    let cache = SourceCache::default();
    let parses = Cell::new(0);
    let parse = |text: &str| -> Result<ParsedSource, ()> {
      parses.set(parses.get() + 1);
      Ok((Node::text(text), Vec::new()))
    };

    cache.parse(SourceKind::Json, "a", parse).unwrap();
    cache.parse(SourceKind::Json, "a", parse).unwrap();
    cache.parse(SourceKind::Html, "a", parse).unwrap();
    cache.parse(SourceKind::Json, "b", parse).unwrap();

    assert_eq!(parses.get(), 3);
  }
}
