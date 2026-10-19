//! Blink's [`FontFallbackMap`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/platform/fonts/font_fallback_map.cc).

use std::{
  cell::RefCell,
  hash::{Hash, Hasher},
  rc::Rc,
};

use rustc_hash::{FxHashMap, FxHasher};

use super::{font_description::FontDescription, font_fallback_list::FontFallbackList};
use crate::resources::font::Fonts;

/// The descriptions that share a digest, with their lists.
type Bucket = Vec<(FontDescription, Rc<FontFallbackList>)>;

/// One fallback list per font description, shared by every style that has it.
#[derive(Default)]
pub(crate) struct FontFallbackMap {
  /// The lists by a digest of their description, each bucket searched by equality, as the
  /// description's sizes are floats.
  lists: RefCell<FxHashMap<u64, Bucket>>,
}

impl FontFallbackMap {
  /// Blink's `Get`.
  pub(crate) fn get(&self, fonts: &Fonts, description: &FontDescription) -> Rc<FontFallbackList> {
    let mut hasher = FxHasher::default();

    description.family.hash_tokens(&mut hasher);
    description
      .selection
      .font_selection_request
      .hash(&mut hasher);
    hasher.write_u32(description.computed_size.to_bits());

    let mut lists = self.lists.borrow_mut();
    let bucket = lists.entry(hasher.finish()).or_default();

    if let Some((_, list)) = bucket.iter().find(|(key, _)| key == description) {
      return Rc::clone(list);
    }

    let list = Rc::new(FontFallbackList::new(fonts, description));

    bucket.push((description.clone(), Rc::clone(&list)));
    list
  }
}
