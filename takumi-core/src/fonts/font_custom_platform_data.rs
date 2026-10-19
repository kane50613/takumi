//! Blink's [`FontCustomPlatformData`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/platform/fonts/font_custom_platform_data.cc):
//! a registered face, and the variable instance and synthesis it is drawn with for a request.
//!
//! `auto_font_selection_capabilities` is takumi's: Blink treats an `auto` descriptor as
//! `normal`, while CSS Fonts 4 takes the face's own value, which takumi reads from its tables
//! as Skia's `SkFontStyle` and `fvar` axes give them. Palettes are not ported.

use std::{
  fmt::{self, Debug, Formatter},
  sync::{Arc, OnceLock},
};

use harfrust::{
  Tag,
  font::{Blob, Font as ShapingFont, Variation},
};
use skrifa::{
  FontRef, MetadataProvider,
  attribute::{Attributes, Style},
  raw::TableProvider,
  string::StringId,
};

use super::{
  font_description::FontSelectionDescription,
  font_selection_types::{
    BOLD_THRESHOLD, FontSelectionCapabilities, FontSelectionRange, FontSelectionValue,
    ITALIC_SLOPE_VALUE, NORMAL_SLOPE_VALUE, NORMAL_WEIGHT_VALUE,
  },
};
use crate::{
  resources::font::{FontBlob, SharedBytes, has_color_table},
  style::FontOpticalSizing,
};

const ITAL_TAG: Tag = Tag::new(b"ital");
const OPSZ_TAG: Tag = Tag::new(b"opsz");
const SLNT_TAG: Tag = Tag::new(b"slnt");
const WDTH_TAG: Tag = Tag::new(b"wdth");
const WGHT_TAG: Tag = Tag::new(b"wght");
const CFF2_TAG: Tag = Tag::new(b"CFF2");

/// Skia's `SkFontStyle::kSemiBold_Weight`, from which `SkTypeface::isBold` holds.
const SK_SEMI_BOLD_WEIGHT: f32 = 600.0;

/// Blink's `FontFormatCheck::VariableFontSubType`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum VariableFontSubType {
  NotVariable,
  VariableTrueType,
  VariableCff2,
}

impl VariableFontSubType {
  /// Blink's `FontFormatCheck::ProbeVariableFont`.
  fn probe_variable_font(font: &FontRef<'_>) -> Self {
    if font.fvar().is_err() {
      return Self::NotVariable;
    }

    if font.table_data(CFF2_TAG).is_some() {
      return Self::VariableCff2;
    }

    Self::VariableTrueType
  }
}

/// Skia's `SkFontParameters::Variation::Axis`: one `fvar` axis.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct VariationAxis {
  pub(crate) tag: Tag,
  pub(crate) minimum: f32,
  pub(crate) default: f32,
  pub(crate) maximum: f32,
}

impl VariationAxis {
  /// The axis span as Blink reads it into a `FontSelectionRange`.
  fn range(&self) -> FontSelectionRange {
    FontSelectionRange::new(
      FontSelectionValue::new(self.minimum),
      FontSelectionValue::new(self.maximum),
    )
  }

  fn includes(&self, value: f32) -> bool {
    self.minimum <= value && self.maximum >= value
  }
}

/// Blink's `FontCustomPlatformData`, holding a decoded face in place of an `SkTypeface`.
pub(crate) struct FontCustomPlatformData {
  pub(crate) data: SharedBytes,
  /// The id glyph caches key the bytes by.
  pub(crate) id: u64,
  /// The face's index in its collection.
  pub(crate) index: u32,
  pub(crate) variable_font_sub_type: VariableFontSubType,
  pub(crate) axes: Vec<VariationAxis>,
  /// The default instance's weight, width, and slope, as Skia's `SkTypeface::fontStyle`.
  pub(crate) attributes: Attributes,
  /// Whether the face has a color glyph table Blink's `ColorTableLookup` accepts.
  pub(crate) is_color: bool,
  /// The default instance harfrust shapes with, built once, so every size, variation, and render
  /// of the face shares its layout tables.
  shaping_font: OnceLock<Option<ShapingFont>>,
}

