//! Small capitals synthesized for a font that lacks the caps feature `font-variant-caps` asks for.
//! Text splits where its case changes, and the runs to convert shape as capitals at 70% size, as
//! Blink's `OpenTypeCapsSupport` and `SmallCapsIterator` do.
//! <https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/platform/fonts/opentype/open_type_caps_support.cc>
//!
//! Each run of the text checks the font fallback picks for it, under its script's default
//! language system, as Blink's `HarfBuzzShaper` does.
//! Approximate: neutral characters take the script before them, or after them at the start,
//! where Blink's `ScriptRunIterator` also pairs brackets; an Indic script never tries its newer
//! OpenType tags; and AAT `morx` caps go undetected.

use std::{borrow::Cow, ops::Range};

use parley::LineHeight;

use icu_properties::{
  CodePointMapData, CodePointSetData, PropertyNamesShort,
  props::{CanonicalCombiningClass, ChangesWhenUppercased, Script},
};
use skrifa::{
  FontRef, MetadataProvider,
  raw::{TableProvider, tables::layout::FeatureRecord, types::Tag},
};
use smallvec::{SmallVec, smallvec};

use crate::{context::RenderContext, font_style::SizedFontStyle, style::FontVariantCaps};

/// Blink's `kSmallCapsFontSizeMultiplier`.
const SMALL_CAPS_FONT_SIZE_MULTIPLIER: f32 = 0.7;

/// How the font serves the requested caps.
#[derive(Clone, Copy, Debug, PartialEq)]
enum FontSupport {
  Full,
  /// The font lacks the feature but has a close one to use instead.
  Fallback,
  None,
}

/// Which letters synthesis turns into small capitals.
#[derive(Clone, Copy, Debug, PartialEq)]
enum CapsSynthesis {
  None,
  LowerToSmallCaps,
  UpperToSmallCaps,
  BothToSmallCaps,
}

/// Whether a run's letters change when uppercased, as Blink's `SmallCapsBehavior`.
#[derive(Clone, Copy, Debug, PartialEq)]
enum CaseClass {
  UppercaseNeeded,
  SameCase,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum CaseMap {
  Keep,
  Upper,
  Lower,
}

impl CaseMap {
  fn apply<'t>(self, text: &'t str) -> Cow<'t, str> {
    match self {
      Self::Keep => Cow::Borrowed(text),
      Self::Upper => Cow::Owned(text.to_uppercase()),
      Self::Lower => Cow::Owned(text.to_lowercase()),
    }
  }
}

/// How one case run shapes.
#[derive(Clone, Copy, Debug, PartialEq)]
struct CapsRun {
  /// The caps feature the run shapes with.
  caps: FontVariantCaps,
  /// Whether the run shapes at the synthetic small-caps size.
  synthetic: bool,
  case_map: CaseMap,
}

/// `font-variant-caps` as a font can render it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct CapsSupport {
  requested: FontVariantCaps,
  font_support: FontSupport,
  synthesis: CapsSynthesis,
  synthesis_allowed: bool,
}

