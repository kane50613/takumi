//! Blink's [`ExclusionSpace`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/layout/exclusions/exclusion_space.cc),
//! without `shape-outside`, initial letters, block fragmentation, or the sharing of exclusions
//! between copies.

use std::cell::RefCell;

use taffy::{Clear, Float};

use super::{exclusion_area::ExclusionArea, layout_opportunity::LayoutOpportunity};
use crate::{
  layout::bfc_rect::{BfcOffset, BfcRect},
  layout_unit::LayoutUnit,
  style::Direction as TextDirection,
};

/// Blink's `ShelfEdge`: a float's side against a shelf's edge.
#[derive(Debug, Clone, Copy, PartialEq)]
struct ShelfEdge {
  block_start: LayoutUnit,
  block_end: LayoutUnit,
}

/// Blink's `Shelf`: the block end of a float, where the space beside the floats above it starts.
#[derive(Debug, Clone, PartialEq)]
struct Shelf {
  block_offset: LayoutUnit,
  line_left: LayoutUnit,
  line_right: LayoutUnit,
  /// The floats against the shelf's line-left edge.
  line_left_edges: Vec<ShelfEdge>,
  /// The floats against the shelf's line-right edge.
  line_right_edges: Vec<ShelfEdge>,
}

impl Shelf {
  fn new(block_offset: LayoutUnit) -> Self {
    Self {
      block_offset,
      line_left: LayoutUnit::MIN,
      line_right: LayoutUnit::MAX,
      line_left_edges: Vec::new(),
      line_right_edges: Vec::new(),
    }
  }
}

/// Blink's `ClosedArea`: an area above a float, which floats aligning their block starts never
/// change once closed off.
#[derive(Debug, Clone, PartialEq)]
struct ClosedArea {
  opportunity: LayoutOpportunity,
  line_left_edges: Vec<ShelfEdge>,
  line_right_edges: Vec<ShelfEdge>,
}

/// Blink's `InsertClosedArea`: by block start, then by shrinking inline size.
fn insert_closed_area(area: ClosedArea, areas: &mut Vec<ClosedArea>) {
  // Most areas go at the end.
  let block_start = area.opportunity.rect.block_start_offset();

  match areas
    .iter()
    .rposition(|other| other.opportunity.rect.block_start_offset() <= block_start)
  {
    Some(index) => areas.insert(index + 1, area),
    None => {
      // A float at the minimum offset can leave an area there after the first one.
      debug_assert!(areas.is_empty() || block_start == LayoutUnit::MIN);

      areas.insert(0, area);
    }
  }
}

/// Blink's `HasSolidEdges`: at least one float against the edge between `block_start` and
/// `block_end`. The initial shelf's edges, with no floats, are always solid.
fn has_solid_edges(edges: &[ShelfEdge], block_start: LayoutUnit, block_end: LayoutUnit) -> bool {
  edges.is_empty()
    || edges
      .iter()
      .any(|edge| edge.block_end > block_start && edge.block_start < block_end)
}

/// Blink's `CollectSolidEdges`: the edges below `block_offset`.
fn collect_solid_edges(edges: &[ShelfEdge], block_offset: LayoutUnit) -> Vec<ShelfEdge> {
  edges
    .iter()
    .filter(|edge| edge.block_end > block_offset)
    .copied()
    .collect()
}

/// Blink's `Intersects`: whether the opportunity meets the area from `offset`, `inline_size`
/// wide and endlessly tall.
fn intersects(opportunity: &LayoutOpportunity, offset: BfcOffset, inline_size: LayoutUnit) -> bool {
  opportunity.rect.line_end_offset() >= offset.line_offset
    && opportunity.rect.line_start_offset() <= offset.line_offset + inline_size
    && opportunity.rect.block_end_offset() > offset.block_offset
}

/// Blink's `CreateLayoutOpportunity` from a closed area's opportunity meeting the area.
fn create_layout_opportunity_from_area(
  other: &LayoutOpportunity,
  offset: BfcOffset,
  inline_size: LayoutUnit,
) -> LayoutOpportunity {
  debug_assert!(intersects(other, offset, inline_size));

  let start_offset = BfcOffset::new(
    other.rect.line_start_offset().max(offset.line_offset),
    other
      .rect
      .start_offset
      .block_offset
      .max(offset.block_offset),
  );
  let end_offset = BfcOffset::new(
    other
      .rect
      .line_end_offset()
      .min(offset.line_offset + inline_size),
    other.rect.block_end_offset(),
  );

  LayoutOpportunity::new(BfcRect::new(start_offset, end_offset))
}

