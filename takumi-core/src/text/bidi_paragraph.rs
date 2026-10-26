//! Blink's [`BidiParagraph`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/platform/text/bidi_paragraph.cc),
//! with `unicode-bidi`'s algorithm in place of ICU's `ubidi`.

use icu_properties::{CodePointMapData, props::BidiClass};
use unicode_bidi::{BidiInfo, Level};

use crate::{style::Direction as TextDirection, text::bidi_data::IcuBidiData};

/// Blink's `DirectionFromLevel`.
pub(crate) fn direction_from_level(level: u8) -> TextDirection {
  if level & 1 == 0 {
    TextDirection::Ltr
  } else {
    TextDirection::Rtl
  }
}

/// Blink's `BidiParagraph::Run`: a run of one bidi level.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Run {
  pub(crate) start: usize,
  pub(crate) end: usize,
  pub(crate) level: u8,
}

/// Blink's `BidiParagraph`.
#[derive(Debug, Default)]
pub(crate) struct BidiParagraph {
  /// The resolved levels as maximal runs, which `ubidi_getLogicalRun` walks.
  runs: Vec<Run>,
  paragraph_level: u8,
  base_direction: TextDirection,
}

impl BidiParagraph {
  pub(crate) fn new(text: &str, base_direction: Option<TextDirection>) -> Self {
    let mut paragraph = Self::default();

    paragraph.set_paragraph(text, base_direction);
    paragraph
  }

  /// Blink's `SetParagraph`: the paragraph level is `base_direction`, or the first strong
  /// character's direction, left-to-right by default.
  pub(crate) fn set_paragraph(
    &mut self,
    text: &str,
    base_direction: Option<TextDirection>,
  ) -> bool {
    let info = BidiInfo::new_with_data_source(
      &IcuBidiData,
      text,
      base_direction.map(|direction| match direction {
        TextDirection::Ltr => Level::ltr(),
        TextDirection::Rtl => Level::rtl(),
      }),
    );
    // ICU's `ubidi_getParaLevel` is the first paragraph's level.
    let paragraph_level = info
      .paragraphs
      .first()
      .map_or(0, |paragraph| paragraph.level.number());
    let bidi_class = CodePointMapData::<BidiClass>::new();
    let classes: Vec<BidiClass> = text.chars().map(|c| bidi_class.get(c)).collect();
    let mut levels: Vec<u8> = text
      .char_indices()
      .map(|(offset, _)| info.levels[offset].number())
      .collect();

    adjust_ws_levels(&classes, &mut levels, paragraph_level);
    self.paragraph_level = paragraph_level;
    self.runs.clear();

    for ((offset, character), level) in text.char_indices().zip(levels) {
      let end = offset + character.len_utf8();

      match self.runs.last_mut() {
        Some(run) if run.level == level => run.end = end,
        _ => self.runs.push(Run {
          start: offset,
          end,
          level,
        }),
      }
    }

    self.base_direction = base_direction.unwrap_or(direction_from_level(paragraph_level));

    true
  }

  /// Blink's `IsUnidirectional`: every level runs the paragraph's way. Approximate: ICU's
  /// `ubidi_getDirection` also calls Arabic digits among neutrals mixed.
  pub(crate) fn is_unidirectional(&self) -> bool {
    self
      .runs
      .iter()
      .all(|run| run.level & 1 == self.paragraph_level & 1)
  }

  pub(crate) fn base_direction(&self) -> TextDirection {
    self.base_direction
  }

  /// Blink's `GetLogicalRun`: the end of the run of one level that `start` is in, and its level.
  pub(crate) fn get_logical_run(&self, start: usize) -> (usize, u8) {
    let run = &self.runs[self.runs.partition_point(|run| run.end <= start)];

    (run.end, run.level)
  }

  /// Blink's `GetVisualRuns`.
  pub(crate) fn get_visual_runs(&self) -> Vec<Run> {
    let levels: Vec<u8> = self.runs.iter().map(|run| run.level).collect();

    Self::indices_in_visual_order(&levels)
      .into_iter()
      .map(|index| self.runs[index])
      .collect()
  }

  /// Blink's `IndicesInVisualOrder`, after ICU's `ubidi_reorderVisual`: UAX #9 rule L2 reverses
  /// every sequence at each level, from the highest down to the lowest odd one.
  /// <https://unicode.org/reports/tr9/#L2>
  pub(crate) fn indices_in_visual_order(levels: &[u8]) -> Vec<usize> {
    let mut indices: Vec<usize> = (0..levels.len()).collect();
    let Some(&highest) = levels.iter().max() else {
      return indices;
    };
    let lowest_odd = levels.iter().copied().min().unwrap_or(0) | 1;

    for level in (lowest_odd..=highest).rev() {
      let mut start = 0;

      while start < indices.len() {
        if levels[indices[start]] < level {
          start += 1;

          continue;
        }

        let end = indices[start..]
          .iter()
          .position(|&index| levels[index] < level)
          .map_or(indices.len(), |length| start + length);

        indices[start..end].reverse();
        start = end;
      }
    }

    indices
  }

  /// Blink's `BaseDirectionForString`: the direction of the first strong character before one
  /// `stop_at` accepts.
  pub(crate) fn base_direction_for_string(
    text: &str,
    stop_at: Option<fn(char) -> bool>,
  ) -> Option<TextDirection> {
    let bidi_class = CodePointMapData::<BidiClass>::new();

    for character in text.chars() {
      match bidi_class.get(character) {
        BidiClass::LeftToRight => return Some(TextDirection::Ltr),
        BidiClass::RightToLeft | BidiClass::ArabicLetter => return Some(TextDirection::Rtl),
        _ => {}
      }

      if stop_at.is_some_and(|stop_at| stop_at(character)) {
        break;
      }
    }

    None
  }
}

