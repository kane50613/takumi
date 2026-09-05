use std::{cell::RefCell, collections::HashMap, rc::Rc};

use crate::geometry::Size;

use super::InlineLayout;

type ShapedText = (InlineLayout, String);
// None records first sight without retaining a layout clone.
pub(crate) type ShapeCache = Rc<RefCell<HashMap<u64, Option<ShapedText>>>>;
pub(crate) type MeasureCache = Rc<RefCell<HashMap<(u64, u32), Size<f32>>>>;

/// Render-local text shaping and measurement reuse.
#[derive(Clone, Default)]
pub(crate) struct InlineLayoutCache {
  shapes: ShapeCache,
  measurements: MeasureCache,
}

impl InlineLayoutCache {
  pub(crate) fn new(shapes: ShapeCache, measurements: MeasureCache) -> Self {
    Self {
      shapes,
      measurements,
    }
  }

  pub(crate) fn get_or_shape(
    &self,
    key: Option<(u64, &str)>,
    shape: impl FnOnce() -> ShapedText,
  ) -> ShapedText {
    let seen = if let Some((fingerprint, expected_text)) = key {
      match self.shapes.borrow().get(&fingerprint) {
        Some(Some((layout, text))) if text == expected_text => {
          return (layout.clone(), text.clone());
        }
        Some(_) => true,
        None => false,
      }
    } else {
      false
    };

    let shaped = shape();
    if let Some((fingerprint, _)) = key {
      self
        .shapes
        .borrow_mut()
        .insert(fingerprint, seen.then(|| shaped.clone()));
    }
    shaped
  }

  pub(crate) fn get_or_measure(
    &self,
    key: (u64, u32),
    measure: impl FnOnce() -> Size<f32>,
  ) -> Size<f32> {
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

  #[test]
  fn shaping_retains_repeated_inputs_and_checks_text() {
    let cache = InlineLayoutCache::default();
    let calls = Cell::new(0);
    let shape = || {
      calls.set(calls.get() + 1);
      (InlineLayout::new(), "hello".to_owned())
    };
    for _ in 0..3 {
      cache.get_or_shape(Some((1, "hello")), shape);
    }
    assert_eq!(calls.get(), 2);
    cache.clone().get_or_shape(Some((1, "hello")), shape);
    assert_eq!(calls.get(), 2);

    let (_, text) = cache.get_or_shape(Some((1, "other")), || {
      (InlineLayout::new(), "other".to_owned())
    });
    assert_eq!(text, "other");
    InlineLayoutCache::default().get_or_shape(Some((1, "hello")), shape);
    assert_eq!(calls.get(), 3);
  }

  #[test]
  fn uncacheable_shapes_do_not_create_entries() {
    let cache = InlineLayoutCache::default();
    cache.get_or_shape(None, || (InlineLayout::new(), "hello".to_owned()));
    assert!(cache.shapes.borrow().is_empty());
  }

  #[test]
  fn measurements_share_only_with_clones_and_matching_inputs() {
    let cache = InlineLayoutCache::default();
    let first = Size {
      width: 10.0,
      height: 20.0,
    };
    let second = Size {
      width: 30.0,
      height: 40.0,
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
