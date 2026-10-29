//! The faces a document's `@font-face` rules add to a render, after Blink's
//! [`FontFaceSetDocument`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/css/font_face_set_document.cc):
//! the host loads each `src` file and hands it over by URL, and each rule makes the face its
//! first usable source gives.

use std::sync::{Arc, LazyLock};

use quick_cache::sync::Cache;
use xxhash_rust::xxh3::Xxh3;

use super::{FontError, FontRegistry, FontSource, Fonts, FontsSnapshot, SharedBytes};
use crate::{
  fonts::{
    css_font_selector::CssFontSelector,
    font_face::{FontFace, PendingSource},
  },
  style::{StyleSheet, font_face_rule::FontFaceRule},
  viewport::Viewport,
};

/// An `@font-face` file the host has loaded, or failed to.
#[derive(Clone)]
pub(super) enum FontFile {
  Loaded { data: SharedBytes, id: u64 },
  Failed,
}

/// The registries the rules of recent renders made, by [`FontRegistry::revision`], so the same
/// rules over the same fonts reuse their decoded faces.
static DOCUMENT_REGISTRIES: LazyLock<Cache<u64, Arc<FontRegistry>>> =
  LazyLock::new(|| Cache::new(16));

impl Fonts {
  /// Hands over the file an `@font-face` `src` names, as [`RenderNeeds::fonts`] reports it.
  ///
  /// [`RenderNeeds::fonts`]: crate::layout::needs::RenderNeeds::fonts
  pub fn load_font_file(
    &mut self,
    url: impl Into<Arc<str>>,
    source: FontSource<'_>,
  ) -> Result<(), FontError> {
    let (data, id) = source.into_sfnt_with_id()?;
    let registry = Arc::make_mut(&mut self.registry);

    registry.revise();
    registry
      .font_files
      .insert(url.into(), FontFile::Loaded { data, id });

    Ok(())
  }

  /// Records that the file `url` names failed to load, so a rule moves on to its next `src`.
  pub fn fail_font_file(&mut self, url: impl Into<Arc<str>>) {
    let registry = Arc::make_mut(&mut self.registry);

    registry.revise();
    registry.font_files.insert(url.into(), FontFile::Failed);
  }
}

impl FontsSnapshot {
  /// This snapshot with the faces of the `@font-face` rules in force at `viewport` added, in
  /// Blink's cascade order: by layer, unlayered rules last, then in source order.
  pub(crate) fn with_font_faces(self, stylesheet: &StyleSheet, viewport: Viewport) -> Self {
    let mut rules: Vec<&FontFaceRule> = stylesheet
      .font_faces()
      .iter()
      .filter(|rule| {
        rule
          .media_queries
          .iter()
          .all(|media_query| media_query.matches(viewport))
      })
      .collect();

    if rules.is_empty() {
      return self;
    }

    rules.sort_by_key(|rule| rule.layer_order.unwrap_or(usize::MAX));

    let base = &self.0.registry;
    let sources: Vec<RuleSource> = rules
      .into_iter()
      .filter_map(|rule| base.source_of(rule))
      .collect();
    let mut revision = Xxh3::new();

    revision.update(&base.revision.to_le_bytes());

    for source in &sources {
      revision.update(format!("{:?}", source.rule.descriptors).as_bytes());
      revision.update(source.url.as_bytes());
    }

    // The high bit keeps a derived stamp apart from the counter `revise` draws from, while the
    // same rules over the same registry stamp alike, so font-dependent caches still hit.
    let revision = revision.digest() | 1 << 63;
    let registry = DOCUMENT_REGISTRIES
      .get_or_insert_with(&revision, || {
        let mut registry = (**base).clone();

        for source in sources {
          if let Some(font_face) = source.font_face(&registry.selector) {
            registry.selector.add_css_connected(font_face);
          }
        }

        registry.revision = revision;

        Ok::<_, ()>(Arc::new(registry))
      })
      .unwrap_or_else(|()| Arc::clone(base));

    Self(Fonts {
      registry,
      fallback_families: self.0.fallback_families,
    })
  }
}

/// A rule and the source Blink's `CSSFontFace::GetFontData` reaches: its first that has not failed.
struct RuleSource<'r> {
  rule: &'r FontFaceRule,
  url: &'r Arc<str>,
  /// The file the host loaded for `url`, none while it has not.
  loaded: Option<(&'r SharedBytes, u64)>,
}

impl FontRegistry {
  /// The source `rule` reaches, none when every source has failed.
  fn source_of<'r>(&'r self, rule: &'r FontFaceRule) -> Option<RuleSource<'r>> {
    rule.sources.iter().find_map(|url| {
      let loaded = match self.font_files.get(url) {
        Some(FontFile::Loaded { data, id }) => Some((data, *id)),
        Some(FontFile::Failed) => return None,
        None => None,
      };

      Some(RuleSource { rule, url, loaded })
    })
  }
}

impl RuleSource<'_> {
  /// Blink's `FontFace::Create` for the rule, drawing with the loaded file or waiting on the URL.
  /// A file that does not parse as a font makes no face.
  fn font_face(&self, selector: &CssFontSelector) -> Option<FontFace> {
    let family = self.rule.descriptors.family.clone()?;
    let descriptors = self.rule.descriptors.clone();

    match self.loaded {
      Some((data, id)) => {
        let (data, id) = descriptors
          .unicode_range
          .restrict_sfnt(Arc::clone(data), id);
        let platform_data = selector.platform_data(&data, id)?;

        Some(FontFace::new(family, descriptors, platform_data))
      }
      None => Some(FontFace::pending(
        family,
        descriptors,
        PendingSource::Url(Arc::clone(self.url)),
      )),
    }
  }
}