/// ICU's `adjustWSLevels` (UAX #9 rule L1, and X9's removed characters): white space before a
/// separator or the end takes the paragraph level, and a removed character the level after it.
fn adjust_ws_levels(classes: &[BidiClass], levels: &mut [u8], paragraph_level: u8) {
  let is_explicit_or_boundary_neutral = |class: BidiClass| {
    matches!(
      class,
      BidiClass::BoundaryNeutral
        | BidiClass::LeftToRightEmbedding
        | BidiClass::RightToLeftEmbedding
        | BidiClass::LeftToRightOverride
        | BidiClass::RightToLeftOverride
        | BidiClass::PopDirectionalFormat
    )
  };
  let is_separator = |class: BidiClass| {
    matches!(
      class,
      BidiClass::ParagraphSeparator | BidiClass::SegmentSeparator
    )
  };
  let is_white_space = |class: BidiClass| {
    is_separator(class)
      || is_explicit_or_boundary_neutral(class)
      || matches!(
        class,
        BidiClass::WhiteSpace
          | BidiClass::LeftToRightIsolate
          | BidiClass::RightToLeftIsolate
          | BidiClass::FirstStrongIsolate
          | BidiClass::PopDirectionalIsolate
      )
  };
  let mut i = levels.len();

  while i > 0 {
    while i > 0 {
      i -= 1;

      if !is_white_space(classes[i]) {
        break;
      }

      levels[i] = paragraph_level;
    }

    while i > 0 {
      i -= 1;

      if is_explicit_or_boundary_neutral(classes[i]) {
        levels[i] = levels[i + 1];
      } else if is_separator(classes[i]) {
        levels[i] = paragraph_level;

        break;
      }
    }
  }
}

#[cfg(test)]
mod tests {
  use super::{BidiParagraph, Run};
  use crate::style::Direction as TextDirection;

  fn is_line_feed(character: char) -> bool {
    character == '\n'
  }

  impl BidiParagraph {
    /// Blink's `GetLogicalRuns`.
    fn get_logical_runs(&self) -> &[Run] {
      &self.runs
    }
  }

  #[test]
  fn set_paragraph_heuristic_ltr() {
    let bidi = BidiParagraph::new("abc", None);

    assert_eq!(bidi.base_direction(), TextDirection::Ltr);
  }

  #[test]
  fn set_paragraph_heuristic_rtl() {
    let bidi = BidiParagraph::new("\u{05D0}\u{05D1}\u{05D2}", None);

    assert_eq!(bidi.base_direction(), TextDirection::Rtl);
  }

  #[test]
  fn get_logical_runs() {
    let text = "\u{05D0}\u{05D1}\u{05D2} abc \u{05D3}\u{05D4}\u{05D5}";
    let bidi = BidiParagraph::new(text, Some(TextDirection::Rtl));

    assert_eq!(
      bidi.get_logical_runs(),
      [
        Run {
          start: 0,
          end: 7,
          level: 1
        },
        Run {
          start: 7,
          end: 10,
          level: 2
        },
        Run {
          start: 10,
          end: 17,
          level: 1
        },
      ]
    );
    assert_eq!(bidi.get_logical_run(8), (10, 2));
    assert!(!bidi.is_unidirectional());
  }

  #[test]
  fn get_visual_runs() {
    let text = "\u{05D0}\u{05D1}\u{05D2} abc \u{05D3}\u{05D4}\u{05D5}";
    let bidi = BidiParagraph::new(text, Some(TextDirection::Rtl));
    let starts: Vec<usize> = bidi.get_visual_runs().iter().map(|run| run.start).collect();

    assert_eq!(starts, [10, 7, 0]);
  }

  #[test]
  fn indices_in_visual_order() {
    assert_eq!(
      BidiParagraph::indices_in_visual_order(&[0, 1, 1, 2, 1]),
      [0, 4, 3, 2, 1]
    );
    assert_eq!(
      BidiParagraph::indices_in_visual_order(&[2, 2, 1]),
      [2, 0, 1]
    );
    assert_eq!(BidiParagraph::indices_in_visual_order(&[2, 2]), [0, 1]);
  }

  #[test]
  fn base_direction_for_string() {
    let cases: [(&str, Option<TextDirection>, Option<TextDirection>); 11] = [
      ("A", Some(TextDirection::Ltr), None),
      ("\u{05D0}", Some(TextDirection::Rtl), None),
      ("!", None, None),
      ("\u{10B15}", Some(TextDirection::Rtl), None),
      ("\u{1F62D}", None, None),
      ("!A", Some(TextDirection::Ltr), None),
      ("!A\u{05D0}", Some(TextDirection::Ltr), None),
      ("!\u{05D0}Z", Some(TextDirection::Rtl), None),
      ("!\nA", None, Some(TextDirection::Ltr)),
      ("!\nA\u{05D0}", None, Some(TextDirection::Ltr)),
      ("!\n\u{05D0}Z", None, Some(TextDirection::Rtl)),
    ];

    for (text, direction_line_feed, direction_no_stop) in cases {
      assert_eq!(
        BidiParagraph::base_direction_for_string(text, Some(is_line_feed)),
        direction_line_feed,
        "{text:?}"
      );
      assert_eq!(
        BidiParagraph::base_direction_for_string(text, None),
        direction_no_stop.or(direction_line_feed),
        "{text:?}"
      );
    }
  }
}