/// Blink's `FontPlatformData`, as far as a registered face's instance goes.
#[derive(Clone, Debug)]
pub(crate) struct FontPlatformData {
  pub(crate) typeface: Arc<FontCustomPlatformData>,
  pub(crate) synthetic_bold: bool,
  pub(crate) synthetic_italic: bool,
  /// The design coordinates the face is instanced at; a later tag wins, as in Skia.
  pub(crate) variations: Vec<Variation>,
}

impl FontCustomPlatformData {
  /// Blink's `Create`: the face at `index` of `data`, or none when it does not parse.
  pub(crate) fn create(data: SharedBytes, id: u64, index: u32) -> Option<Self> {
    let font = FontRef::from_index((*data).as_ref(), index).ok()?;
    let variable_font_sub_type = VariableFontSubType::probe_variable_font(&font);
    let axes = font
      .axes()
      .iter()
      .map(|axis| VariationAxis {
        tag: Tag::new(&axis.tag().to_be_bytes()),
        minimum: axis.min_value(),
        default: axis.default_value(),
        maximum: axis.max_value(),
      })
      .collect();
    let attributes = font.attributes();
    let is_color = has_color_table(&font);

    Some(Self {
      data,
      id,
      index,
      variable_font_sub_type,
      axes,
      attributes,
      is_color,
      shaping_font: OnceLock::new(),
    })
  }

  /// The face's default instance as harfrust shapes with it; none when it does not parse.
  pub(crate) fn shaping_font(&self) -> Option<&ShapingFont> {
    self
      .shaping_font
      .get_or_init(|| ShapingFont::new(Blob::from(Arc::clone(&self.data)), self.index))
      .as_ref()
  }

  /// The sfnt bytes and the id glyph caches key them by.
  pub(crate) fn blob(&self) -> FontBlob {
    FontBlob::new(Arc::clone(&self.data), self.id)
  }

