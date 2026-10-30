//! The faces a document's `@font-face` rules add to a render, after Blink's
//! [`FontFaceSetDocument`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/css/font_face_set_document.cc):
//! the host loads each `src` file and hands it over by URL, and each rule makes the face its
//! first usable source gives.

use std::{
  collections::HashMap,
  sync::{Arc, LazyLock},
};

use quick_cache::{Weighter, sync::Cache};
use skrifa::{FontRef, raw::TableProvider};

use super::{FontError, FontRegistry, FontSource, Fonts, FontsSnapshot, SharedBytes};
use crate::{
  fonts::{
    css_font_selector::CssFontSelector,
    font_face::{FontFace, PendingSource},
  },
  resources::unicode_range::UnicodeRangeSet,
  style::{StyleSheet, font_face_rule::FontFaceRule},
  viewport::Viewport,
};

/// An `@font-face` file the host has loaded, or failed to.
#[derive(Clone)]
pub(super) enum FontFile {
  Loaded { data: SharedBytes, id: u64 },
  Failed,
}

/// The bytes the `unicode-range` copies of one document's faces may take, past which a face that
/// needs another copy fails to load, so a stylesheet cannot multiply a large file without bound.
const MAX_RESTRICTED_BYTES: usize = 256 << 20;

/// The least a cached document registry weighs, so the cache holds at most 16 of them.
const MIN_DOCUMENT_WEIGHT: u64 = 16 << 20;

/// The registries the rules of recent renders made, so the same rules over the same fonts reuse
/// their decoded faces, weighed by their `unicode-range` copies.
static DOCUMENT_REGISTRIES: LazyLock<Cache<DocumentKey, DocumentFonts, ByCopies>> =
  LazyLock::new(|| Cache::with_weighter(16, MAX_RESTRICTED_BYTES as u64, ByCopies));

/// A document's registry, and the bytes of the `unicode-range` copies it alone holds.
#[derive(Clone)]
struct DocumentFonts {
  registry: Arc<FontRegistry>,
  copied_bytes: u64,
}

#[derive(Clone)]
struct ByCopies;

impl Weighter<DocumentKey, DocumentFonts> for ByCopies {
  fn weight(&self, _key: &DocumentKey, fonts: &DocumentFonts) -> u64 {
    fonts.copied_bytes.max(MIN_DOCUMENT_WEIGHT)
  }
}

/// What a document's faces are made of: the registry they extend, and each face's descriptors,
/// as written, with the URL of the source it reaches.
#[derive(Clone, PartialEq, Eq, Hash)]
struct DocumentKey {
  base: u64,
  faces: Vec<(String, Arc<str>)>,
}

/// The `unicode-range` copies of the files one document's faces draw with, one per file and range.
#[derive(Default)]
struct RestrictedFiles {
  copies: HashMap<(u64, String), (SharedBytes, u64)>,
  bytes: usize,
}

impl RestrictedFiles {
  /// `data`, which `id` names, with its `cmap` limited to `range`, none once the copies would
  /// pass [`MAX_RESTRICTED_BYTES`].
  fn restrict(
    &mut self,
    data: &SharedBytes,
    id: u64,
    range: &UnicodeRangeSet,
  ) -> Option<(SharedBytes, u64)> {
    if range.is_entire_range() {
      return Some((Arc::clone(data), id));
    }

    let key = (id, format!("{range:?}"));

    if let Some((data, id)) = self.copies.get(&key) {
      return Some((Arc::clone(data), *id));
    }

    let len = (**data).as_ref().len();

    if self.bytes + len > MAX_RESTRICTED_BYTES {
      return None;
    }

    self.bytes += len;

    let restricted = range.restrict_sfnt(Arc::clone(data), id);

    self
      .copies
      .insert(key, (Arc::clone(&restricted.0), restricted.1));

    Some(restricted)
  }
}

impl Fonts {
  /// Hands over the file an `@font-face` `src` names, as [`RenderNeeds::fonts`] reports it. A file
  /// that is not a usable font fails, as Blink's OTS sanitizing fails a load, so the host can
  /// record it with [`Self::fail_font_file`].
  ///
  /// [`RenderNeeds::fonts`]: crate::layout::needs::RenderNeeds::fonts
  pub fn load_font_file(
    &mut self,
    url: impl Into<Arc<str>>,
    source: FontSource<'_>,
  ) -> Result<(), FontError> {
    let (data, id) = source.into_sfnt_with_id()?;

    if !is_usable_font(&data) {
      return Err(FontError::UnsupportedFormat);
    }
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
    let key = DocumentKey {
      base: base.revision,
      faces: sources
        .iter()
        .map(|source| {
          (
            format!("{:?}", source.rule.descriptors),
            Arc::clone(source.url),
          )
        })
        .collect(),
    };
    let registry = DOCUMENT_REGISTRIES
      .get_or_insert_with(&key, || {
        let mut registry = (**base).clone();
        let mut restricted = RestrictedFiles::default();

        for source in sources {
          if let Some(font_face) = source.font_face(&registry.selector, &mut restricted) {
            registry.selector.add_css_connected(font_face);
          }
        }

        registry.revise();

        Ok::<_, ()>(DocumentFonts {
          registry: Arc::new(registry),
          copied_bytes: restricted.bytes as u64,
        })
      })
      .map_or_else(|()| Arc::clone(base), |fonts| fonts.registry);

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
  fn font_face(
    &self,
    selector: &CssFontSelector,
    restricted: &mut RestrictedFiles,
  ) -> Option<FontFace> {
    let family = self.rule.descriptors.family.clone()?;
    let descriptors = self.rule.descriptors.clone();

    match self.loaded {
      Some((data, id)) => {
        let (data, id) = restricted.restrict(data, id, &descriptors.unicode_range)?;
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

/// Whether `data` is a font with the tables a face draws from. Approximate: Blink's OTS sanitizer
/// checks every table, where this only requires `head`, `hhea`, `maxp`, and `cmap`.
fn is_usable_font(data: &SharedBytes) -> bool {
  FontRef::from_index((**data).as_ref(), 0).is_ok_and(|font| {
    font.head().is_ok() && font.hhea().is_ok() && font.maxp().is_ok() && font.cmap().is_ok()
  })
}
