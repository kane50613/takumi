//! `unicode-bidi` data source backed by the icu_properties tables parley
//! already links, replacing the crate's `hardcoded-data` feature.

use icu_properties::CodePointMapData;
use icu_properties::props::{BidiMirroringGlyph, BidiPairedBracketType};
use unicode_bidi::data_source::BidiMatchedOpeningBracket;
use unicode_bidi::{BidiClass, BidiDataSource};

pub(crate) struct IcuBidiData;

impl BidiDataSource for IcuBidiData {
  fn bidi_class(&self, c: char) -> BidiClass {
    use icu_properties::props::BidiClass as Icu;

    match CodePointMapData::<Icu>::new().get(c) {
      Icu::LeftToRight => BidiClass::L,
      Icu::RightToLeft => BidiClass::R,
      Icu::EuropeanNumber => BidiClass::EN,
      Icu::EuropeanSeparator => BidiClass::ES,
      Icu::EuropeanTerminator => BidiClass::ET,
      Icu::ArabicNumber => BidiClass::AN,
      Icu::CommonSeparator => BidiClass::CS,
      Icu::ParagraphSeparator => BidiClass::B,
      Icu::SegmentSeparator => BidiClass::S,
      Icu::WhiteSpace => BidiClass::WS,
      Icu::OtherNeutral => BidiClass::ON,
      Icu::LeftToRightEmbedding => BidiClass::LRE,
      Icu::LeftToRightOverride => BidiClass::LRO,
      Icu::ArabicLetter => BidiClass::AL,
      Icu::RightToLeftEmbedding => BidiClass::RLE,
      Icu::RightToLeftOverride => BidiClass::RLO,
      Icu::PopDirectionalFormat => BidiClass::PDF,
      Icu::NonspacingMark => BidiClass::NSM,
      Icu::BoundaryNeutral => BidiClass::BN,
      Icu::FirstStrongIsolate => BidiClass::FSI,
      Icu::LeftToRightIsolate => BidiClass::LRI,
      Icu::RightToLeftIsolate => BidiClass::RLI,
      Icu::PopDirectionalIsolate => BidiClass::PDI,
      _ => BidiClass::L,
    }
  }

  fn bidi_matched_opening_bracket(&self, c: char) -> Option<BidiMatchedOpeningBracket> {
    // BidiBrackets.txt's only canonical-equivalence pairs.
    fn normalize(c: char) -> char {
      match c {
        '\u{2329}' => '\u{3008}',
        '\u{232A}' => '\u{3009}',
        c => c,
      }
    }

    let glyph = CodePointMapData::<BidiMirroringGlyph>::new().get(c);

    match glyph.paired_bracket_type {
      BidiPairedBracketType::Open => Some(BidiMatchedOpeningBracket {
        opening: normalize(c),
        is_open: true,
      }),
      BidiPairedBracketType::Close => Some(BidiMatchedOpeningBracket {
        opening: normalize(glyph.mirroring_glyph?),
        is_open: false,
      }),
      _ => None,
    }
  }
}

#[cfg(test)]
mod tests {
  use unicode_bidi::{BidiDataSource, HardcodedBidiData};

  use super::IcuBidiData;

  /// Codepoints where the two sources disagree because icu tracks a newer
  /// Unicode than `unicode-bidi` 0.3.18's tables: characters assigned after
  /// the crate's snapshot, plus noncharacters. New entries after an icu bump
  /// need the same "unassigned or noncharacter in the older table" review.
  fn known_version_skew(point: u32) -> bool {
    matches!(
      point,
      0x086B..=0x086F
        | 0x088F
        | 0x0892..=0x0896
        | 0x1ACF..=0x1ADD
        | 0x1AE0..=0x1AEB
        | 0x2065
        | 0x2B96
        | 0xFBC3..=0xFBD2
        | 0xFD90..=0xFD91
        | 0xFDC8..=0xFDCE
        | 0xFDD0..=0xFDEF
        | 0xFFF0..=0xFFF8
        | 0xFFFE..=0xFFFF
    )
  }

  #[test]
  fn matches_hardcoded_tables_across_the_bmp() {
    for point in 0..=0xFFFFu32 {
      let Some(c) = char::from_u32(point) else {
        continue;
      };

      if !known_version_skew(point) {
        assert_eq!(
          IcuBidiData.bidi_class(c),
          HardcodedBidiData.bidi_class(c),
          "bidi_class mismatch at U+{point:04X}"
        );
      }

      let ours = IcuBidiData
        .bidi_matched_opening_bracket(c)
        .map(|b| (b.opening, b.is_open));
      let reference = HardcodedBidiData
        .bidi_matched_opening_bracket(c)
        .map(|b| (b.opening, b.is_open));

      assert_eq!(ours, reference, "bracket mismatch at U+{point:04X}");
    }
  }
}