/// Blink's `CreateLayoutOpportunity` from a shelf meeting the area.
fn create_layout_opportunity_from_shelf(
  shelf: &Shelf,
  offset: BfcOffset,
  inline_size: LayoutUnit,
  direction: TextDirection,
) -> LayoutOpportunity {
  let mut line_left = shelf.line_left.max(offset.line_offset);
  let mut line_right = shelf.line_right.min(offset.line_offset + inline_size);

  // A negative inline size collapses to the dominant side.
  match direction {
    TextDirection::Ltr => line_right = line_right.max(line_left),
    TextDirection::Rtl => line_left = line_left.min(line_right),
  }

  LayoutOpportunity::new(BfcRect::new(
    BfcOffset::new(line_left, shelf.block_offset.max(offset.block_offset)),
    BfcOffset::new(line_right, LayoutUnit::MAX),
  ))
}

/// Blink's `DerivedGeometry`: the shelves and closed areas queries walk, rebuilt from the
/// exclusions when needed.
#[derive(Debug, Clone)]
struct DerivedGeometry {
  /// Always one at the minimum offset first.
  shelves: Vec<Shelf>,
  areas: Vec<ClosedArea>,
  /// The highest block offset the geometry is valid for.
  block_offset_limit: LayoutUnit,
}

impl DerivedGeometry {
  fn new(block_offset_limit: LayoutUnit) -> Self {
    Self {
      shelves: vec![Shelf::new(LayoutUnit::MIN)],
      areas: Vec::new(),
      block_offset_limit,
    }
  }

