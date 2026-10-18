//! Blink's [`OpenTypeCapsSupport`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/platform/fonts/opentype/open_type_caps_support.cc).

use std::cell::Cell;

use harfrust::{FontRef, Script, Tag, script};
use skrifa::raw::{FontRead, TableProvider, tables::gsub::Gsub};
use smallvec::SmallVec;

use crate::{
  fonts::{
    shaping::case_mapping_harfbuzz_buffer_filler::CaseMapIntend,
    small_caps_iterator::SmallCapsBehavior,
  },
  style::{FontSynthesic, FontVariantCaps},
};

/// HarfBuzz's `HB_AAT_LAYOUT_FEATURE_TYPE_LETTER_CASE`, the old-style small caps feature.
const AAT_LETTER_CASE: u16 = 3;
/// HarfBuzz's `HB_AAT_LAYOUT_FEATURE_SELECTOR_SMALL_CAPS`.
const AAT_SMALL_CAPS: u16 = 3;
/// HarfBuzz's `HB_AAT_LAYOUT_FEATURE_TYPE_LOWER_CASE`.
const AAT_LOWER_CASE: u16 = 37;
/// HarfBuzz's `HB_AAT_LAYOUT_FEATURE_SELECTOR_LOWER_CASE_SMALL_CAPS`.
const AAT_LOWER_CASE_SMALL_CAPS: u16 = 1;
/// HarfBuzz's `HB_AAT_LAYOUT_FEATURE_TYPE_UPPER_CASE`.
const AAT_UPPER_CASE: u16 = 38;
/// HarfBuzz's `HB_AAT_LAYOUT_FEATURE_SELECTOR_UPPER_CASE_SMALL_CAPS`.
const AAT_UPPER_CASE_SMALL_CAPS: u16 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FontFormat {
  OpenType,
  Aat,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FontSupport {
  Full,
  /// Falls back to `smcp`, or `smcp` and `c2sc`.
  Fallback,
  None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CapsSynthesis {
  None,
  LowerToSmallCaps,
  UpperToSmallCaps,
  BothToSmallCaps,
}

/// Blink's `OpenTypeCapsSupport`: how a font draws the caps `font-variant-caps` asks for, with
/// its features or with synthesized small capitals.
pub(crate) struct OpenTypeCapsSupport<'a> {
  font: Option<FontRef<'a>>,
  requested_caps: FontVariantCaps,
  font_synthesis_small_caps: FontSynthesic,
  font_support: FontSupport,
  caps_synthesis: CapsSynthesis,
  font_format: Cell<Option<FontFormat>>,
}

impl Default for OpenTypeCapsSupport<'_> {
  fn default() -> Self {
    Self {
      font: None,
      requested_caps: FontVariantCaps::Normal,
      font_synthesis_small_caps: FontSynthesic::Auto,
      font_support: FontSupport::Full,
      caps_synthesis: CapsSynthesis::None,
      font_format: Cell::new(None),
    }
  }
}

impl<'a> OpenTypeCapsSupport<'a> {
  pub(crate) fn new(
    font: FontRef<'a>,
    requested_caps: FontVariantCaps,
    font_synthesis_small_caps: FontSynthesic,
    script: Script,
  ) -> Self {
    let mut caps_support = Self {
      font: Some(font),
      requested_caps,
      font_synthesis_small_caps,
      ..Self::default()
    };

    if requested_caps != FontVariantCaps::Normal {
      caps_support.determine_font_support(script);
    }

    caps_support
  }

  /// Blink's `FontFeatureToUse`.
  pub(crate) fn font_feature_to_use(&self, source_text_case: SmallCapsBehavior) -> FontVariantCaps {
    match self.font_support {
      FontSupport::Full => self.requested_caps,
      FontSupport::Fallback => match self.requested_caps {
        FontVariantCaps::AllPetiteCaps => FontVariantCaps::AllSmallCaps,
        FontVariantCaps::PetiteCaps => FontVariantCaps::SmallCaps,
        FontVariantCaps::Unicase if source_text_case == SmallCapsBehavior::SameCase => {
          FontVariantCaps::SmallCaps
        }
        _ => FontVariantCaps::Normal,
      },
      FontSupport::None => FontVariantCaps::Normal,
    }
  }

  /// Blink's `NeedsRunCaseSplitting`; titling caps are not synthesized.
  pub(crate) fn needs_run_case_splitting(&self) -> bool {
    self.font_support != FontSupport::Full
      && self.requested_caps != FontVariantCaps::TitlingCaps
      && self.synthetic_small_caps_allowed()
  }

