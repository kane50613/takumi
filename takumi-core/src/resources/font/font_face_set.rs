//! The faces a document's `@font-face` rules add to a render, after Blink's
//! [`FontFaceSetDocument`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/css/font_face_set_document.cc):
//! the host loads each `src` file and hands it over by URL, and each rule makes the face its
//! first usable source gives.

use crate::sort_key::sort_by_key;
use std::sync::Arc;

use skrifa::{FontRef, raw::TableProvider};

use super::{FontError, FontRegistry, FontSource, Fonts, FontsSnapshot, SharedBytes};
use crate::{
  fonts::{
    css_font_selector::CssFontSelector,
    font_custom_platform_data::FontCustomPlatformData,
    font_face::{FontFace, PendingSource},
  },
  resources::cache::{KindStore, Weighed},
  style::{
    StyleSheet,
    font_face_rule::{FontFaceRule, FontFaceSrc},
  },
  viewport::Viewport,
};

/// An `@font-face` file the host has loaded, or failed to.
#[derive(Clone)]
pub(super) enum FontFile {
  Loaded { data: SharedBytes, id: u64 },
  Failed,
}

/// The registries the `@font-face` rules of recent renders made, which a [`Cache`](crate::Cache)
/// keeps so the same rules over the same fonts reuse their decoded faces. A file
/// [`Fonts::forget_font_file`] releases stays held until the registries holding it are evicted.
pub(crate) struct DocumentRegistries {
  store: KindStore<CachedRegistry>,
}

#[derive(Clone)]
pub(crate) struct CachedRegistry {
  registry: Arc<FontRegistry>,
  /// The bytes of the font files the rules' faces hold.
  bytes: usize,
}

impl Weighed for CachedRegistry {
  fn bytes(&self) -> usize {
    self.bytes
  }
}

impl DocumentRegistries {
  pub(crate) fn new(store: KindStore<CachedRegistry>) -> Self {
    Self { store }
  }
}

/// What a document's faces are made of: the registry they extend, and each face's descriptors
/// and the source it reaches, as written.
#[derive(Clone, PartialEq, Eq, Hash)]
pub(crate) struct DocumentKey {
  base: u64,
  faces: Vec<(String, String)>,
}

impl Fonts {
  /// Hands over the file an `@font-face` `src` names, as [`RenderNeeds::font_files`] reports it. A file
  /// that is not a usable font fails, as Blink's OTS sanitizing fails a load, so the host can
  /// record it with [`Self::with_failed_font_files`].
  ///
  /// [`RenderNeeds::font_files`]: crate::layout::needs::RenderNeeds::font_files
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

  /// Lets go of the file `url` names, so a rule waits on it again; false when it held none.
  pub fn forget_font_file(&mut self, url: &str) -> bool {
    if !self.registry.font_files.contains_key(url) {
      return false;
    }

    let registry = Arc::make_mut(&mut self.registry);

    registry.revise();
    registry.font_files.remove(url);

    true
  }

  /// These fonts with each of `urls` failed to load, as one document's downloads did. The revision
  /// stays, since only `@font-face` rules read the files, and the document cache keys on the
  /// source each rule reaches, so renders of one document with the same failures share its faces.
  pub fn with_failed_font_files(&self, urls: &[String]) -> Self {
    let mut fonts = self.clone();
    let registry = Arc::make_mut(&mut fonts.registry);

    for url in urls {
      registry
        .font_files
        .insert(url.as_str().into(), FontFile::Failed);
    }

    fonts
  }
}

impl FontsSnapshot {
  /// This snapshot with the faces of the `@font-face` rules in force at `viewport` added, in
  /// Blink's cascade order: by layer, unlayered rules last, then in source order.
  pub(crate) fn with_font_faces(
    self,
    stylesheet: &StyleSheet,
    viewport: Viewport,
    registries: &DocumentRegistries,
  ) -> Self {
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

    sort_by_key(&mut rules, |rule| rule.layer_order.unwrap_or(usize::MAX));

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
            format!("{:?}", source.src),
          )
        })
        .collect(),
    };
    let bytes = sources.iter().map(RuleSource::file_bytes).sum();
    let registry = registries
      .store
      .get_or_insert_with(key, || {
        let mut registry = (**base).clone();

        for source in sources {
          if let Some(font_face) = source.font_face(&registry.selector) {
            registry.selector.add_css_connected(font_face);
          }
        }

        registry.revise();

        Ok::<_, ()>(CachedRegistry {
          registry: Arc::new(registry),
          bytes,
        })
      })
      .map_or_else(|()| Arc::clone(base), |cached| cached.registry);

    Self(Fonts {
      registry,
      fallback_families: self.0.fallback_families,
    })
  }
}

/// A rule and the source Blink's `CSSFontFace::GetFontData` reaches: its first that has not failed.
struct RuleSource<'r> {
  rule: &'r FontFaceRule,
  src: &'r FontFaceSrc,
  data: SourceData<'r>,
}

/// What a reached source draws with.
enum SourceData<'r> {
  /// The file the host loaded for a `url()`.
  File(&'r SharedBytes, u64),
  /// The registered face a `local()` names.
  Local(&'r Arc<FontCustomPlatformData>),
  /// A `url()` the host has not loaded yet.
  Pending(&'r Arc<str>),
}

impl FontRegistry {
  /// The source `rule` reaches, none when every source has failed or names no registered face.
  fn source_of<'r>(&'r self, rule: &'r FontFaceRule) -> Option<RuleSource<'r>> {
    rule.sources.iter().find_map(|src| {
      let data = match src {
        FontFaceSrc::Url(url) => match self.font_files.get(url) {
          Some(FontFile::Loaded { data, id }) => SourceData::File(data, *id),
          Some(FontFile::Failed) => return None,
          None => SourceData::Pending(url),
        },
        FontFaceSrc::Local(name) => SourceData::Local(self.selector.local_face(name)?),
      };

      Some(RuleSource { rule, src, data })
    })
  }
}

impl RuleSource<'_> {
  /// The bytes of the loaded file the source draws with, none for a face it shares or awaits.
  fn file_bytes(&self) -> usize {
    match self.data {
      SourceData::File(data, _) => (**data).as_ref().len(),
      SourceData::Local(_) | SourceData::Pending(_) => 0,
    }
  }

  /// Blink's `FontFace::Create` for the rule, drawing with the source's data or waiting on its
  /// URL. Data that does not parse as a font makes no face.
  fn font_face(&self, selector: &CssFontSelector) -> Option<FontFace> {
    let family = self.rule.descriptors.family.clone()?;
    let descriptors = self.rule.descriptors.clone();
    let (data, id, index) = match self.data {
      SourceData::File(data, id) => (data, id, 0),
      SourceData::Local(typeface) => (&typeface.data, typeface.id, typeface.index),
      SourceData::Pending(url) => {
        return Some(FontFace::pending(
          family,
          descriptors,
          PendingSource::Url(Arc::clone(url)),
        ));
      }
    };
    let platform_data = selector.platform_data(data, id, index)?;

    Some(FontFace::new(family, descriptors, platform_data))
  }
}

/// Whether `data` is a font with the tables a face draws from. Approximate: Blink's OTS sanitizer
/// checks every table, where this only requires `head`, `hhea`, `maxp`, and `cmap`.
fn is_usable_font(data: &SharedBytes) -> bool {
  FontRef::from_index((**data).as_ref(), 0).is_ok_and(|font| {
    font.head().is_ok() && font.hhea().is_ok() && font.maxp().is_ok() && font.cmap().is_ok()
  })
}