  /// Blink's `DerivedGeometry::Add`: the shelves and areas `exclusion` changes.
  fn add(&mut self, exclusion: &ExclusionArea) {
    debug_assert!(exclusion.rect.block_start_offset() >= self.block_offset_limit);

    // An exclusion without inline size only moves later floats down.
    if exclusion.rect.line_end_offset() <= exclusion.rect.line_start_offset() {
      return;
    }

    let exclusion_end_offset = exclusion.rect.block_end_offset();
    let mut i = 0;

    while i < self.shelves.len() {
      // A new shelf between two others, which needs this one as it was.
      let is_between_shelves = exclusion_end_offset >= self.shelves[i].block_offset
        && (i + 1 >= self.shelves.len() || exclusion_end_offset < self.shelves[i + 1].block_offset);
      let shelf_copy = is_between_shelves.then(|| self.shelves[i].clone());
      let mut removed = false;

      {
        let shelf_block_offset = self.shelves[i].block_offset;
        let is_below = exclusion.rect.block_start_offset() > shelf_block_offset;

        if is_below {
          let shelf = &self.shelves[i];
          // An area closes off only between solid edges, with floats against both sides.
          let solid = has_solid_edges(
            &shelf.line_left_edges,
            shelf.block_offset,
            exclusion.rect.block_start_offset(),
          ) && has_solid_edges(
            &shelf.line_right_edges,
            shelf.block_offset,
            exclusion.rect.block_start_offset(),
          );
          // A right exclusion against the shelf's right edge closes nothing; a left exclusion
          // against it does.
          let is_overlapping = match exclusion.float_type {
            Float::Left => {
              exclusion.rect.line_start_offset() <= shelf.line_right
                && exclusion.rect.line_end_offset() > shelf.line_left
            }
            _ => {
              exclusion.rect.line_start_offset() < shelf.line_right
                && exclusion.rect.line_end_offset() >= shelf.line_left
            }
          };

          if solid && is_overlapping {
            let opportunity = LayoutOpportunity::new(BfcRect::new(
              BfcOffset::new(shelf.line_left, shelf.block_offset),
              BfcOffset::new(shelf.line_right, exclusion.rect.block_start_offset()),
            ));

            insert_closed_area(
              ClosedArea {
                opportunity,
                line_left_edges: shelf.line_left_edges.clone(),
                line_right_edges: shelf.line_right_edges.clone(),
              },
              &mut self.areas,
            );
          }
        }

        // The exclusion cuts across the shelf.
        let is_intersecting = !is_below && exclusion_end_offset > shelf_block_offset;

        if is_below || is_intersecting {
          let shelf = &mut self.shelves[i];
          let edge = ShelfEdge {
            block_start: exclusion.rect.block_start_offset(),
            block_end: exclusion.rect.block_end_offset(),
          };

          match exclusion.float_type {
            Float::Left => {
              if exclusion.rect.line_end_offset() >= shelf.line_left {
                // Pushing the edge in clears its floats.
                if exclusion.rect.line_end_offset() > shelf.line_left {
                  shelf.line_left_edges.clear();
                }

                shelf.line_left = exclusion.rect.line_end_offset();
                shelf.line_left_edges.push(edge);
              }
            }
            _ => {
              if exclusion.rect.line_start_offset() <= shelf.line_right {
                if exclusion.rect.line_start_offset() < shelf.line_right {
                  shelf.line_right_edges.clear();
                }

                shelf.line_right = exclusion.rect.line_start_offset();
                shelf.line_right_edges.push(edge);
              }
            }
          }

          // A shelf closed off entirely, or the same as the one before, goes.
          let is_closed_off = shelf.line_left > shelf.line_right;
          let is_same_as_previous = i > 0
            && self.shelves[i].line_left == self.shelves[i - 1].line_left
            && self.shelves[i].line_right == self.shelves[i - 1].line_right;

          if is_closed_off || is_same_as_previous {
            self.shelves.remove(i);
            removed = true;
          }
        }
      }

      if let Some(shelf_copy) = shelf_copy {
        // Only at another block offset.
        if exclusion_end_offset != shelf_copy.block_offset {
          let mut new_shelf = Shelf::new(exclusion_end_offset);

          new_shelf.line_left_edges =
            collect_solid_edges(&shelf_copy.line_left_edges, exclusion_end_offset);
          new_shelf.line_right_edges =
            collect_solid_edges(&shelf_copy.line_right_edges, exclusion_end_offset);
          // Without edges, the shelf reaches as far as it can.
          new_shelf.line_left = if new_shelf.line_left_edges.is_empty() {
            LayoutUnit::MIN
          } else {
            shelf_copy.line_left
          };
          new_shelf.line_right = if new_shelf.line_right_edges.is_empty() {
            LayoutUnit::MAX
          } else {
            shelf_copy.line_right
          };

          let insert_at = if removed { i } else { i + 1 };

          self.shelves.insert(insert_at, new_shelf);
        }

        // Later shelves stay as they are.
        break;
      }

      if !removed {
        i += 1;
      }
    }
  }

  /// Blink's `IterateAllLayoutOpportunities`: each opportunity meeting the area from `offset`,
  /// until `visit` returns true.
  fn iterate_all_layout_opportunities(
    &self,
    offset: BfcOffset,
    available_inline_size: LayoutUnit,
    direction: TextDirection,
    mut visit: impl FnMut(LayoutOpportunity) -> bool,
  ) {
    let mut shelves = self.shelves.iter().peekable();
    let mut areas = self.areas.iter().peekable();

    // The endless last shelf outlasts the areas.
    while let Some(&shelf) = shelves.peek() {
      if let Some(&area) = areas.peek() {
        if !intersects(&area.opportunity, offset, available_inline_size) {
          areas.next();

          continue;
        }

        let block_start_offset = area
          .opportunity
          .rect
          .block_start_offset()
          .max(offset.block_offset);

        // A closed area wins over a shelf at its offset.
        if block_start_offset <= shelf.block_offset.max(offset.block_offset) {
          let block_end_offset = area.opportunity.rect.block_end_offset();
          let solid = has_solid_edges(&area.line_left_edges, block_start_offset, block_end_offset)
            && has_solid_edges(&area.line_right_edges, block_start_offset, block_end_offset);

          if solid
            && visit(create_layout_opportunity_from_area(
              &area.opportunity,
              offset,
              available_inline_size,
            ))
          {
            return;
          }

          areas.next();

          continue;
        }
      }

      let solid = has_solid_edges(&shelf.line_left_edges, offset.block_offset, LayoutUnit::MAX)
        && has_solid_edges(
          &shelf.line_right_edges,
          offset.block_offset,
          LayoutUnit::MAX,
        );

      if solid
        && visit(create_layout_opportunity_from_shelf(
          shelf,
          offset,
          available_inline_size,
          direction,
        ))
      {
        return;
      }

      shelves.next();
    }
  }

