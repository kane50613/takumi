//! Blink's [`CSSFontSelector`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/css/css_font_selector.cc):
//! the registry of faces a renderer draws text with, and the font data a family gives a request.
//!
//! Approximate: Blink resolves a generic family to one platform family per script through
//! `GenericFontFamilySettings` and looks it up among system fonts. takumi has no system fonts,
//! so a generic family lists registered families, in registration order, looked up among the
//! registered faces. System font fallback becomes [`CssFontSelector::fallback_families`]: every
//! registered family in registration order, last-resort families last, for every script.
//! A clone is a render's snapshot, keeping every face it holds, and the bytes behind it, alive
//! after the registry lets go.

use std::{collections::HashMap, sync::Arc};

use skrifa::raw::FileRef;

use super::{
  css_segmented_font_face::FontPlatformDataForRangeSet,
  font_custom_platform_data::FontCustomPlatformData,
  font_description::FontSelectionDescription,
  font_face::{FontFace, FontFaceDescriptors},
  font_face_cache::{FontFaceCache, fold_family_name},
};
use crate::resources::font::SharedBytes;

/// Blink's `CSSFontSelector`.
#[derive(Clone, Debug, Default)]
pub(crate) struct CssFontSelector {
  font_face_cache: FontFaceCache,
  /// Every registered face, in registration order.
  font_faces: Vec<RegisteredFontFace>,
  /// The families each generic keyword lists, by folded keyword.
  generic_font_family_settings: HashMap<String, Vec<Arc<str>>>,
  /// Registered families in registration order, which system fallback walks.
  fallback_families: Vec<Arc<str>>,
  /// Families registered as a last resort, which fallback walks after every other.
  last_resort_families: Vec<Arc<str>>,
}

/// A registered face, and whether it registered as a last resort.
#[derive(Clone, Debug)]
struct RegisteredFontFace {
  font_face: Arc<FontFace>,
  last_resort: bool,
}

/// Adds `family` to `families` unless it is there under any case.
fn insert_family(families: &mut Vec<Arc<str>>, family: &Arc<str>) {
  let key = fold_family_name(family);

  if !families
    .iter()
    .any(|existing| fold_family_name(existing) == key)
  {
    families.push(Arc::clone(family));
  }
}

impl CssFontSelector {
  /// Registers every face of the sfnt or collection `data` under `descriptors`, its own family
  /// name when they give none; its faces, the ones registered before included, skipping any
  /// without a family name.
  pub(crate) fn register(
    &mut self,
    data: SharedBytes,
    id: u64,
    descriptors: &FontFaceDescriptors,
    generic_family: Option<&str>,
    last_resort: bool,
  ) -> Vec<Arc<FontFace>> {
    let count = FileRef::new((*data).as_ref()).map_or(0, |file| file.fonts().count());
    let mut faces = Vec::new();

    for index in 0..count {
      let Some(platform_data) = u32::try_from(index)
        .ok()
        .and_then(|index| FontCustomPlatformData::create(Arc::clone(&data), id, index))
      else {
        continue;
      };
      let Some(family) = descriptors
        .family
        .clone()
        .or_else(|| platform_data.family_name_for_inspector().map(Arc::from))
      else {
        continue;
      };

      if family.is_empty() {
        continue;
      }

      if let Some(font_face) = self.font_face(&family, id, platform_data.index) {
        faces.push(Arc::clone(font_face));

        continue;
      }

      let font_face = Arc::new(FontFace::new(
        Arc::clone(&family),
        descriptors.clone(),
        Arc::new(platform_data),
      ));

      self.font_face_cache.add_font_face(Arc::clone(&font_face));
      self.font_faces.push(RegisteredFontFace {
        font_face: Arc::clone(&font_face),
        last_resort,
      });

      let order = if last_resort {
        &mut self.last_resort_families
      } else {
        &mut self.fallback_families
      };

      insert_family(order, &family);

      if let Some(generic_family) = generic_family {
        insert_family(
          self
            .generic_font_family_settings
            .entry(fold_family_name(generic_family))
            .or_default(),
          &family,
        );
      }

      faces.push(font_face);
    }

    faces
  }

