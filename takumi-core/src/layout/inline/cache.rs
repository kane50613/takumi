use std::{
  cell::RefCell,
  collections::{HashMap, VecDeque},
  mem::take,
  rc::Rc,
};

use super::{InlineLayout, InlineMeasurement, ShapedRun};

type ShapedText = (InlineLayout, String);
pub(crate) type ShapeCache = Rc<RefCell<HashMap<u64, ShapedText>>>;
pub(crate) type MeasureCache = Rc<RefCell<HashMap<(u64, u32), InlineMeasurement>>>;

/// Render-local text shaping and measurement reuse.
#[derive(Clone, Default)]
pub(crate) struct InlineLayoutCache {
  shapes: ShapeCache,
  /// The box each shape was laid out for, or `None` once a second box reused it.
  owners: Rc<RefCell<HashMap<u64, Option<usize>>>>,
  /// The faces runs last drew in, which a run of the next boxes drawing in an equal one shares.
  faces: Rc<RefCell<VecDeque<Rc<ShapedRun>>>>,
  measurements: MeasureCache,
}

impl InlineLayoutCache {
  pub(crate) fn new(shapes: ShapeCache, measurements: MeasureCache) -> Self {
    Self {
      shapes,
      owners: Rc::default(),
      faces: Rc::default(),
      measurements,
    }
  }

  /// The text `key` names shaped for the box `owner`, shaped now when no box shaped it before.
  pub(crate) fn get_or_shape(
    &self,
    key: Option<(u64, &str)>,
    owner: usize,
    shape: impl FnOnce() -> ShapedText,
  ) -> ShapedText {
    if let Some((fingerprint, expected_text)) = key
      && let Some((layout, text)) = self.shapes.borrow().get(&fingerprint)
      && text == expected_text
    {
      let mut owners = self.owners.borrow_mut();
      let first = owners.entry(fingerprint).or_insert(Some(owner));

      if *first != Some(owner) {
        *first = None;
      }
      return (layout.clone(), text.clone());
    }

    let shaped = shape();
    if let Some((fingerprint, _)) = key {
      self.shapes.borrow_mut().insert(fingerprint, shaped.clone());
      self.owners.borrow_mut().insert(fingerprint, Some(owner));
    }
    shaped
  }

  /// Drops every shaped text, which layout keeps to break the same text again, and the room the
  /// maps grew to hold it.
  pub(crate) fn clear_shapes(&self) {
    take(&mut *self.shapes.borrow_mut());
    take(&mut *self.owners.borrow_mut());
  }

  /// Drops the shaped text at `fingerprint` unless a second box laid it out: the box it was
  /// shaped for keeps its fragment items now.
  pub(crate) fn release_unshared(&self, fingerprint: u64) {
    let mut owners = self.owners.borrow_mut();

    if owners.get(&fingerprint).is_some_and(Option::is_some) {
      owners.remove(&fingerprint);
      self.shapes.borrow_mut().remove(&fingerprint);
    }
  }

  /// How many shaped texts it keeps.
  #[cfg(test)]
  pub(crate) fn shape_count(&self) -> usize {
    self.shapes.borrow().len()
  }

  /// `face`, or an equal face a run laid out lately drew in.
  pub(crate) fn share_face(&self, face: ShapedRun) -> Rc<ShapedRun> {
    const RECENT_FACES: usize = 8;

    let mut faces = self.faces.borrow_mut();

    if let Some(shared) = faces.iter().find(|shared| shared.same_face(&face)) {
      return Rc::clone(shared);
    }
    let face = Rc::new(face);

    if faces.len() == RECENT_FACES {
      faces.pop_front();
    }
    faces.push_back(Rc::clone(&face));
    face
  }

  pub(crate) fn get_or_measure(
    &self,
    key: (u64, u32),
    measure: impl FnOnce() -> InlineMeasurement,
  ) -> InlineMeasurement {
    if let Some(size) = self.measurements.borrow().get(&key) {
      return *size;
    }
    let size = measure();
    self.measurements.borrow_mut().insert(key, size);
    size
  }
}

#[cfg(test)]
mod tests {
  use std::cell::Cell;

  use super::*;
  use crate::geometry::Size;

  #[test]
  fn shaping_retains_first_sight_and_checks_text() {
    let cache = InlineLayoutCache::default();
    let calls = Cell::new(0);
    let shape = || {
      calls.set(calls.get() + 1);
      (InlineLayout::new(), "hello".to_owned())
    };
    for _ in 0..3 {
      cache.get_or_shape(Some((1, "hello")), 0, shape);
    }
    assert_eq!(calls.get(), 1);
    cache.clone().get_or_shape(Some((1, "hello")), 0, shape);
    assert_eq!(calls.get(), 1);

    let (_, text) = cache.get_or_shape(Some((1, "other")), 0, || {
      (InlineLayout::new(), "other".to_owned())
    });
    assert_eq!(text, "other");
    InlineLayoutCache::default().get_or_shape(Some((1, "hello")), 0, shape);
    assert_eq!(calls.get(), 2);
  }

  #[test]
  fn uncacheable_shapes_do_not_create_entries() {
    let cache = InlineLayoutCache::default();
    cache.get_or_shape(None, 0, || (InlineLayout::new(), "hello".to_owned()));
    assert!(cache.shapes.borrow().is_empty());
  }

  #[test]
  fn measurements_share_only_with_clones_and_matching_inputs() {
    let cache = InlineLayoutCache::default();
    let first = InlineMeasurement {
      size: Size {
        width: 10.0,
        height: 20.0,
      },
      first_baseline: Some(8.0),
      last_baseline: Some(18.0),
      clamped: false,
    };
    let second = InlineMeasurement {
      size: Size {
        width: 30.0,
        height: 40.0,
      },
      first_baseline: None,
      last_baseline: None,
      clamped: true,
    };
    assert_eq!(cache.get_or_measure((1, 5), || first), first);
    assert_eq!(cache.clone().get_or_measure((1, 5), || second), first);
    assert_eq!(cache.get_or_measure((1, 6), || second), second);
    assert_eq!(
      InlineLayoutCache::default().get_or_measure((1, 5), || second),
      second
    );
  }
}