  /// Blink's `DerivedGeometry::FindLayoutOpportunity`.
  fn find_layout_opportunity(
    &self,
    offset: BfcOffset,
    available_inline_size: LayoutUnit,
    direction: TextDirection,
    minimum_inline_size: LayoutUnit,
  ) -> LayoutOpportunity {
    debug_assert!(offset.block_offset >= self.block_offset_limit);

    let mut found = LayoutOpportunity::default();

    self.iterate_all_layout_opportunities(
      offset,
      available_inline_size,
      direction,
      |opportunity| {
        // An opportunity as wide as the available size fits, even below the minimum.
        if opportunity.rect.inline_size() >= minimum_inline_size
          || (opportunity.rect.inline_size() == available_inline_size
            && opportunity.rect.line_start_offset() == offset.line_offset)
        {
          found = opportunity;

          return true;
        }

        false
      },
    );

    found
  }

  /// Blink's `DerivedGeometry::AllLayoutOpportunities`.
  fn all_layout_opportunities(
    &self,
    offset: BfcOffset,
    available_inline_size: LayoutUnit,
    direction: TextDirection,
  ) -> Vec<LayoutOpportunity> {
    debug_assert!(offset.block_offset >= self.block_offset_limit);

    let mut opportunities = Vec::new();

    self.iterate_all_layout_opportunities(
      offset,
      available_inline_size,
      direction,
      |opportunity| {
        opportunities.push(opportunity);

        false
      },
    );

    opportunities
  }
}

/// Blink's `ExclusionSpace`: the floats of a block formatting context, queried for the space
/// they leave.
#[derive(Debug, Clone, Default)]
pub(crate) struct ExclusionSpace {
  /// In ascending block start order.
  exclusions: Vec<ExclusionArea>,
  /// The lowest block end of the left and right floats, for clearance.
  left_clear_offset: Option<LayoutUnit>,
  right_clear_offset: Option<LayoutUnit>,
  /// For the top edge alignment rule; floats may sit at negative offsets.
  last_float_block_start: Option<LayoutUnit>,
  derived_geometry: RefCell<Option<DerivedGeometry>>,
}

impl PartialEq for ExclusionSpace {
  fn eq(&self, other: &Self) -> bool {
    self.exclusions == other.exclusions
  }
}

impl ExclusionSpace {
  fn left_clear_offset(&self) -> LayoutUnit {
    self.left_clear_offset.unwrap_or(LayoutUnit::MIN)
  }

  fn right_clear_offset(&self) -> LayoutUnit {
    self.right_clear_offset.unwrap_or(LayoutUnit::MIN)
  }

  /// Blink's `Add`.
  pub(crate) fn add(&mut self, mut exclusion: ExclusionArea) {
    let exclusion_block_offset = exclusion.rect.block_start_offset();

    exclusion.is_past_other_exclusions = exclusion_block_offset
      >= self
        .left_clear_offset()
        .max(exclusion_block_offset)
        .max(self.right_clear_offset());

    // The offsets clearance reads.
    let clear_offset = exclusion.rect.block_end_offset();

    self.last_float_block_start = Some(self.last_float_block_start().max(exclusion_block_offset));

    match exclusion.float_type {
      Float::Left => self.left_clear_offset = Some(self.left_clear_offset().max(clear_offset)),
      Float::Right => self.right_clear_offset = Some(self.right_clear_offset().max(clear_offset)),
      Float::None => {}
    }

    if let Some(derived_geometry) = self.derived_geometry.get_mut() {
      derived_geometry.add(&exclusion);
    }

    self.exclusions.push(exclusion);
  }