  /// Blink's `NeedsSyntheticFont`.
  pub(crate) fn needs_synthetic_font(&self, run_case: SmallCapsBehavior) -> bool {
    if self.font_support != FontSupport::None
      || self.requested_caps == FontVariantCaps::TitlingCaps
      || !self.synthetic_small_caps_allowed()
    {
      return false;
    }

    match run_case {
      SmallCapsBehavior::UppercaseNeeded => matches!(
        self.caps_synthesis,
        CapsSynthesis::LowerToSmallCaps | CapsSynthesis::BothToSmallCaps
      ),
      SmallCapsBehavior::SameCase => matches!(
        self.caps_synthesis,
        CapsSynthesis::UpperToSmallCaps | CapsSynthesis::BothToSmallCaps
      ),
      SmallCapsBehavior::Invalid => false,
    }
  }

  /// Blink's `NeedsCaseChange`.
  pub(crate) fn needs_case_change(&self, run_case: SmallCapsBehavior) -> CaseMapIntend {
    if self.font_support == FontSupport::Full || !self.synthetic_small_caps_allowed() {
      return CaseMapIntend::KeepSameCase;
    }

    match run_case {
      SmallCapsBehavior::SameCase
        if self.font_support == FontSupport::Fallback
          && matches!(
            self.caps_synthesis,
            CapsSynthesis::BothToSmallCaps | CapsSynthesis::UpperToSmallCaps
          ) =>
      {
        CaseMapIntend::LowerCase
      }
      SmallCapsBehavior::UppercaseNeeded
        if self.font_support != FontSupport::Fallback
          && matches!(
            self.caps_synthesis,
            CapsSynthesis::LowerToSmallCaps | CapsSynthesis::BothToSmallCaps
          ) =>
      {
        CaseMapIntend::UpperCase
      }
      _ => CaseMapIntend::KeepSameCase,
    }
  }

  /// Blink's `GetFontFormat`: AAT for a font with `morx` or `mort` and no `GSUB`.
  fn font_format(&self) -> FontFormat {
    if let Some(font_format) = self.font_format.get() {
      return font_format;
    }

    let has_table = |tag: &[u8; 4]| {
      self
        .font
        .as_ref()
        .and_then(|font| font.table_data(Tag::new(tag)))
        .is_some_and(|data| !data.is_empty())
    };
    let font_format = if (has_table(b"morx") || has_table(b"mort")) && !has_table(b"GSUB") {
      FontFormat::Aat
    } else {
      FontFormat::OpenType
    };

    self.font_format.set(Some(font_format));

    font_format
  }

  /// Blink's `SupportsFeature`.
  fn supports_feature(&self, script: Script, tag: &[u8; 4]) -> bool {
    match self.font_format() {
      FontFormat::Aat => self.supports_aat_feature(tag),
      FontFormat::OpenType => self.supports_open_type_feature(script, Tag::new(tag)),
    }
  }

  /// Blink's `SupportsAatFeature`, which only finds small caps.
  fn supports_aat_feature(&self, tag: &[u8; 4]) -> bool {
    match tag {
      b"smcp" => {
        self.has_aat_selector(AAT_LOWER_CASE, AAT_LOWER_CASE_SMALL_CAPS)
          || self.has_aat_selector(AAT_LETTER_CASE, AAT_SMALL_CAPS)
      }
      b"c2sc" => self.has_aat_selector(AAT_UPPER_CASE, AAT_UPPER_CASE_SMALL_CAPS),
      _ => false,
    }
  }

  /// Blink's `ActivationSelectorPresent`: whether the `feat` table has `feature_type` with a
  /// setting that turns on `selector`, as HarfBuzz's selector infos read it.
  fn has_aat_selector(&self, feature_type: u16, selector: u16) -> bool {
    let Some(feat) = self.font.as_ref().and_then(|font| font.feat().ok()) else {
      return false;
    };

    feat
      .names()
      .iter()
      .filter(|name| name.feature() == feature_type)
      .filter_map(|name| name.setting_table(feat.offset_data()).ok())
      .any(|settings| {
        settings
          .settings()
          .iter()
          .any(|setting| setting.setting() == selector)
      })
  }

