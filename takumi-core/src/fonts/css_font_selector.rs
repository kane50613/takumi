//! Blink's [`CSSFontSelector`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/css/css_font_selector.cc):
//! the registry of faces a renderer draws text with, and the font data a family gives a request.
//! It holds only registered faces, Blink's document fonts. A family no registered face has
//! resolves among [`SystemFonts`](super::system_fonts::SystemFonts), as Blink's `GetFontData`
//! falls through to `FontCache`.
//!
//! Approximate: Blink's `FamilyNameFromSettings` maps a generic family to one platform family per
//! script. takumi lists registered families for a generic family instead, in registration order,
//! so a generic family resolves to registered families first and reaches system fonts only through
//! fallback. System font fallback becomes [`CssFontSelector::fallback_families`]: every registered
//! family in registration order, for every script, before the system fonts.
//! A clone is a render's snapshot, keeping every face it holds, and the bytes behind it, alive
//! after the registry lets go.

use std::{collections::HashMap, sync::Arc};

use super::{
  css_segmented_font_face::FontPlatformDataForRangeSet,
  font_custom_platform_data::FontCustomPlatformData,
  font_description::FontSelectionDescription,
  font_face::{FaceData, FontFace, FontFaceDescriptors, PendingSource},
  font_face_cache::{FontFaceCache, fold_family_name, insert_family},
};
use crate::resources::font::SharedBytes;

/// Blink's `CSSFontSelector`.
#[derive(Clone, Debug, Default)]
pub(crate) struct CssFontSelector {
  font_face_cache: FontFaceCache,
  /// Every registered face, in registration order.
  font_faces: Vec<Arc<FontFace>>,
  /// The families each generic keyword lists, by folded keyword.
  generic_font_family_settings: HashMap<String, Vec<Arc<str>>>,
  /// Registered families in registration order, which system fallback walks.
  fallback_families: Vec<Arc<str>>,
}

impl CssFontSelector {
  /// Registers each usable face of `data` under `descriptors`, and returns them.
  pub(crate) fn register(
    &mut self,
    data: &SharedBytes,
    id: u64,
    descriptors: &FontFaceDescriptors,
    generic_family: Option<&str>,
  ) -> Vec<Arc<FontFace>> {
    let mut faces = Vec::new();

    for font_face in FontFace::from_data(data, id, descriptors, |index| self.typeface(id, index)) {
      if let Some(font_face) = self.promote(&font_face) {
        faces.push(font_face);

        continue;
      }

      let font_face = Arc::new(font_face);

      self.font_face_cache.add_font_face(Arc::clone(&font_face));
      self.font_faces.push(Arc::clone(&font_face));
      insert_family(&mut self.fallback_families, &font_face.family);

      if let Some(generic_family) = generic_family {
        insert_family(
          self
            .generic_font_family_settings
            .entry(fold_family_name(generic_family))
            .or_default(),
          &font_face.family,
        );
      }

      faces.push(font_face);
    }

    faces
  }

  /// Registers a face of `descriptors`' family with no data yet, which matching finds loading
  /// from `source`.
  pub(crate) fn register_pending(
    &mut self,
    family: Arc<str>,
    descriptors: FontFaceDescriptors,
    source: PendingSource,
  ) {
    let font_face = Arc::new(FontFace::pending(Arc::clone(&family), descriptors, source));

    self.font_face_cache.add_font_face(Arc::clone(&font_face));
    self.font_faces.push(font_face);
    insert_family(&mut self.fallback_families, &family);
  }

  /// The face at `index` of `data`, shared with a registration of the same bytes, or none when
  /// it does not parse.
  #[cfg(feature = "font-face")]
  pub(crate) fn platform_data(
    &self,
    data: &SharedBytes,
    id: u64,
    index: u32,
  ) -> Option<Arc<FontCustomPlatformData>> {
    self
      .typeface(id, index)
      .or_else(|| FontCustomPlatformData::create(Arc::clone(data), id, index).map(Arc::new))
  }

  /// Adds the face an `@font-face` rule makes, after the CSS-connected faces before it in cascade
  /// order. Fallback does not walk its family, as Blink's system fallback skips web fonts.
  #[cfg(feature = "font-face")]
  pub(crate) fn add_css_connected(&mut self, font_face: FontFace) {
    let font_face = Arc::new(FontFace {
      css_connected: true,
      ..font_face
    });

    self.font_face_cache.add_font_face(Arc::clone(&font_face));
    self.font_faces.push(font_face);
  }

  /// Whether a pending face is registered.
  pub(crate) fn has_pending_faces(&self) -> bool {
    self
      .font_faces
      .iter()
      .any(|font_face| font_face.custom_platform_data().is_none())
  }