impl CapsSupport {
  /// Splits `text` into the runs it shapes as, each case-mapped and under its own copy of `style`:
  /// first where the caps the font renders change, then where the case changes inside a run that
  /// synthesizes them.
  pub(super) fn styled_runs<'s>(
    text: Cow<'_, str>,
    style: SizedFontStyle<'s>,
    context: &RenderContext,
  ) -> SmallVec<[(String, SizedFontStyle<'s>); 1]> {
    if style.parent.inherited_data.font_variant_caps == FontVariantCaps::Normal {
      return smallvec![(text.into_owned(), style)];
    }

    let segments = Self::segments(&text, &style, context);

    if let [(_, support)] = segments.as_slice()
      && !support.splits_runs()
    {
      return smallvec![(
        text.into_owned(),
        style.for_caps_run(support.run(CaseClass::SameCase))
      )];
    }

    let mut runs = SmallVec::new();

    for (segment, support) in segments {
      let case_runs = if support.splits_runs() {
        case_runs(&text, segment)
      } else {
        smallvec![(segment, CaseClass::SameCase)]
      };

      for (range, class) in case_runs {
        let run = support.run(class);
        let mut run_style = style.clone().for_caps_run(run);

        run_style.case_run_parity = Some(runs.len() % 2 == 1);
        runs.push((run.case_map.apply(&text[range]).into_owned(), run_style));
      }
    }

    runs
  }

  /// Splits `text` where the font fallback picks or the script changes the caps `style` gets.
  fn segments(
    text: &str,
    style: &SizedFontStyle,
    context: &RenderContext,
  ) -> SmallVec<[(Range<usize>, Self); 2]> {
    let inherited = &style.parent.inherited_data;
    let requested = inherited.font_variant_caps;
    let synthesis_allowed = inherited.font_synthesis_small_caps.is_allowed();
    let blobs = context.fallback_fonts(
      style.font_family.query_families(),
      style.parent.font_attributes(),
    );
    let fonts: SmallVec<[FontRef; 4]> = blobs
      .iter()
      .filter_map(|(blob, index)| FontRef::from_index(blob.data(), *index).ok())
      .collect();
    let font_caps: SmallVec<[FontCaps; 4]> = fonts.iter().map(FontCaps::of).collect();
    let supports = FontRun::split(text, &fonts).into_iter().map(|run| {
      let caps = font_caps.get(run.font);

      (
        run.range,
        Self::new(requested, synthesis_allowed, |feature| {
          caps.is_some_and(|caps| caps.supports(run.script, feature))
        }),
      )
    });
    let mut segments: SmallVec<[(Range<usize>, Self); 2]> = SmallVec::new();

    for (range, support) in supports {
      match segments.last_mut() {
        Some((last, last_support)) if *last_support == support => last.end = range.end,
        _ => segments.push((range, support)),
      }
    }

    segments
  }

  /// Blink's `DetermineFontSupport`.
  fn new(
    requested: FontVariantCaps,
    synthesis_allowed: bool,
    supports: impl Fn(Tag) -> bool,
  ) -> Self {
    let has = |tag: &[u8; 4]| supports(Tag::new(tag));
    let (font_support, synthesis) = match requested {
      FontVariantCaps::Normal => (FontSupport::Full, CapsSynthesis::None),
      FontVariantCaps::SmallCaps if !has(b"smcp") => {
        (FontSupport::None, CapsSynthesis::LowerToSmallCaps)
      }
      FontVariantCaps::AllSmallCaps if !(has(b"smcp") && has(b"c2sc")) => {
        (FontSupport::None, CapsSynthesis::BothToSmallCaps)
      }
      FontVariantCaps::PetiteCaps if !has(b"pcap") => {
        if has(b"smcp") {
          (FontSupport::Fallback, CapsSynthesis::None)
        } else {
          (FontSupport::None, CapsSynthesis::LowerToSmallCaps)
        }
      }
      FontVariantCaps::AllPetiteCaps if !(has(b"pcap") && has(b"c2pc")) => {
        if has(b"smcp") && has(b"c2sc") {
          (FontSupport::Fallback, CapsSynthesis::None)
        } else {
          (FontSupport::None, CapsSynthesis::BothToSmallCaps)
        }
      }
      FontVariantCaps::Unicase if !has(b"unic") => {
        if has(b"smcp") {
          (FontSupport::Fallback, CapsSynthesis::UpperToSmallCaps)
        } else {
          (FontSupport::None, CapsSynthesis::UpperToSmallCaps)
        }
      }
      FontVariantCaps::TitlingCaps if !has(b"titl") => (FontSupport::None, CapsSynthesis::None),
      _ => (FontSupport::Full, CapsSynthesis::None),
    };

    Self {
      requested,
      font_support,
      synthesis,
      synthesis_allowed,
    }
  }

  /// Blink's `NeedsRunCaseSplitting`: titling caps are never synthesized.
  fn splits_runs(self) -> bool {
    self.font_support != FontSupport::Full
      && self.requested != FontVariantCaps::TitlingCaps
      && self.synthesis_allowed
  }

  /// How a run of `class` letters shapes, as Blink's `FontFeatureToUse`, `NeedsSyntheticFont` and
  /// `NeedsCaseChange` decide.
  fn run(self, class: CaseClass) -> CapsRun {
    let caps = match self.font_support {
      FontSupport::Full => self.requested,
      FontSupport::Fallback => match self.requested {
        FontVariantCaps::AllPetiteCaps => FontVariantCaps::AllSmallCaps,
        FontVariantCaps::PetiteCaps => FontVariantCaps::SmallCaps,
        FontVariantCaps::Unicase if class == CaseClass::SameCase => FontVariantCaps::SmallCaps,
        _ => FontVariantCaps::Normal,
      },
      FontSupport::None => FontVariantCaps::Normal,
    };

    if !self.splits_runs() {
      return CapsRun {
        caps,
        synthetic: false,
        case_map: CaseMap::Keep,
      };
    }

    let lower_to_small = matches!(
      self.synthesis,
      CapsSynthesis::LowerToSmallCaps | CapsSynthesis::BothToSmallCaps
    );
    let upper_to_small = matches!(
      self.synthesis,
      CapsSynthesis::UpperToSmallCaps | CapsSynthesis::BothToSmallCaps
    );
    let (synthetic, case_map) = match class {
      CaseClass::UppercaseNeeded => (
        self.font_support == FontSupport::None && lower_to_small,
        if self.font_support != FontSupport::Fallback && lower_to_small {
          CaseMap::Upper
        } else {
          CaseMap::Keep
        },
      ),
      CaseClass::SameCase => (
        self.font_support == FontSupport::None && upper_to_small,
        if self.font_support == FontSupport::Fallback && upper_to_small {
          CaseMap::Lower
        } else {
          CaseMap::Keep
        },
      ),
    };

    CapsRun {
      caps,
      synthetic,
      case_map,
    }
  }
}

