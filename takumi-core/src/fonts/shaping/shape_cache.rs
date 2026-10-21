//! Blink's [`NGShapeCache`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/platform/fonts/shaping/ng_shape_cache.h).

use std::{
  cell::{Cell, RefCell},
  fmt::{self, Debug, Formatter},
  hash::Hasher,
  rc::Rc,
};

use rustc_hash::{FxHashMap, FxHasher};

use super::shape_result::ShapeResult;
use crate::{fonts::font_description::FontDescription, style::Direction as TextDirection};

/// Blink's `ShapeCacheKey`, with the font's whole description in place of its locale and features.
struct ShapeCacheKey {
  text: Box<str>,
  start: usize,
  end: usize,
  direction: TextDirection,
  description: FontDescription,
}

impl ShapeCacheKey {
  fn matches(
    &self,
    text: &str,
    start: usize,
    end: usize,
    direction: TextDirection,
    description: &FontDescription,
  ) -> bool {
    *self.text == *text
      && self.start == start
      && self.end == end
      && self.direction == direction
      && self.description == *description
  }
}

/// The keys that share a digest, with their results.
type Bucket = Vec<(ShapeCacheKey, Rc<ShapeResult>)>;

/// Blink's `NGShapeCache`: the results of shaping short texts, kept by a fallback list. Blink keeps
/// it on the list's primary font, which a result here would hold back.
#[derive(Default)]
pub(crate) struct ShapeCache {
  /// The results by a digest of their key, each bucket searched by equality, as the description
  /// holds floats.
  entries: RefCell<FxHashMap<u64, Bucket>>,
  len: Cell<usize>,
}

impl ShapeCache {
  /// Blink's `kMaxTextLengthOfEntries`, in UTF-16 code units.
  const MAX_TEXT_LENGTH_OF_ENTRIES: usize = 30;
  /// Blink's `kMaxSize`.
  const MAX_SIZE: usize = 2048;

  /// Blink's `GetOrCreate`: the result of shaping `start..end` of `text` in `description`, or
  /// `shape`'s, kept when `text` is short and the cache has room.
  pub(crate) fn get_or_create(
    &self,
    text: &str,
    start: usize,
    end: usize,
    direction: TextDirection,
    description: &FontDescription,
    shape: impl FnOnce() -> ShapeResult,
  ) -> Rc<ShapeResult> {
    if text.len() > Self::MAX_TEXT_LENGTH_OF_ENTRIES * 3
      || text.encode_utf16().count() > Self::MAX_TEXT_LENGTH_OF_ENTRIES
    {
      return Rc::new(shape());
    }

    let mut hasher = FxHasher::default();

    hasher.write(text.as_bytes());
    hasher.write_usize(start);
    hasher.write_usize(end);
    hasher.write_u8(direction as u8);

    let digest = hasher.finish();

    if let Some((_, result)) = self.entries.borrow().get(&digest).and_then(|bucket| {
      bucket
        .iter()
        .find(|(key, _)| key.matches(text, start, end, direction, description))
    }) {
      return Rc::clone(result);
    }

    let result = Rc::new(shape());

    if self.len.get() < Self::MAX_SIZE {
      self.len.set(self.len.get() + 1);
      self.entries.borrow_mut().entry(digest).or_default().push((
        ShapeCacheKey {
          text: text.into(),
          start,
          end,
          direction,
          description: description.clone(),
        },
        Rc::clone(&result),
      ));
    }

    result
  }
}

impl Debug for ShapeCache {
  fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
    f.debug_struct("ShapeCache")
      .field("len", &self.len.get())
      .finish()
  }
}
