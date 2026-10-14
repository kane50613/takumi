//! Small capitals synthesized for a font that lacks the caps feature `font-variant-caps` asks for.
//! Text splits where its case changes, and the runs to convert shape as capitals at 70% size, as
//! Blink's `OpenTypeCapsSupport` and `SmallCapsIterator` do.
//! <https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/platform/fonts/opentype/open_type_caps_support.cc>
//!
//! Approximate: Blink checks every font that fallback picks, but takumi checks only the primary
//! font. A feature counts when GSUB lists it under any script, where Blink reads only the run
//! script's default language system. AAT `morx` caps go undetected.

use std::{borrow::Cow, ops::Range};

use icu_properties::{
  CodePointMapData, CodePointSetData,
  props::{CanonicalCombiningClass, ChangesWhenUppercased},
};
use skrifa::{
  FontRef,
  raw::{TableProvider, tables::layout::FeatureRecord, types::Tag},
};
use smallvec::{SmallVec, smallvec};

use crate::{context::RenderContext, font_style::SizedFontStyle, style::FontVariantCaps};

/// Blink's `kSmallCapsFontSizeMultiplier`.
pub(crate) const SMALL_CAPS_FONT_SIZE_MULTIPLIER: f32 = 0.7;

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
pub(crate) struct CapsRun {
  /// The caps feature the run shapes with.
  pub(crate) caps: FontVariantCaps,
  /// Whether the run shapes at the synthetic small-caps size.
  pub(crate) synthetic: bool,
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
  /// The caps `style` asks for, as its primary font can render them.
  pub(super) fn of(style: &SizedFontStyle, context: &RenderContext) -> Self {
    let inherited = &style.parent.inherited_data;
    let requested = inherited.font_variant_caps;
    let synthesis_allowed = inherited.font_synthesis_small_caps.is_allowed();

    if requested == FontVariantCaps::Normal {
      return Self::new(requested, synthesis_allowed, |_| true);
    }

    let features = context
      .with_primary_font(
        style.font_family.query_families(),
        style.parent.font_attributes(),
        gsub_feature_tags,
      )
      .unwrap_or_default();

    Self::new(requested, synthesis_allowed, |tag| features.contains(&tag))
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

  /// Splits `text` into the runs it shapes as, each case-mapped and under its own copy of `style`.
  pub(super) fn styled_runs<'s>(
    self,
    text: Cow<'_, str>,
    style: SizedFontStyle<'s>,
  ) -> SmallVec<[(String, SizedFontStyle<'s>); 1]> {
    if !self.splits_runs() {
      return smallvec![(
        text.into_owned(),
        style.for_caps_run(self.run(CaseClass::SameCase))
      )];
    }

    case_runs(&text)
      .map(|(range, class)| {
        let run = self.run(class);

        (
          run.case_map.apply(&text[range]).into_owned(),
          style.clone().for_caps_run(run),
        )
      })
      .collect()
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

/// The feature tags the font's GSUB lists.
fn gsub_feature_tags(font: &FontRef) -> SmallVec<[Tag; 32]> {
  font
    .gsub()
    .and_then(|gsub| gsub.feature_list())
    .map(|list| {
      list
        .feature_records()
        .iter()
        .map(FeatureRecord::feature_tag)
        .collect()
    })
    .unwrap_or_default()
}

/// Splits `text` where its letters switch between needing uppercasing and not, as Blink's
/// `SmallCapsIterator`. A combining mark stays with the letter before it.
fn case_runs(text: &str) -> impl Iterator<Item = (Range<usize>, CaseClass)> + '_ {
  let combining_class = CodePointMapData::<CanonicalCombiningClass>::new();
  let changes_when_uppercased = CodePointSetData::new::<ChangesWhenUppercased>();
  let mut starts: SmallVec<[(usize, CaseClass); 4]> = SmallVec::new();

  for (index, ch) in text.char_indices() {
    if combining_class.get(ch) != CanonicalCombiningClass::NotReordered {
      continue;
    }

    let class = if changes_when_uppercased.contains(ch) {
      CaseClass::UppercaseNeeded
    } else {
      CaseClass::SameCase
    };

    match starts.last() {
      Some((_, last)) if *last == class => {}
      Some(_) => starts.push((index, class)),
      None => starts.push((0, class)),
    }
  }
  if starts.is_empty() {
    starts.push((0, CaseClass::SameCase));
  }

  let ends: SmallVec<[usize; 4]> = starts
    .iter()
    .skip(1)
    .map(|(start, _)| *start)
    .chain([text.len()])
    .collect();

  starts
    .into_iter()
    .zip(ends)
    .map(|((start, class), end)| (start..end, class))
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
    case_runs(text)
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