impl SizedFontStyle<'_> {
  /// This style shaping one case run of `font-variant-caps` text, at Blink's synthetic small-caps
  /// size when the run synthesizes them.
  fn for_caps_run(mut self, run: CapsRun) -> Self {
    self.caps = run.caps;

    if run.synthetic {
      let font_size = self.sizing.font_size;

      if let LineHeight::FontSizeRelative(value) = self.line_height {
        self.line_height = LineHeight::Absolute(value * font_size);
      }
      self.sizing.font_size = (font_size * SMALL_CAPS_FONT_SIZE_MULTIPLIER).round();
    }

    self
  }
}

/// The features each script of a font's GSUB lists under its default language system.
#[derive(Default)]
struct FontCaps {
  scripts: SmallVec<[(Tag, SmallVec<[Tag; 16]>); 4]>,
}

impl FontCaps {
  fn of(font: &FontRef) -> Self {
    let Ok(gsub) = font.gsub() else {
      return Self::default();
    };
    let (Ok(script_list), Ok(feature_list)) = (gsub.script_list(), gsub.feature_list()) else {
      return Self::default();
    };
    let features = feature_list.feature_records();
    let scripts = script_list
      .script_records()
      .iter()
      .map(|record| {
        let tags = record
          .script(script_list.offset_data())
          .ok()
          .and_then(|script| script.default_lang_sys()?.ok())
          .map(|lang_sys| {
            lang_sys
              .feature_indices()
              .iter()
              .filter_map(|index| features.get(usize::from(index.get())))
              .map(FeatureRecord::feature_tag)
              .collect()
          })
          .unwrap_or_default();

        (record.script_tag(), tags)
      })
      .collect();

    Self { scripts }
  }

  /// Whether the font has `feature` for text in `script`, under the script HarfBuzz's
  /// `hb_ot_layout_table_select_script` falls back to, `None` taking the default script.
  fn supports(&self, script: Option<Tag>, feature: Tag) -> bool {
    script
      .into_iter()
      .chain([Tag::new(b"DFLT"), Tag::new(b"dflt"), Tag::new(b"latn")])
      .find_map(|tag| self.scripts.iter().find(|(script, _)| *script == tag))
      .is_some_and(|(_, features)| features.contains(&feature))
  }
}

/// A stretch of text one font draws in one script.
struct FontRun {
  range: Range<usize>,
  /// Its font, among the fonts text falls back through.
  font: usize,
  /// Its OpenType script, `None` while only neutral characters fill it.
  script: Option<Tag>,
}

impl FontRun {
  /// Splits `text` where the font among `fonts` fallback picks or the OpenType script changes. A
  /// combining mark stays with the character before it, and a neutral character takes the script
  /// before it, or the one after it at the start.
  fn split(text: &str, fonts: &[FontRef]) -> SmallVec<[Self; 2]> {
    let combining_class = CodePointMapData::<CanonicalCombiningClass>::new();
    let scripts = CodePointMapData::<Script>::new();
    let charmaps: SmallVec<[_; 4]> = fonts.iter().map(MetadataProvider::charmap).collect();
    let mut runs: SmallVec<[Self; 2]> = SmallVec::new();

    for (index, ch) in text.char_indices() {
      if combining_class.get(ch) != CanonicalCombiningClass::NotReordered {
        continue;
      }

      let font = charmaps
        .iter()
        .position(|charmap| charmap.map(ch).is_some())
        .unwrap_or(0);
      let script = ot_script_tag(scripts.get(ch));

      let Some(last) = runs.last_mut() else {
        runs.push(Self {
          range: 0..text.len(),
          font,
          script,
        });
        continue;
      };

      if last.font == font && (script.is_none() || last.script == script) {
        continue;
      }

      if last.font == font && last.script.is_none() {
        last.script = script;
        continue;
      }

      last.range.end = index;
      runs.push(Self {
        range: index..text.len(),
        font,
        script,
      });
    }

    runs
  }
}

/// The OpenType script tag for `script`, `None` for a neutral one.
fn ot_script_tag(script: Script) -> Option<Tag> {
  if matches!(script, Script::Common | Script::Inherited | Script::Unknown) {
    return None;
  }

  let code = PropertyNamesShort::<Script>::new().get(script)?;
  let tag = match code {
    "Hira" | "Kana" => *b"kana",
    "Laoo" => *b"lao ",
    "Nkoo" => *b"nko ",
    "Vaii" => *b"vai ",
    "Yiii" => *b"yi  ",
    _ => {
      let mut tag = [0; 4];

      tag.copy_from_slice(code.as_bytes().get(..4)?);
      tag.make_ascii_lowercase();
      tag
    }
  };

  Some(Tag::new(&tag))
}

