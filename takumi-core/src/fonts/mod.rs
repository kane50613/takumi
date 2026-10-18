//! Blink's `platform/fonts`.

#[cfg_attr(
  not(test),
  expect(
    dead_code,
    reason = "TODO: font matching moves off fontique once the registry is rewired"
  )
)]
#[cfg_attr(test, allow(dead_code))]
pub(crate) mod css_font_selector;
#[cfg_attr(
  not(test),
  expect(
    dead_code,
    reason = "TODO: font matching moves off fontique once the registry is rewired"
  )
)]
#[cfg_attr(test, allow(dead_code))]
pub(crate) mod css_segmented_font_face;
pub(crate) mod emoji_presentation_scanner;
pub(crate) mod font;
pub(crate) mod font_cache;
#[cfg_attr(
  not(test),
  expect(
    dead_code,
    reason = "TODO: font matching moves off fontique once the registry is rewired"
  )
)]
#[cfg_attr(test, allow(dead_code))]
pub(crate) mod font_custom_platform_data;
pub(crate) mod font_data_for_range_set;
pub(crate) mod font_description;
#[cfg_attr(
  not(test),
  expect(
    dead_code,
    reason = "TODO: font matching moves off fontique once the registry is rewired"
  )
)]
#[cfg_attr(test, allow(dead_code))]
pub(crate) mod font_face;
#[cfg_attr(
  not(test),
  expect(
    dead_code,
    reason = "TODO: font matching moves off fontique once the registry is rewired"
  )
)]
#[cfg_attr(test, allow(dead_code))]
pub(crate) mod font_face_cache;
pub(crate) mod font_fallback_iterator;
pub(crate) mod font_fallback_list;
pub(crate) mod font_fallback_priority;
pub(crate) mod font_height;
#[cfg_attr(
  not(test),
  expect(
    dead_code,
    reason = "TODO: font matching moves off fontique once the registry is rewired"
  )
)]
#[cfg_attr(test, allow(dead_code))]
pub(crate) mod font_selection_algorithm;
#[cfg_attr(
  not(test),
  expect(
    dead_code,
    reason = "TODO: font matching moves off fontique once the registry is rewired"
  )
)]
#[cfg_attr(test, allow(dead_code))]
pub(crate) mod font_selection_types;
pub(crate) mod open_type_caps_support;
pub(crate) mod script_run_iterator;
pub(crate) mod shaping;
pub(crate) mod simple_font_data;
pub(crate) mod small_caps_iterator;
pub(crate) mod symbols_iterator;