  /// Blink's `FindLayoutOpportunity`: within the area from `offset`, `available_inline_size`
  /// wide, the first opportunity at least `minimum_inline_size` wide.
  pub(crate) fn find_layout_opportunity(
    &self,
    offset: BfcOffset,
    available_inline_size: LayoutUnit,
    direction: TextDirection,
    minimum_inline_size: LayoutUnit,
  ) -> LayoutOpportunity {
    // Past every float, the whole area.
    if offset.block_offset >= self.left_clear_offset().max(self.right_clear_offset()) {
      return LayoutOpportunity::new(BfcRect::new(
        offset,
        BfcOffset::new(
          offset.line_offset + available_inline_size.clamp_negative_to_zero(),
          LayoutUnit::MAX,
        ),
      ));
    }

    self.with_derived_geometry(offset.block_offset, |geometry| {
      geometry.find_layout_opportunity(
        offset,
        available_inline_size,
        direction,
        minimum_inline_size,
      )
    })
  }

  /// Blink's `AllLayoutOpportunities`.
  pub(crate) fn all_layout_opportunities(
    &self,
    offset: BfcOffset,
    available_inline_size: LayoutUnit,
    direction: TextDirection,
  ) -> Vec<LayoutOpportunity> {
    if offset.block_offset >= self.left_clear_offset().max(self.right_clear_offset()) {
      return vec![LayoutOpportunity::new(BfcRect::new(
        offset,
        BfcOffset::new(
          offset.line_offset + available_inline_size.clamp_negative_to_zero(),
          LayoutUnit::MAX,
        ),
      ))];
    }

    self.with_derived_geometry(offset.block_offset, |geometry| {
      geometry.all_layout_opportunities(offset, available_inline_size, direction)
    })
  }

  /// Blink's `ClearanceOffset`.
  pub(crate) fn clearance_offset(&self, clear_type: Clear) -> LayoutUnit {
    match clear_type {
      Clear::None => LayoutUnit::MIN,
      Clear::Left => self.left_clear_offset(),
      Clear::Right => self.right_clear_offset(),
      Clear::Both => self.left_clear_offset().max(self.right_clear_offset()),
    }
  }

  /// Blink's `LastFloatBlockStart`.
  pub(crate) fn last_float_block_start(&self) -> LayoutUnit {
    self.last_float_block_start.unwrap_or(LayoutUnit::MIN)
  }

  /// Blink's `GetDerivedGeometry`: the geometry valid from `block_offset_limit`, rebuilt from the
  /// highest exclusion past the others above it.
  fn with_derived_geometry<R>(
    &self,
    block_offset_limit: LayoutUnit,
    query: impl FnOnce(&DerivedGeometry) -> R,
  ) -> R {
    let mut derived_geometry = self.derived_geometry.borrow_mut();

    // Built for a higher limit, it does not cover this one.
    if derived_geometry
      .as_ref()
      .is_some_and(|geometry| block_offset_limit < geometry.block_offset_limit)
    {
      *derived_geometry = None;
    }

    let geometry = derived_geometry.get_or_insert_with(|| {
      debug_assert!(!self.exclusions.is_empty());

      // The first exclusion starting at or after the limit.
      let mut index = self
        .exclusions
        .partition_point(|exclusion| exclusion.rect.block_start_offset() < block_offset_limit);
      let limit = if index == 0 {
        LayoutUnit::MIN
      } else {
        // The highest exclusion past the others.
        loop {
          index -= 1;

          if index == 0 || self.exclusions[index].is_past_other_exclusions {
            break;
          }
        }

        debug_assert!(self.exclusions[index].rect.block_start_offset() <= block_offset_limit);

        self.exclusions[index].rect.block_start_offset()
      };
      let mut geometry = DerivedGeometry::new(limit);

      for exclusion in &self.exclusions[index..] {
        geometry.add(exclusion);
      }

      geometry
    });

    query(geometry)
  }
}

#[cfg(test)]
mod tests {
  use taffy::Float;

  use super::ExclusionSpace;
  use crate::{
    layout::{
      bfc_rect::{BfcOffset, BfcRect},
      exclusions::{exclusion_area::ExclusionArea, layout_opportunity::LayoutOpportunity},
    },
    layout_unit::LayoutUnit,
    style::Direction as TextDirection,
  };