/// Splits `span` of `text` where its letters switch between needing uppercasing and not, as Blink's
/// `SmallCapsIterator`. A combining mark stays with the letter before it.
fn case_runs(text: &str, span: Range<usize>) -> SmallVec<[(Range<usize>, CaseClass); 4]> {
  let combining_class = CodePointMapData::<CanonicalCombiningClass>::new();
  let changes_when_uppercased = CodePointSetData::new::<ChangesWhenUppercased>();
  let mut runs: SmallVec<[(Range<usize>, CaseClass); 4]> = SmallVec::new();

  for (index, ch) in text[span.clone()].char_indices() {
    let index = span.start + index;

    if combining_class.get(ch) != CanonicalCombiningClass::NotReordered {
      continue;
    }

    let class = if changes_when_uppercased.contains(ch) {
      CaseClass::UppercaseNeeded
    } else {
      CaseClass::SameCase
    };

    match runs.last_mut() {
      Some((_, last)) if *last == class => {}
      Some((range, _)) => {
        range.end = index;
        runs.push((index..span.end, class));
      }
      None => runs.push((span.clone(), class)),
    }
  }

  if runs.is_empty() {
    runs.push((span, CaseClass::SameCase));
  }

  runs
}

#[cfg(test)]
mod tests {
  use super::*;

  fn support(requested: FontVariantCaps, features: &[&[u8; 4]]) -> CapsSupport {
    CapsSupport::new(requested, true, |tag| {
      features.iter().any(|feature| Tag::new(feature) == tag)
    })
  }

  fn runs(text: &str) -> Vec<(&str, CaseClass)> {
    case_runs(text, 0..text.len())
      .into_iter()
      .map(|(range, class)| (&text[range], class))
      .collect()
  }

  #[test]
  fn text_splits_where_its_case_changes() {
    assert_eq!(
      runs("Unterkunft 12"),
      vec![
        ("U", CaseClass::SameCase),
        ("nterkunft", CaseClass::UppercaseNeeded),
        (" 12", CaseClass::SameCase),
      ]
    );
  }

  #[test]
  fn a_combining_mark_stays_with_its_letter() {
    assert_eq!(
      runs("e\u{301}A"),
      vec![
        ("e\u{301}", CaseClass::UppercaseNeeded),
        ("A", CaseClass::SameCase),
      ]
    );
  }

  #[test]
  fn small_caps_without_smcp_scale_uppercased_lowercase() {
    let caps = support(FontVariantCaps::SmallCaps, &[]);

    assert_eq!(
      caps.run(CaseClass::UppercaseNeeded),
      CapsRun {
        caps: FontVariantCaps::Normal,
        synthetic: true,
        case_map: CaseMap::Upper,
      }
    );
    assert_eq!(
      caps.run(CaseClass::SameCase),
      CapsRun {
        caps: FontVariantCaps::Normal,
        synthetic: false,
        case_map: CaseMap::Keep,
      }
    );
  }

  #[test]
  fn small_caps_with_smcp_keep_the_feature() {
    let caps = support(FontVariantCaps::SmallCaps, &[b"smcp"]);

    assert!(!caps.splits_runs());
    assert_eq!(
      caps.run(CaseClass::SameCase).caps,
      FontVariantCaps::SmallCaps
    );
  }

  #[test]
  fn petite_caps_fall_back_to_smcp() {
    let caps = support(FontVariantCaps::PetiteCaps, &[b"smcp"]);

    assert_eq!(
      caps.run(CaseClass::UppercaseNeeded),
      CapsRun {
        caps: FontVariantCaps::SmallCaps,
        synthetic: false,
        case_map: CaseMap::Keep,
      }
    );
  }

  #[test]
  fn unicase_lowercases_capitals_into_smcp() {
    let caps = support(FontVariantCaps::Unicase, &[b"smcp"]);

    assert_eq!(
      caps.run(CaseClass::SameCase),
      CapsRun {
        caps: FontVariantCaps::SmallCaps,
        synthetic: false,
        case_map: CaseMap::Lower,
      }
    );
    assert_eq!(
      caps.run(CaseClass::UppercaseNeeded).caps,
      FontVariantCaps::Normal
    );
  }

  #[test]
  fn synthesis_none_keeps_the_text_whole() {
    let caps = CapsSupport::new(FontVariantCaps::AllSmallCaps, false, |_| false);

    assert!(!caps.splits_runs());
    assert!(!caps.run(CaseClass::UppercaseNeeded).synthetic);
  }
}
