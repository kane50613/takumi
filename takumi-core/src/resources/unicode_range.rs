use crate::sort_key::sort_by_key;

/// An inclusive codepoint range from an `@font-face` `unicode-range`, as Blink's `UnicodeRange`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct UnicodeRange {
  from: u32,
  to: u32,
}

impl UnicodeRange {
  /// The codepoints `from..=to`.
  pub const fn new(from: u32, to: u32) -> Self {
    Self { from, to }
  }

  fn contains(self, codepoint: u32) -> bool {
    (self.from..=self.to).contains(&codepoint)
  }
}

/// The codepoints a face may serve, as Blink's `UnicodeRangeSet`. An empty set covers every
/// codepoint.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct UnicodeRangeSet(Vec<UnicodeRange>);

impl FromIterator<UnicodeRange> for UnicodeRangeSet {
  fn from_iter<T: IntoIterator<Item = UnicodeRange>>(ranges: T) -> Self {
    let mut sorted: Vec<UnicodeRange> = ranges
      .into_iter()
      .filter(|range| range.from <= range.to)
      .collect();

    sort_by_key(&mut sorted, |range| (range.from, range.to));

    let mut merged: Vec<UnicodeRange> = Vec::with_capacity(sorted.len());

    for range in sorted {
      match merged.last_mut() {
        Some(last) if range.from <= last.to.saturating_add(1) => {
          last.to = last.to.max(range.to);
        }
        _ => merged.push(range),
      }
    }

    Self(merged)
  }
}

impl UnicodeRangeSet {
  /// Whether `codepoint` falls in the set.
  pub fn contains(&self, codepoint: u32) -> bool {
    if self.0.is_empty() {
      return true;
    }

    let index = self.0.partition_point(|range| range.to < codepoint);

    self
      .0
      .get(index)
      .is_some_and(|range| range.contains(codepoint))
  }

  /// Whether the set covers every codepoint, so the face needs no filtering.
  pub fn is_entire_range(&self) -> bool {
    self.0.is_empty() || (self.0.len() == 1 && self.0[0].from == 0 && self.0[0].to >= 0x10FFFF)
  }
}

#[cfg(test)]
mod tests {
  use super::{UnicodeRange, UnicodeRangeSet};

  #[test]
  fn ranges_merge_when_they_overlap_or_touch() {
    let set = UnicodeRangeSet::from_iter([
      UnicodeRange::new(0x30, 0x39),
      UnicodeRange::new(0x41, 0x5A),
      UnicodeRange::new(0x3A, 0x40),
      UnicodeRange::new(0x7A, 0x61),
    ]);

    assert_eq!(
      set,
      UnicodeRangeSet::from_iter([UnicodeRange::new(0x30, 0x5A)])
    );
    assert!(set.contains(0x30) && set.contains(0x5A));
    assert!(!set.contains(0x2F) && !set.contains(0x61));
  }

  #[test]
  fn an_empty_set_covers_everything() {
    let set = UnicodeRangeSet::default();

    assert!(set.contains(0x10FFFF));
    assert!(set.is_entire_range());
    assert!(UnicodeRangeSet::from_iter([UnicodeRange::new(0, 0x10FFFF)]).is_entire_range());
  }
}
