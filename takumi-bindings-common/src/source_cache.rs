//! The last source text a renderer parsed, so a render that follows `needs` on the same HTML or
//! JSON reads it once.

use std::sync::{Arc, Mutex};

use takumi_core::layout::node::Node;
use xxhash_rust::xxh3::xxh3_64_with_seed;

/// What a source text holds: the node tree, and the CSS of its `<style>` elements.
pub type ParsedSource = (Node, Vec<String>);

/// How a source text reads, which keeps the same text read two ways apart.
#[derive(Clone, Copy)]
pub enum SourceKind {
  /// HTML markup.
  Html = 1,
  /// A node tree as JSON.
  Json = 2,
}

/// The one source text a renderer parsed last, by a hash of its kind and text.
#[derive(Default)]
pub struct SourceCache(Mutex<Option<(u64, Arc<ParsedSource>)>>);

impl SourceCache {
  /// `text` read as `kind`, parsed with `parse` unless it is the text parsed last.
  pub fn parse<E>(
    &self,
    kind: SourceKind,
    text: &str,
    parse: impl FnOnce(&str) -> Result<ParsedSource, E>,
  ) -> Result<ParsedSource, E> {
    let key = xxh3_64_with_seed(text.as_bytes(), kind as u64);
    let cached = self.0.lock().ok().and_then(|last| {
      last
        .as_ref()
        .filter(|(held, _)| *held == key)
        .map(|(_, parsed)| Arc::clone(parsed))
    });

    if let Some(parsed) = cached {
      return Ok(ParsedSource::clone(&parsed));
    }

    let parsed = parse(text)?;

    if let Ok(mut last) = self.0.lock() {
      *last = Some((key, Arc::new(parsed.clone())));
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