  /// Removes every face of `family`, so it falls back as an unknown family; false when it has none.
  pub(crate) fn unregister_family(&mut self, family: &str) -> bool {
    let key = fold_family_name(family);

    self.remove_font_faces(|font_face| fold_family_name(&font_face.family) == key)
  }

  /// Removes the faces `is_removed` picks, and from each family list the families left without a
  /// face it lists them for; false when it picks none.
  fn remove_font_faces(&mut self, is_removed: impl Fn(&Arc<FontFace>) -> bool) -> bool {
    let (removed, kept): (Vec<_>, Vec<_>) = self
      .font_faces
      .drain(..)
      .partition(|registered| is_removed(&registered.font_face));

    self.font_faces = kept;

    if removed.is_empty() {
      return false;
    }

    for registered in &removed {
      self.font_face_cache.remove_font_face(&registered.font_face);
    }

    let font_faces = &self.font_faces;
    let has_faces = |family: &Arc<str>, last_resort: Option<bool>| {
      let key = fold_family_name(family);

      font_faces.iter().any(|registered| {
        last_resort.is_none_or(|last_resort| registered.last_resort == last_resort)
          && fold_family_name(&registered.font_face.family) == key
      })
    };

    self
      .fallback_families
      .retain(|family| has_faces(family, Some(false)));
    self
      .last_resort_families
      .retain(|family| has_faces(family, Some(true)));
    self.generic_font_family_settings.retain(|_, families| {
      families.retain(|family| has_faces(family, None));
      !families.is_empty()
    });

    true
  }

  /// Every registered face, in registration order.
  pub(crate) fn font_faces(&self) -> impl Iterator<Item = &Arc<FontFace>> + '_ {
    self
      .font_faces
      .iter()
      .map(|registered| &registered.font_face)
  }

  /// Blink's `FamilyNameFromSettings`: the families the generic keyword `generic_family` lists.
  pub(crate) fn family_names_from_settings(&self, generic_family: &str) -> &[Arc<str>] {
    self
      .generic_font_family_settings
      .get(&fold_family_name(generic_family))
      .map_or(&[], Vec::as_slice)
  }

  /// The face `family` already lists at `index` of the data `id` names.
  fn font_face(&self, family: &str, id: u64, index: u32) -> Option<&Arc<FontFace>> {
    let family = fold_family_name(family);

    self
      .font_faces
      .iter()
      .map(|registered| &registered.font_face)
      .find(|font_face| {
        let custom_platform_data = &font_face.custom_platform_data;

        custom_platform_data.id == id
          && custom_platform_data.index == index
          && fold_family_name(&font_face.family) == family
      })
  }

  /// The families registered as a last resort, which fallback walks after every other.
  pub(crate) fn last_resort_families(&self) -> impl Iterator<Item = &Arc<str>> + '_ {
    self.last_resort_families.iter()
  }

  /// The families system fallback walks, in order.
  pub(crate) fn fallback_families(&self) -> impl Iterator<Item = &Arc<str>> + '_ {
    self
      .fallback_families
      .iter()
      .chain(&self.last_resort_families)
  }

  /// Blink's `GetFontData`: the faces of `family` matching `font_description`, or of the first
  /// family a generic keyword lists that has faces.
  pub(crate) fn get_font_data(
    &self,
    font_description: &FontSelectionDescription,
    family: &str,
    family_is_generic: bool,
  ) -> Option<Vec<FontPlatformDataForRangeSet>> {
    if !family_is_generic {
      return self.font_data_for_family(font_description, family);
    }

    self
      .family_names_from_settings(family)
      .iter()
      .find_map(|settings_family_name| {
        self.font_data_for_family(font_description, settings_family_name)
      })
  }

  /// Approximate: Blink takes the faces of the best bucket alone. takumi gives an `auto`
  /// descriptor the face's own capabilities, which splits the `unicode-range` subsets of one
  /// family into buckets, so a best bucket whose faces leave out codepoints is followed by the
  /// faces of the others, the better matches first.
  fn font_data_for_family(
    &self,
    font_description: &FontSelectionDescription,
    family: &str,
  ) -> Option<Vec<FontPlatformDataForRangeSet>> {
    let ranked = self
      .font_face_cache
      .get_ranked(font_description.font_selection_request, family);
    let (best, others) = ranked.split_first()?;
    let mut faces = best.get_font_data(font_description)?;

    if faces.iter().any(|face| !face.range_set.is_entire_range()) {
      faces.extend(
        others
          .iter()
          .filter_map(|segmented| segmented.get_font_data(font_description))
          .flatten(),
      );
    }

    Some(faces)
  }
}