  pub(crate) fn font_ref(&self) -> Option<FontRef<'_>> {
    FontRef::from_index((*self.data).as_ref(), self.index).ok()
  }

  /// Blink's `FamilyNameForInspector`: the typographic family name, else the family name, each in
  /// English when the face has it.
  pub(crate) fn family_name_for_inspector(&self) -> Option<String> {
    let font = self.font_ref()?;

    [StringId::TYPOGRAPHIC_FAMILY_NAME, StringId::FAMILY_NAME]
      .into_iter()
      .find_map(|id| font.localized_strings(id).english_or_first())
      .map(|name| name.chars().collect())
  }

  /// Skia's `SkTypeface::isBold`.
  pub(crate) fn is_bold(&self) -> bool {
    self.attributes.weight.value() >= SK_SEMI_BOLD_WEIGHT
  }

  /// Skia's `SkTypeface::isItalic`.
  pub(crate) fn is_italic(&self) -> bool {
    self.attributes.style != Style::Normal
  }

  /// Blink's `RetrieveVariationDesignParametersByTag`.
  fn retrieve_variation_design_parameters_by_tag(&self, tag: Tag) -> Option<VariationAxis> {
    self.axes.iter().find(|axis| axis.tag == tag).copied()
  }

  /// The range an `fvar` axis spans, when the face has a valid one.
  fn axis_range(&self, tag: Tag) -> Option<FontSelectionRange> {
    self
      .retrieve_variation_design_parameters_by_tag(tag)
      .as_ref()
      .map(VariationAxis::range)
      .filter(FontSelectionRange::is_valid)
  }

  /// The capabilities CSS Fonts 4 gives an `auto` descriptor: the `wght`, `wdth`, and `slnt`
  /// axis ranges, else the default instance's own values, with an `ital` axis reaching italic.
  /// <https://drafts.csswg.org/css-fonts-4/#font-prop-desc>
  pub(crate) fn auto_font_selection_capabilities(&self) -> FontSelectionCapabilities {
    let weight = self.axis_range(WGHT_TAG).unwrap_or_else(|| {
      FontSelectionRange::single(FontSelectionValue::new(self.attributes.weight.value()))
    });
    let width = self.axis_range(WDTH_TAG).unwrap_or_else(|| {
      FontSelectionRange::single(FontSelectionValue::new(
        self.attributes.stretch.percentage(),
      ))
    });
    let default_slope = match self.attributes.style {
      Style::Normal => NORMAL_SLOPE_VALUE,
      Style::Italic => ITALIC_SLOPE_VALUE,
      // `post` angles turn counter-clockwise, CSS oblique angles clockwise.
      Style::Oblique(angle) => angle
        .filter(|angle| *angle != 0.0)
        .map_or(ITALIC_SLOPE_VALUE, |angle| FontSelectionValue::new(-angle)),
    };
    let mut slope = FontSelectionRange::single(default_slope);

    if let Some(slnt) = self.axis_range(SLNT_TAG) {
      slope.expand(&FontSelectionRange::new(-slnt.maximum, -slnt.minimum));
    }

    if self
      .retrieve_variation_design_parameters_by_tag(ITAL_TAG)
      .is_some_and(|ital| ital.includes(1.0))
    {
      slope.expand(&FontSelectionRange::single(ITALIC_SLOPE_VALUE));
    }

    FontSelectionCapabilities::new(width, slope, weight)
  }

  /// Blink's `GetFontPlatformData`, taking the arguments `BinaryDataFontFaceSource::CreateFontData`
  /// reads from `font_description`.
  pub(crate) fn get_font_platform_data(
    self: &Arc<Self>,
    font_description: &FontSelectionDescription,
    selection_capabilities: &FontSelectionCapabilities,
  ) -> FontPlatformData {
    let bold = font_description.is_synthetic_bold && font_description.synthetic_bold_allowed;
    let synthesize_italic =
      font_description.is_synthetic_italic && font_description.synthetic_italic_allowed;
    let selection_request = font_description.font_selection_request;
    let mut synthetic_bold = bold;
    let mut synthetic_italic = synthesize_italic;
    let mut variation = Vec::new();

    if self.variable_font_sub_type != VariableFontSubType::NotVariable {
      let mut weight_coordinate = Variation {
        tag: WGHT_TAG,
        value: selection_capabilities
          .weight
          .clamp_to_range(selection_request.weight)
          .to_f32(),
      };
      let wght_parameters = self.retrieve_variation_design_parameters_by_tag(WGHT_TAG);

      if selection_capabilities.weight.is_range_set_from_auto()
        && let Some(wght_parameters) = wght_parameters
      {
        let wght_range = wght_parameters.range();

        if wght_range.is_valid() {
          weight_coordinate.value = wght_range.clamp_to_range(selection_request.weight).to_f32();

          let has_bold_variations = wght_range.maximum > NORMAL_WEIGHT_VALUE;

          synthetic_bold =
            bold && !has_bold_variations && selection_request.weight >= BOLD_THRESHOLD;
        }
      }

      let mut width_coordinate = Variation {
        tag: WDTH_TAG,
        value: selection_capabilities
          .width
          .clamp_to_range(selection_request.width)
          .to_f32(),
      };
      let wdth_parameters = self.retrieve_variation_design_parameters_by_tag(WDTH_TAG);

      if selection_capabilities.width.is_range_set_from_auto()
        && let Some(wdth_parameters) = wdth_parameters
      {
        let wdth_range = wdth_parameters.range();

        if wdth_range.is_valid() {
          width_coordinate.value = wdth_range.clamp_to_range(selection_request.width).to_f32();
        }
      }

      let ital_parameters = self.retrieve_variation_design_parameters_by_tag(ITAL_TAG);
      let use_ital_axis = font_description.is_italic_keyword
        && ital_parameters.is_some_and(|ital| ital.includes(1.0));

      // The italic keyword instantiates `ital` at 1 and any other style at 0, as matching
      // never satisfies an oblique request with it.
      if ital_parameters.is_some() {
        variation.push(Variation {
          tag: ITAL_TAG,
          value: if use_ital_axis { 1.0 } else { 0.0 },
        });
      }

      // CSS and OpenType slant in opposite directions.
      let mut slant_coordinate = Variation {
        tag: SLNT_TAG,
        value: (-selection_capabilities
          .slope
          .clamp_to_range(selection_request.slope))
        .to_f32(),
      };
      let slnt_parameters = self.retrieve_variation_design_parameters_by_tag(SLNT_TAG);

      if use_ital_axis && let Some(slnt_parameters) = slnt_parameters {
        slant_coordinate.value = slnt_parameters.default;
      } else if selection_capabilities.slope.is_range_set_from_auto()
        && let Some(slnt_parameters) = slnt_parameters
      {
        let slnt_range = slnt_parameters.range();

        if slnt_range.is_valid() {
          slant_coordinate.value = slnt_range.clamp_to_range(-selection_request.slope).to_f32();

          let has_right_slanted_variations = slnt_range.minimum < NORMAL_SLOPE_VALUE;

          synthetic_italic = synthesize_italic
            && !has_right_slanted_variations
            && selection_request.slope >= ITALIC_SLOPE_VALUE;
        }
      }

      if use_ital_axis {
        synthetic_italic = false;
      }

      variation.push(weight_coordinate);
      variation.push(width_coordinate);
      variation.push(slant_coordinate);

      let mut explicit_opsz_configured = false;

      if font_description.variation_settings.len() < usize::from(u16::MAX) {
        for setting in &font_description.variation_settings {
          if setting.tag == OPSZ_TAG {
            explicit_opsz_configured = true;
          }

          variation.push(*setting);
        }
      }

      if !explicit_opsz_configured {
        match font_description.font_optical_sizing {
          FontOpticalSizing::Auto => variation.push(Variation {
            tag: OPSZ_TAG,
            value: font_description.adjusted_specified_size,
          }),
          FontOpticalSizing::None => {
            if let Some(opsz_parameters) =
              self.retrieve_variation_design_parameters_by_tag(OPSZ_TAG)
            {
              variation.push(Variation {
                tag: OPSZ_TAG,
                value: opsz_parameters.default,
              });
            }
          }
        }
      }
    }

    FontPlatformData {
      typeface: Arc::clone(self),
      synthetic_bold: synthetic_bold && !self.is_bold(),
      synthetic_italic: synthetic_italic && !self.is_italic(),
      variations: variation,
    }
  }

  /// The decoded asset at `path`, from the crate root, and its id.
  #[cfg(test)]
  pub(crate) fn read_asset(path: &str) -> (SharedBytes, u64) {
    use std::{fs::read, path::Path};

    use crate::resources::font::FontSource;

    let bytes = read(Path::new(env!("CARGO_MANIFEST_DIR")).join(path)).unwrap();

    FontSource::from(bytes).into_sfnt_with_id().unwrap()
  }

  /// The faces of the asset at `path`, from the crate root.
  #[cfg(test)]
  pub(crate) fn from_asset(path: &str) -> Vec<Arc<Self>> {
    use skrifa::raw::FileRef;

    let (data, id) = Self::read_asset(path);
    let count = FileRef::new((*data).as_ref()).unwrap().fonts().count();

    (0..count as u32)
      .map(|index| Arc::new(Self::create(Arc::clone(&data), id, index).unwrap()))
      .collect()
  }
}