  const MAX: i32 = i32::MAX;

  fn unit(value: i32) -> LayoutUnit {
    if value == MAX {
      LayoutUnit::MAX
    } else {
      LayoutUnit::from_int(value)
    }
  }

  fn offset(line: i32, block: i32) -> BfcOffset {
    BfcOffset::new(unit(line), unit(block))
  }

  fn add(space: &mut ExclusionSpace, start: (i32, i32), end: (i32, i32), float_type: Float) {
    space.add(ExclusionArea::new(
      BfcRect::new(offset(start.0, start.1), offset(end.0, end.1)),
      float_type,
    ));
  }

  /// Each opportunity's start and end, as Blink's `TEST_OPPORTUNITY` compares them.
  fn all(
    space: &ExclusionSpace,
    start: (i32, i32),
    available_size: i32,
    direction: TextDirection,
  ) -> Vec<((i32, i32), (i32, i32))> {
    let raw = |unit: LayoutUnit| {
      if unit == LayoutUnit::MAX {
        MAX
      } else {
        unit.to_int()
      }
    };

    space
      .all_layout_opportunities(offset(start.0, start.1), unit(available_size), direction)
      .into_iter()
      .map(|opportunity| {
        let rect = opportunity.rect;

        (
          (
            raw(rect.start_offset.line_offset),
            raw(rect.start_offset.block_offset),
          ),
          (
            raw(rect.end_offset.line_offset),
            raw(rect.end_offset.block_offset),
          ),
        )
      })
      .collect()
  }

  #[test]
  fn empty() {
    let space = ExclusionSpace::default();

    assert_eq!(
      all(&space, (0, 0), 100, TextDirection::Ltr),
      [((0, 0), (100, MAX))]
    );
    assert_eq!(
      all(&space, (-30, -100), 50, TextDirection::Ltr),
      [((-30, -100), (20, MAX))]
    );
    assert_eq!(
      all(&space, (30, 100), 50, TextDirection::Ltr),
      [((30, 100), (80, MAX))]
    );
  }

  #[test]
  fn line_layout_opportunity_clamps_non_dominant_side() {
    let opportunity = |start: (i32, i32), end: (i32, i32)| {
      LayoutOpportunity::new(BfcRect::new(offset(start.0, start.1), offset(end.0, end.1)))
    };
    let line = |opportunity: LayoutOpportunity, direction| {
      let line = opportunity.compute_line_layout_opportunity(
        unit(10),
        unit(100),
        direction,
        unit(10),
        LayoutUnit::ZERO,
      );

      (
        line.line_left_offset.to_int(),
        line.line_right_offset.to_int(),
      )
    };

    assert_eq!(
      line(opportunity((20, 0), (150, 100)), TextDirection::Ltr),
      (20, 110)
    );
    assert_eq!(
      line(opportunity((120, 0), (150, 100)), TextDirection::Ltr),
      (120, 120)
    );
    assert_eq!(
      line(opportunity((-50, 0), (100, 100)), TextDirection::Rtl),
      (10, 100)
    );
    assert_eq!(
      line(opportunity((-50, 0), (0, 100)), TextDirection::Rtl),
      (0, 0)
    );
  }

  #[test]
  fn single_exclusion() {
    let mut space = ExclusionSpace::default();

    add(&mut space, (10, 20), (60, 90), Float::Left);

    assert_eq!(
      all(&space, (0, 0), 100, TextDirection::Ltr),
      [
        ((0, 0), (100, 20)),
        ((60, 0), (100, MAX)),
        ((0, 90), (100, MAX))
      ]
    );
    assert_eq!(
      all(&space, (-10, -100), 100, TextDirection::Ltr),
      [
        ((-10, -100), (90, 20)),
        ((60, -100), (90, MAX)),
        ((-10, 90), (90, MAX))
      ]
    );
    // A zero-width opportunity on the right.
    assert_eq!(
      all(&space, (10, 10), 50, TextDirection::Ltr),
      [
        ((10, 10), (60, 20)),
        ((60, 10), (60, MAX)),
        ((10, 90), (60, MAX))
      ]
    );
    // The right opportunity outside the area is zero-width too.
    assert_eq!(
      all(&space, (10, 10), 49, TextDirection::Ltr),
      [
        ((10, 10), (59, 20)),
        ((60, 10), (60, MAX)),
        ((10, 90), (59, MAX))
      ]
    );
  }