  /// Removes every face of `family`, so it falls back as an unknown family; false when it has none.
  pub(crate) fn unregister_family(&mut self, family: &str) -> bool {
    let key = fold_family_name(family);

    self.remove_font_faces(|font_face| fold_family_name(&font_face.family) == key)
  }

  /// Removes the faces `is_removed` picks, and from each family list the families left without a
  /// face; false when it picks none.
  fn remove_font_faces(&mut self, is_removed: impl Fn(&Arc<FontFace>) -> bool) -> bool {
    let (removed, kept): (Vec<_>, Vec<_>) = self.font_faces.drain(..).partition(is_removed);

    self.font_faces = kept;

    if removed.is_empty() {
      return false;
    }

    for font_face in &removed {
      self.font_face_cache.remove_font_face(font_face);
    }

    let font_faces = &self.font_faces;
    let has_faces = |family: &Arc<str>| {
      let key = fold_family_name(family);

      font_faces
        .iter()
        .any(|font_face| fold_family_name(&font_face.family) == key)
    };

    self.fallback_families.retain(has_faces);
    self.generic_font_family_settings.retain(|_, families| {
      families.retain(has_faces);
      !families.is_empty()
    });

    true
  }

  /// Every registered face, in registration order.
  pub(crate) fn font_faces(&self) -> impl Iterator<Item = &Arc<FontFace>> + '_ {
    self.font_faces.iter()
  }

  /// Blink's `FamilyNameFromSettings`: the families the generic keyword `generic_family` lists.
  pub(crate) fn family_names_from_settings(&self, generic_family: &str) -> &[Arc<str>] {
    self
      .generic_font_family_settings
      .get(&fold_family_name(generic_family))
      .map_or(&[], Vec::as_slice)
  }

  /// The decoded face at `index` of the data `id` names, under any family, so registering the same
  /// bytes again shares one face and its shaping tables.
  fn typeface(&self, id: u64, index: u32) -> Option<Arc<FontCustomPlatformData>> {
    self
      .font_faces
      .iter()
      .filter_map(|font_face| font_face.custom_platform_data())
      .find(|typeface| typeface.id == id && typeface.index == index)
      .map(Arc::clone)
  }

  /// Moves the face registered with `font_face`'s family, data, and descriptors after every other
  /// face, as registering it anew would place it.
  fn promote(&mut self, font_face: &FontFace) -> Option<Arc<FontFace>> {
    let typeface = font_face.custom_platform_data()?;
    let family = fold_family_name(&font_face.family);
    let position = self.font_faces.iter().position(|registered| {
      registered.custom_platform_data().is_some_and(|registered| {
        registered.id == typeface.id && registered.index == typeface.index
      }) && fold_family_name(&registered.family) == family
        && registered.descriptors == font_face.descriptors
    })?;
    let registered = self.font_faces.remove(position);

    self.font_face_cache.remove_font_face(&registered);
    self.font_face_cache.add_font_face(Arc::clone(&registered));
    self.font_faces.push(Arc::clone(&registered));

    Some(registered)
  }

  /// The families system fallback walks before the system fonts, in registration order.
  pub(crate) fn fallback_families(&self) -> impl Iterator<Item = &Arc<str>> + '_ {
    self.fallback_families.iter()
  }

  /// Blink's `GetFontData` over document fonts: the registered faces of `family` matching
  /// `font_description`, or of the first family a generic keyword lists that has faces.
  pub(crate) fn get_font_data(
    &self,
    font_description: &FontSelectionDescription,
    family: &str,
    family_is_generic: bool,
  ) -> Option<Vec<FaceData<FontPlatformDataForRangeSet>>> {
    if !family_is_generic {
      return self.font_face_cache.get_font_data(font_description, family);
    }

    self
      .family_names_from_settings(family)
      .iter()
      .find_map(|settings_family_name| {
        self
          .font_face_cache
          .get_font_data(font_description, settings_family_name)
      })
  }
}

#[cfg(test)]
mod tests {
  use harfrust::{Tag, font::Variation};

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

    selector.register(&data, id, descriptors, None)
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

    let FaceData::Loaded(face) = faces.remove(0) else {
      panic!("{family} matched a pending face");
    };

    face
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
    assert!(faces[0].range_set().contains(u32::from('a')));
    assert!(faces[1].range_set().contains(u32::from('A')));
  }

  #[test]
  fn generic_families_list_registered_families() {
    let mut selector = CssFontSelector::default();
    let (data, id) = FontCustomPlatformData::read_asset(GEIST_MONO);

    selector.register(&data, id, &named("Code"), Some("monospace"));

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

      selector.register(&data, id, &named("Cycle"), None);

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