#[cfg(test)]
mod tests {
  use harfrust::{Tag, Variation};

  use super::*;
  use crate::{
    fonts::font_selection_types::{
      FontSelectionRequest, FontSelectionValue, ITALIC_SLOPE_VALUE, NORMAL_SLOPE_VALUE,
      NORMAL_WIDTH_VALUE,
    },
    resources::unicode_range::{UnicodeRange, UnicodeRangeSet},
  };

  const GEIST: &str = "../assets/fonts/geist/Geist[wght].woff2";
  const GEIST_MONO: &str = "../assets/fonts/geist/GeistMono[wght].woff2";
  const GEIST_DUO: &str = "../assets/fonts/geist/GeistDuo.ttc";
  const POPPINS_REGULAR: &str =
    "../assets/fonts/poppins/poppins-v24-devanagari_latin-regular.woff2";
  const POPPINS_BOLD: &str = "../assets/fonts/poppins/poppins-v24-devanagari_latin-700.woff2";
  const SCHEHERAZADE: [&str; 4] = [
    "../assets/fonts/sil/scheherazade-new-v17-arabic-regular.woff2",
    "../assets/fonts/sil/scheherazade-new-v17-arabic-500.woff2",
    "../assets/fonts/sil/scheherazade-new-v17-arabic-600.woff2",
    "../assets/fonts/sil/scheherazade-new-v17-arabic-700.woff2",
  ];

  fn register(
    selector: &mut CssFontSelector,
    path: &str,
    descriptors: &FontFaceDescriptors,
  ) -> Vec<Arc<FontFace>> {
    let (data, id) = FontCustomPlatformData::read_asset(path);

    selector.register(data, id, descriptors, None, false)
  }

  fn named(family: &str) -> FontFaceDescriptors {
    FontFaceDescriptors {
      family: Some(family.into()),
      ..Default::default()
    }
  }

  fn request(weight: f32, slope: FontSelectionValue) -> FontSelectionDescription {
    FontSelectionDescription::for_request(FontSelectionRequest::new(
      FontSelectionValue::new(weight),
      NORMAL_WIDTH_VALUE,
      slope,
    ))
  }

  /// The single face `family` gives `description`.
  fn face(
    selector: &CssFontSelector,
    description: &FontSelectionDescription,
    family: &str,
  ) -> FontPlatformDataForRangeSet {
    let mut faces = selector.get_font_data(description, family, false).unwrap();

    assert_eq!(faces.len(), 1);
    faces.remove(0)
  }

  /// The weight class of the face `family` picks for `weight`.
  fn matched_weight(selector: &CssFontSelector, family: &str, weight: f32) -> f32 {
    face(selector, &request(weight, NORMAL_SLOPE_VALUE), family)
      .font_platform_data
      .typeface
      .attributes
      .weight
      .value()
  }

  #[test]
  fn a_variable_face_instances_the_requested_weight() {
    let mut selector = CssFontSelector::default();

    register(&mut selector, GEIST, &FontFaceDescriptors::default());

    for weight in [100.0, 350.0, 700.0, 900.0] {
      let face = face(&selector, &request(weight, NORMAL_SLOPE_VALUE), "geist");

      assert_eq!(face.font_platform_data.variation(b"wght"), Some(weight));
      assert!(!face.font_platform_data.synthetic_bold);
    }
  }

