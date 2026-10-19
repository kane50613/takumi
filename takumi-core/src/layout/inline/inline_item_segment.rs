//! Blink's [`InlineItemSegments`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/layout/inline/inline_item_segment.cc),
//! without font orientation.

use icu_properties::props::Script;

use super::inline_item::{InlineItem, InlineItems};
use crate::fonts::{
  font_fallback_priority::FontFallbackPriority,
  shaping::run_segmenter::{RunSegmenter, RunSegmenterRange},
};

/// Blink's `InlineItemSegment`: where a run of one script and fallback priority ends.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct InlineItemSegment {
  end_offset: usize,
  segment_data: (Script, FontFallbackPriority),
}

impl InlineItemSegment {
  fn new(range: &RunSegmenterRange) -> Self {
    Self {
      end_offset: range.end,
      segment_data: (range.script, range.font_fallback_priority),
    }
  }

  /// Blink's `ToRunSegmenterRange`.
  fn to_run_segmenter_range(self, start_offset: usize, end_offset: usize) -> RunSegmenterRange {
    debug_assert!(start_offset < end_offset);
    debug_assert!(start_offset < self.end_offset);

    let (script, font_fallback_priority) = self.segment_data;

    RunSegmenterRange {
      start: start_offset,
      end: end_offset.min(self.end_offset),
      script,
      font_fallback_priority,
    }
  }
}

/// Blink's `InlineItemSegments`: the runs of a text content that has more than one.
#[derive(Debug, Clone, Default)]
pub(crate) struct InlineItemSegments {
  segments: Vec<InlineItemSegment>,
  /// The segment each item starts in.
  items_to_segments: Vec<usize>,
}

impl InlineItemSegments {
  /// Blink's `ComputeSegments`: `range` and every range after it in `segmenter`.
  pub(crate) fn compute_segments(
    segmenter: &mut RunSegmenter<'_>,
    range: RunSegmenterRange,
  ) -> Self {
    let segments = std::iter::once(range)
      .chain(std::iter::from_fn(|| segmenter.consume()))
      .map(|range| InlineItemSegment::new(&range))
      .collect();

    Self {
      segments,
      items_to_segments: Vec::new(),
    }
  }

  pub(crate) fn end_offset(&self) -> usize {
    self.segments.last().map_or(0, |segment| segment.end_offset)
  }

  /// Blink's `ComputeItemIndex`.
  pub(crate) fn compute_item_index(&mut self, items: &InlineItems<'_>) {
    debug_assert_eq!(
      items.last().map(InlineItem::end_offset),
      Some(self.end_offset())
    );

    let mut segment_index = 0;

    self.items_to_segments.clear();

    for item in items {
      while segment_index < self.segments.len()
        && item.start_offset() >= self.segments[segment_index].end_offset
      {
        segment_index += 1;
      }

      self.items_to_segments.push(segment_index);
    }
  }

  /// Blink's `ToRanges`.
  pub(crate) fn to_ranges(&self) -> Vec<RunSegmenterRange> {
    let mut start_offset = 0;

    self
      .segments
      .iter()
      .map(|segment| {
        let range = segment.to_run_segmenter_range(start_offset, segment.end_offset);

        start_offset = segment.end_offset;
        range
      })
      .collect()
  }

  /// Blink's `Ranges`: the runs from `start_offset` to `end_offset` of item `item_index`.
  pub(crate) fn ranges(
    &self,
    start_offset: usize,
    end_offset: usize,
    item_index: usize,
  ) -> impl Iterator<Item = RunSegmenterRange> + '_ {
    debug_assert!(start_offset < end_offset);
    debug_assert!(end_offset <= self.end_offset());

    let first = self.items_to_segments[item_index];
    let last = self
      .items_to_segments
      .get(item_index + 1)
      .copied()
      .unwrap_or(self.segments.len());
    let segment_index = first
      + self.segments[first..last].partition_point(|segment| segment.end_offset <= start_offset);
    let mut start = start_offset;

    self.segments[segment_index..]
      .iter()
      .map_while(move |segment| {
        (start < end_offset).then(|| {
          let range = segment.to_run_segmenter_range(start, end_offset);

          start = range.end;
          range
        })
      })
  }
}