  #[test]
  fn two_exclusions() {
    let mut space = ExclusionSpace::default();

    add(&mut space, (0, 0), (150, 75), Float::Left);
    add(&mut space, (100, 75), (400, 150), Float::Right);

    assert_eq!(
      all(&space, (0, 0), 400, TextDirection::Ltr),
      [
        ((150, 0), (400, 75)),
        ((0, 75), (100, MAX)),
        ((0, 150), (400, MAX))
      ]
    );
  }

  /// No opportunity above the last float, whose left edge has no float against it.
  #[test]
  fn solid_edges() {
    let mut space = ExclusionSpace::default();

    add(&mut space, (0, 0), (20, 15), Float::Left);
    add(&mut space, (65, 0), (85, 15), Float::Right);
    add(&mut space, (10, 25), (30, 40), Float::Left);
    add(&mut space, (50, 25), (70, 35), Float::Right);

    assert_eq!(
      all(&space, (0, 0), 80, TextDirection::Ltr),
      [
        ((20, 0), (65, 25)),
        ((30, 0), (50, MAX)),
        ((0, 15), (80, 25)),
        ((30, 35), (80, MAX)),
        ((0, 40), (80, MAX)),
      ]
    );
  }

  /// An exclusion past the shelf's extent closes no area.
  #[test]
  fn overlapping_with_shelf() {
    let mut space = ExclusionSpace::default();

    add(&mut space, (0, 0), (20, 15), Float::Left);
    add(&mut space, (65, 0), (85, 15), Float::Right);
    add(&mut space, (70, 25), (90, 35), Float::Right);

    assert_eq!(
      all(&space, (0, 0), 80, TextDirection::Ltr),
      [
        ((20, 0), (65, MAX)),
        ((0, 15), (80, 25)),
        ((0, 15), (70, MAX)),
        ((0, 35), (80, MAX)),
      ]
    );
  }

  /// Opportunities of the same size repeat, which those reading them tolerate.
  #[test]
  fn insert_between_shelves() {
    let mut space = ExclusionSpace::default();

    add(&mut space, (0, 0), (30, 10), Float::Left);
    add(&mut space, (65, 0), (85, 35), Float::Right);
    add(&mut space, (10, 15), (30, 25), Float::Left);

    assert_eq!(
      all(&space, (30, 15), 30, TextDirection::Ltr),
      [
        ((30, 15), (60, MAX)),
        ((30, 25), (60, MAX)),
        ((30, 35), (60, MAX))
      ]
    );
  }

  #[test]
  fn zero_inline_size_opportunity() {
    let mut space = ExclusionSpace::default();

    add(&mut space, (0, 0), (100, 10), Float::Left);

    assert_eq!(
      all(&space, (0, 0), 100, TextDirection::Ltr),
      [((100, 0), (100, MAX)), ((0, 10), (100, MAX))]
    );
  }

  #[test]
  fn negative_inline_size_opportunity_left() {
    let mut space = ExclusionSpace::default();

    add(&mut space, (0, 0), (120, 10), Float::Left);

    assert_eq!(
      all(&space, (0, 0), 100, TextDirection::Ltr),
      [((120, 0), (120, MAX)), ((0, 10), (100, MAX))]
    );
    assert_eq!(
      all(&space, (0, 0), 100, TextDirection::Rtl),
      [((100, 0), (100, MAX)), ((0, 10), (100, MAX))]
    );
  }

  #[test]
  fn negative_inline_size_opportunity_right() {
    let mut space = ExclusionSpace::default();

    add(&mut space, (-20, 0), (100, 10), Float::Right);

    assert_eq!(
      all(&space, (0, 0), 100, TextDirection::Ltr),
      [((0, 0), (0, MAX)), ((0, 10), (100, MAX))]
    );
    assert_eq!(
      all(&space, (0, 0), 100, TextDirection::Rtl),
      [((-20, 0), (-20, MAX)), ((0, 10), (100, MAX))]
    );
  }
}