  #[test]
  fn static_faces_follow_the_nearest_weight_rules() {
    let mut selector = CssFontSelector::default();

    for path in SCHEHERAZADE {
      register(&mut selector, path, &named("Scheherazade"));
    }

    // 400 to 500 looks up to 500, then down, then above 500.
    assert_eq!(matched_weight(&selector, "Scheherazade", 400.0), 400.0);
    assert_eq!(matched_weight(&selector, "Scheherazade", 450.0), 500.0);
    assert_eq!(matched_weight(&selector, "Scheherazade", 500.0), 500.0);
    // Above 500 looks up first; below 400 down first, then up.
    assert_eq!(matched_weight(&selector, "Scheherazade", 550.0), 600.0);
    assert_eq!(matched_weight(&selector, "Scheherazade", 800.0), 700.0);
    assert_eq!(matched_weight(&selector, "Scheherazade", 300.0), 400.0);

    let mut poppins = CssFontSelector::default();

    register(&mut poppins, POPPINS_REGULAR, &named("Poppins"));
    register(&mut poppins, POPPINS_BOLD, &named("Poppins"));

    assert_eq!(matched_weight(&poppins, "Poppins", 500.0), 400.0);
    assert_eq!(matched_weight(&poppins, "Poppins", 600.0), 700.0);
    assert_eq!(matched_weight(&poppins, "Poppins", 350.0), 400.0);
  }

  #[test]
  fn a_missing_bold_or_italic_is_synthesized() {
    let mut selector = CssFontSelector::default();

    register(&mut selector, POPPINS_REGULAR, &named("Poppins"));

    let bold = face(&selector, &request(700.0, NORMAL_SLOPE_VALUE), "Poppins");

    assert!(bold.font_platform_data.synthetic_bold);
    assert!(!bold.font_platform_data.synthetic_italic);

    let italic = face(&selector, &request(400.0, ITALIC_SLOPE_VALUE), "Poppins");

    assert!(italic.font_platform_data.synthetic_italic);

    // Blink only slants for oblique angles at or past the italic threshold.
    let shallow = face(
      &selector,
      &request(400.0, FontSelectionValue::new(10.0)),
      "Poppins",
    );

    assert!(!shallow.font_platform_data.synthetic_italic);

    let mut forbidden = request(700.0, ITALIC_SLOPE_VALUE);

    forbidden.synthetic_bold_allowed = false;
    forbidden.synthetic_italic_allowed = false;

    let forbidden = face(&selector, &forbidden, "Poppins");

    assert!(!forbidden.font_platform_data.synthetic_bold);
    assert!(!forbidden.font_platform_data.synthetic_italic);
  }

  #[test]
  fn an_italic_request_takes_an_italic_face_over_synthesis() {
    let mut selector = CssFontSelector::default();
    let italic = FontFaceDescriptors {
      family: Some("Mixed".into()),
      slope: Some((14.0, 14.0)),
      ..Default::default()
    };

    register(&mut selector, GEIST, &named("Mixed"));
    register(&mut selector, GEIST_MONO, &italic);

    let face = face(&selector, &request(400.0, ITALIC_SLOPE_VALUE), "Mixed");

    assert!(!face.font_platform_data.synthetic_italic);
    // The slope descriptor is explicit, so `slnt` takes the request clamped to it.
    assert_eq!(face.font_platform_data.variation(b"slnt"), Some(-14.0));
    assert_eq!(
      face
        .font_platform_data
        .typeface
        .family_name_for_inspector()
        .as_deref(),
      Some("Geist Mono")
    );
  }

  #[test]
  fn a_collection_registers_each_family_in_face_order() {
    let mut selector = CssFontSelector::default();
    let faces = register(&mut selector, GEIST_DUO, &FontFaceDescriptors::default());
    let families: Vec<&str> = faces.iter().map(|face| &*face.family).collect();

    assert_eq!(families, ["Geist", "Geist Mono"]);
    assert_eq!(
      selector
        .fallback_families()
        .map(|family| &**family)
        .collect::<Vec<_>>(),
      ["Geist", "Geist Mono"]
    );
  }