  /// Blink's `SupportsOpenTypeFeature`: whether the GSUB script HarfBuzz selects for `script`
  /// lists `tag` under its default language system.
  fn supports_open_type_feature(&self, script: Script, tag: Tag) -> bool {
    let Some(gsub) = self
      .font
      .as_ref()
      .and_then(|font| font.table_data(Tag::new(b"GSUB")))
      .and_then(|data| Gsub::read(data).ok())
    else {
      return false;
    };
    let (Ok(script_list), Ok(feature_list)) = (gsub.script_list(), gsub.feature_list()) else {
      return false;
    };
    let script_records = script_list.script_records();
    // `hb_ot_layout_table_select_script`, after the script's tags.
    let Some(script_record) = script_tags(script)
      .into_iter()
      .chain([Tag::new(b"DFLT"), Tag::new(b"dflt"), Tag::new(b"latn")])
      .find_map(|script_tag| {
        script_records
          .iter()
          .find(|record| record.script_tag() == script_tag)
      })
    else {
      return false;
    };
    let Some(Ok(lang_sys)) = script_record
      .script(script_list.offset_data())
      .ok()
      .and_then(|script| script.default_lang_sys())
    else {
      return false;
    };
    let feature_records = feature_list.feature_records();

    lang_sys.feature_indices().iter().any(|index| {
      feature_records
        .get(usize::from(index.get()))
        .is_some_and(|record| record.feature_tag() == tag)
    })
  }

  /// Blink's `SyntheticSmallCapsAllowed`.
  fn synthetic_small_caps_allowed(&self) -> bool {
    self.font_synthesis_small_caps.is_allowed()
  }

  /// Blink's `DetermineFontSupport`.
  fn determine_font_support(&mut self, script: Script) {
    let supports = |tag| self.supports_feature(script, tag);

    let (font_support, caps_synthesis) = match self.requested_caps {
      FontVariantCaps::SmallCaps if !supports(b"smcp") => {
        (FontSupport::None, CapsSynthesis::LowerToSmallCaps)
      }
      FontVariantCaps::AllSmallCaps if !(supports(b"smcp") && supports(b"c2sc")) => {
        (FontSupport::None, CapsSynthesis::BothToSmallCaps)
      }
      FontVariantCaps::PetiteCaps if !supports(b"pcap") => {
        if supports(b"smcp") {
          (FontSupport::Fallback, CapsSynthesis::None)
        } else {
          (FontSupport::None, CapsSynthesis::LowerToSmallCaps)
        }
      }
      FontVariantCaps::AllPetiteCaps if !(supports(b"pcap") && supports(b"c2pc")) => {
        if supports(b"smcp") && supports(b"c2sc") {
          (FontSupport::Fallback, CapsSynthesis::None)
        } else {
          (FontSupport::None, CapsSynthesis::BothToSmallCaps)
        }
      }
      FontVariantCaps::Unicase if !supports(b"unic") => {
        if supports(b"smcp") {
          (FontSupport::Fallback, CapsSynthesis::UpperToSmallCaps)
        } else {
          (FontSupport::None, CapsSynthesis::UpperToSmallCaps)
        }
      }
      FontVariantCaps::TitlingCaps if !supports(b"titl") => {
        (FontSupport::None, CapsSynthesis::None)
      }
      _ => (FontSupport::Full, CapsSynthesis::None),
    };

    self.font_support = font_support;
    self.caps_synthesis = caps_synthesis;
  }
}

/// HarfBuzz's `hb_ot_tags_from_script_and_language` with room for two script tags, as Blink asks:
/// the newest Indic or Myanmar tags first, and the old tag otherwise.
fn script_tags(script: Script) -> SmallVec<[Tag; 2]> {
  let new_tag = match script {
    script::BENGALI => Some(*b"bng2"),
    script::DEVANAGARI => Some(*b"dev2"),
    script::GUJARATI => Some(*b"gjr2"),
    script::GURMUKHI => Some(*b"gur2"),
    script::KANNADA => Some(*b"knd2"),
    script::MALAYALAM => Some(*b"mlm2"),
    script::ORIYA => Some(*b"ory2"),
    script::TAMIL => Some(*b"tml2"),
    script::TELUGU => Some(*b"tel2"),
    script::MYANMAR => Some(*b"mym2"),
    _ => None,
  };

  if let Some(new_tag) = new_tag {
    // Myanmar has no third version.
    if &new_tag == b"mym2" {
      return SmallVec::from_buf([Tag::new(&new_tag), old_script_tag(script)]);
    }

    let mut tag3 = new_tag;

    tag3[3] = b'3';

    return SmallVec::from_buf([Tag::new(&tag3), Tag::new(&new_tag)]);
  }

  SmallVec::from_slice(&[old_script_tag(script)])
}

/// HarfBuzz's `hb_ot_old_tag_from_script`.
fn old_script_tag(script: Script) -> Tag {
  match script {
    script::MATH => Tag::new(b"math"),
    script::HIRAGANA => Tag::new(b"kana"),
    script::LAO => Tag::new(b"lao "),
    script::YI => Tag::new(b"yi  "),
    script::NKO => Tag::new(b"nko "),
    script::VAI => Tag::new(b"vai "),
    _ => Tag::from_u32(u32::from_be_bytes(script.tag().to_be_bytes()) | 0x2000_0000),
  }
}
