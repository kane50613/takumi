//! The faces a document's `@font-face` rules add to a render, after Blink's
//! [`FontFaceSetDocument`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/css/font_face_set_document.cc):
//! the host loads each `src` file and hands it over by URL, and each rule makes the face its
//! first usable source gives.

use crate::sort_key::sort_by_key;
use std::sync::Arc;

use rustc_hash::FxHashMap;
use skrifa::{FontRef, raw::TableProvider};

use super::{FontError, FontRegistry, FontSource, Fonts, FontsSnapshot, SharedBytes};
use crate::{
  fonts::{
    css_font_selector::CssFontSelector,
    font_custom_platform_data::{FontCustomPlatformData, UNITS_PER_EM_RANGE},
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
/// Each registry is weighed by the bytes of the font files its rules' faces hold.
pub(crate) type DocumentRegistries = KindStore<Arc<FontRegistry>>;

/// What a document's faces are made of: the registry they extend, and each face's descriptors,
/// the source it reaches as written, and the id of the file that source loaded.
#[derive(Clone, PartialEq, Eq, Hash)]
pub(crate) struct DocumentKey {
  base: u64,
  faces: Vec<(String, String, Option<u64>)>,
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
    // Files with the same bytes share one copy, so a host that charges a file by its id charges what it holds.
    let data = registry
      .font_files
      .values()
      .find_map(|file| match file {
        FontFile::Loaded { data, id: held } if *held == id => Some(Arc::clone(data)),
        _ => None,
      })
      .unwrap_or(data);

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

    Arc::make_mut(&mut self.registry).font_files.remove(url);

    true
  }

  /// The URLs of the loaded `@font-face` files, by the id of their data.
  pub(crate) fn loaded_font_files(&self) -> FxHashMap<u64, Vec<Arc<str>>> {
    let mut files: FxHashMap<u64, Vec<Arc<str>>> = FxHashMap::default();

    for (url, file) in &self.registry.font_files {
      if let FontFile::Loaded { id, .. } = file {
        files.entry(*id).or_default().push(Arc::clone(url));
      }
    }

    files
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
            source.file_id(),
          )
        })
        .collect(),
    };
    let bytes = sources.iter().map(RuleSource::file_bytes).sum();
    let registry = registries
      .get_or_insert_with(key, || {
        let mut registry = (**base).clone();

        for source in sources {
          if let Some(font_face) = source.font_face(&registry.selector) {
            registry.selector.add_css_connected(font_face);
          }
        }

        registry.revise();

        Ok::<_, ()>(Weighed::new(Arc::new(registry), bytes))
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
  src: &'r FontFaceSrc,
  data: SourceData<'r>,
}

/// What a reached source draws with.
enum SourceData<'r> {
  /// The file the host loaded for a `url()`.
  File(&'r SharedBytes, u64),
  /// The system or registered face a `local()` names.
  Local(&'r Arc<FontCustomPlatformData>),
  /// A `url()` the host has not loaded yet.
  Pending(&'r Arc<str>),
}

impl FontRegistry {
  /// The face `local()` names, Blink's `LocalFontFaceSource` among takumi's stand-in for platform
  /// fonts: the system fonts, then the faces registered through [`crate::Fonts`].
  fn local_face(&self, name: &str) -> Option<&Arc<FontCustomPlatformData>> {
    self
      .font_faces()
      .filter(|font_face| !font_face.css_connected)
      .filter_map(|font_face| font_face.custom_platform_data())
      .find(|typeface| typeface.has_unique_name(name))
  }

  /// The source `rule` reaches, none when every source has failed or names no face.
  fn source_of<'r>(&'r self, rule: &'r FontFaceRule) -> Option<RuleSource<'r>> {
    rule.sources.iter().find_map(|src| {
      let data = match src {
        FontFaceSrc::Url(url) => match self.font_files.get(url) {
          Some(FontFile::Loaded { data, id }) => SourceData::File(data, *id),
          Some(FontFile::Failed) => return None,
          None => SourceData::Pending(url),
        },
        FontFaceSrc::Local(name) => SourceData::Local(self.local_face(name)?),
      };

      Some(RuleSource { rule, src, data })
    })
  }
}

impl RuleSource<'_> {
  /// The id of the loaded file the source draws with, none for a face it shares or awaits.
  fn file_id(&self) -> Option<u64> {
    match self.data {
      SourceData::File(_, id) => Some(id),
      SourceData::Local(_) | SourceData::Pending(_) => None,
    }
  }

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
    let platform_data = match self.data {
      SourceData::File(data, id) => selector.platform_data(data, id, 0)?,
      SourceData::Local(typeface) => Arc::clone(typeface),
      SourceData::Pending(url) => {
        return Some(FontFace::pending(
          family,
          descriptors,
          PendingSource::Url(Arc::clone(url)),
        ));
      }
    };

    Some(FontFace::new(family, descriptors, platform_data))
  }
}

/// Whether `data` is a font with the tables a face draws from. Approximate: Blink's OTS sanitizer
/// checks every table, where this only requires `head`, with a `unitsPerEm` in OTS's range,
/// `hhea`, `maxp`, and `cmap`.
fn is_usable_font(data: &SharedBytes) -> bool {
  FontRef::from_index((**data).as_ref(), 0).is_ok_and(|font| {
    font
      .head()
      .is_ok_and(|head| UNITS_PER_EM_RANGE.contains(&head.units_per_em()))
      && font.hhea().is_ok()
      && font.maxp().is_ok()
      && font.cmap().is_ok()
  })
}

#[cfg(test)]
mod tests {
  use std::{fs, path::Path, sync::Arc};

  use super::*;

  #[test]
  fn files_with_the_same_bytes_share_one_copy() {
    let bytes = fs::read(
      Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../assets/fonts/geist/geist-latin-wght-300-800.ttf"),
    )
    .unwrap();
    let mut fonts = Fonts::default();

    fonts
      .load_font_file("a.ttf", FontSource::from(bytes.clone()))
      .unwrap();
    fonts
      .load_font_file("b.ttf", FontSource::from(bytes))
      .unwrap();

    let data = |url: &str| match &fonts.registry.font_files[url] {
      FontFile::Loaded { data, .. } => Arc::clone(data),
      FontFile::Failed => panic!("{url} failed"),
    };

    assert!(Arc::ptr_eq(&data("a.ttf"), &data("b.ttf")));
  }
}