impl FontPlatformData {
  /// The coordinate the instance sets `tag` to, the last one winning.
  #[cfg(test)]
  pub(crate) fn variation(&self, tag: &[u8; 4]) -> Option<f32> {
    let tag = Tag::new(tag);

    self
      .variations
      .iter()
      .rev()
      .find(|variation| variation.tag == tag)
      .map(|variation| variation.value)
  }
}

impl Debug for FontCustomPlatformData {
  fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
    f.debug_struct("FontCustomPlatformData")
      .field("id", &self.id)
      .field("index", &self.index)
      .field("axes", &self.axes)
      .field("attributes", &self.attributes)
      .finish_non_exhaustive()
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::fonts::font_selection_types::{
    BOLD_WEIGHT_VALUE, FontSelectionRequest, NORMAL_WIDTH_VALUE, RangeType,
  };

  const GEIST: &str = "../assets/fonts/geist/Geist[wght].woff2";
  const ARCHIVO: &str = "../assets/fonts/archivo/Archivo-VariableFont_wdth,wght.ttf";
  const INTER: &str = "../assets/fonts/inter/inter-latin-opsz-wght-normal.woff2";
  const POPPINS_BOLD: &str = "../assets/fonts/poppins/poppins-v24-devanagari_latin-700.woff2";

  fn description(weight: f32, slope: f32) -> FontSelectionDescription {
    FontSelectionDescription::for_request(FontSelectionRequest::new(
      FontSelectionValue::new(weight),
      NORMAL_WIDTH_VALUE,
      FontSelectionValue::new(slope),
    ))
  }

  fn first(path: &str) -> Arc<FontCustomPlatformData> {
    FontCustomPlatformData::from_asset(path).remove(0)
  }