  #[test]
  fn ranged_faces_segment_one_family_the_last_added_first() {
    let mut selector = CssFontSelector::default();

    for (path, range) in [
      (GEIST, UnicodeRange::new(0x41, 0x5A)),
      (GEIST_MONO, UnicodeRange::new(0x61, 0x7A)),
    ] {
      register(
        &mut selector,
        path,
        &FontFaceDescriptors {
          family: Some("Logical".into()),
          unicode_range: UnicodeRangeSet::from_iter([range]),
          ..Default::default()
        },
      );
    }

    let faces = selector
      .get_font_data(&request(400.0, NORMAL_SLOPE_VALUE), "Logical", false)
      .unwrap();

    assert_eq!(faces.len(), 2);
    assert!(faces[0].range_set.contains(u32::from('a')));
    assert!(faces[1].range_set.contains(u32::from('A')));
  }

  #[test]
  fn generic_families_list_registered_families() {
    let mut selector = CssFontSelector::default();
    let (data, id) = FontCustomPlatformData::read_asset(GEIST_MONO);

    selector.register(data, id, &named("Code"), Some("monospace"), false);

    let description = request(400.0, NORMAL_SLOPE_VALUE);

    assert!(
      selector
        .get_font_data(&description, "monospace", true)
        .is_some()
    );
    assert!(
      selector
        .get_font_data(&description, "monospace", false)
        .is_none()
    );
    assert_eq!(selector.family_names_from_settings("Monospace").len(), 1);
  }

  #[test]
  fn last_resort_families_fall_back_last() {
    let mut selector = CssFontSelector::default();
    let (data, id) = FontCustomPlatformData::read_asset(GEIST);

    selector.register(data, id, &named("Embedded"), None, true);
    register(&mut selector, GEIST_MONO, &named("User Font"));

    assert_eq!(
      selector
        .fallback_families()
        .map(|family| &**family)
        .collect::<Vec<_>>(),
      ["User Font", "Embedded"]
    );
  }

  #[test]
  fn an_unregistered_family_no_longer_matches() {
    let mut selector = CssFontSelector::default();

    register(&mut selector, GEIST, &named("Gone"));
    register(&mut selector, GEIST_MONO, &named("Kept"));

    assert!(selector.unregister_family("gone"));
    assert!(!selector.unregister_family("gone"));
    assert!(
      selector
        .get_font_data(&request(400.0, NORMAL_SLOPE_VALUE), "Gone", false)
        .is_none()
    );
    assert_eq!(selector.font_faces().count(), 1);
    assert_eq!(selector.fallback_families().count(), 1);
  }

  #[test]
  fn descriptor_variation_settings_reach_the_instance() {
    let mut selector = CssFontSelector::default();

    register(
      &mut selector,
      GEIST,
      &FontFaceDescriptors {
        family: Some("Light".into()),
        variation_settings: vec![Variation {
          tag: Tag::new(b"wght"),
          value: 300.0,
        }],
        ..Default::default()
      },
    );

    let face = face(&selector, &request(700.0, NORMAL_SLOPE_VALUE), "Light");

    assert_eq!(face.font_platform_data.variation(b"wght"), Some(300.0));
  }

  #[test]
  fn register_and_unregister_cycles_release_every_byte() {
    let mut selector = CssFontSelector::default();

    for _ in 0..3 {
      let (data, id) = FontCustomPlatformData::read_asset(GEIST);

      selector.register(Arc::clone(&data), id, &named("Cycle"), None, false);

      assert!(Arc::strong_count(&data) > 1);
      assert!(selector.unregister_family("cycle"));
      assert_eq!(
        Arc::strong_count(&data),
        1,
        "the registry let go of the bytes"
      );
      assert_eq!(selector.font_faces().count(), 0);
      assert_eq!(selector.fallback_families().count(), 0);
    }
  }
}