  #[test]
  fn a_variable_face_spans_its_axes() {
    let geist = first(GEIST);
    let capabilities = geist.auto_font_selection_capabilities();

    assert_eq!(
      geist.variable_font_sub_type,
      VariableFontSubType::VariableTrueType
    );
    assert_eq!(
      capabilities.weight,
      FontSelectionRange::new(
        FontSelectionValue::new(100.0),
        FontSelectionValue::new(900.0)
      )
    );
    assert!(capabilities.weight.is_range_set_from_auto());
    assert_eq!(
      capabilities.width,
      FontSelectionRange::single(NORMAL_WIDTH_VALUE)
    );
    assert_eq!(
      capabilities.slope,
      FontSelectionRange::single(NORMAL_SLOPE_VALUE)
    );

    let archivo = first(ARCHIVO).auto_font_selection_capabilities();

    assert!(archivo.width.is_range());
    assert!(archivo.weight.is_range());
  }

  #[test]
  fn a_static_face_takes_its_weight_class() {
    let poppins = first(POPPINS_BOLD);

    assert_eq!(
      poppins.auto_font_selection_capabilities().weight,
      FontSelectionRange::single(BOLD_WEIGHT_VALUE)
    );
    assert!(poppins.is_bold());
  }

  #[test]
  fn a_variable_face_instances_the_requested_weight() {
    let geist = first(GEIST);
    let capabilities = geist.auto_font_selection_capabilities();
    let mut bold = description(700.0, 0.0);

    bold.is_synthetic_bold = true;

    let platform_data = geist.get_font_platform_data(&bold, &capabilities);

    assert_eq!(platform_data.variation(b"wght"), Some(700.0));
    assert_eq!(platform_data.variation(b"slnt"), Some(0.0));
    assert!(!platform_data.synthetic_bold);

    let heavier = geist.get_font_platform_data(&description(1000.0, 0.0), &capabilities);

    assert_eq!(heavier.variation(b"wght"), Some(900.0));
  }

  #[test]
  fn an_explicit_weight_clamps_to_the_descriptor() {
    let geist = first(GEIST);
    let mut capabilities = geist.auto_font_selection_capabilities();

    capabilities.weight = FontSelectionRange::with_type(
      FontSelectionValue::new(300.0),
      FontSelectionValue::new(500.0),
      RangeType::SetExplicitly,
    );

    let platform_data = geist.get_font_platform_data(&description(800.0, 0.0), &capabilities);

    assert_eq!(platform_data.variation(b"wght"), Some(500.0));
  }

  #[test]
  fn a_static_face_synthesizes_what_it_lacks() {
    let poppins = first(POPPINS_BOLD);
    let capabilities = poppins.auto_font_selection_capabilities();
    let mut italic = description(700.0, 14.0);

    italic.is_synthetic_bold = true;
    italic.is_synthetic_italic = true;

    let platform_data = poppins.get_font_platform_data(&italic, &capabilities);

    assert!(platform_data.variations.is_empty());
    assert!(!platform_data.synthetic_bold, "the face is already bold");
    assert!(platform_data.synthetic_italic);

    italic.synthetic_italic_allowed = false;

    assert!(
      !poppins
        .get_font_platform_data(&italic, &capabilities)
        .synthetic_italic
    );
  }

  #[test]
  fn optical_sizing_follows_the_size_unless_set() {
    let inter = first(INTER);
    let capabilities = inter.auto_font_selection_capabilities();
    let mut auto = description(400.0, 0.0);

    auto.font_optical_sizing = FontOpticalSizing::Auto;
    auto.adjusted_specified_size = 24.0;

    assert_eq!(
      inter
        .get_font_platform_data(&auto, &capabilities)
        .variation(b"opsz"),
      Some(24.0)
    );

    auto.variation_settings = vec![Variation {
      tag: OPSZ_TAG,
      value: 18.0,
    }];

    assert_eq!(
      inter
        .get_font_platform_data(&auto, &capabilities)
        .variation(b"opsz"),
      Some(18.0)
    );

    let opsz_default = inter
      .retrieve_variation_design_parameters_by_tag(OPSZ_TAG)
      .map(|axis| axis.default);

    assert_eq!(
      inter
        .get_font_platform_data(&description(400.0, 0.0), &capabilities)
        .variation(b"opsz"),
      opsz_default
    );
  }
}
