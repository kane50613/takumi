//! Blink's [`FontFallbackMap`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/platform/fonts/font_fallback_map.cc).

use std::{
  cell::RefCell,
  hash::{Hash, Hasher},
  rc::Rc,
};

use rustc_hash::{FxHashMap, FxHasher};

use super::{font_description::FontFallbackKey, font_fallback_list::FontFallbackList};
use crate::resources::font::Fonts;

/// The keys that share a digest, with their lists.
type Bucket = Vec<(FontFallbackKey, Rc<FontFallbackList>)>;

/// One fallback list per [`FontFallbackKey`], shared by every style that has it. Approximate:
/// Blink keys its map by the whole `FontDescription`, though the list reads only the key.
#[derive(Default)]
pub(crate) struct FontFallbackMap {
  /// The lists by a digest of their key, each bucket searched by equality, as the key's sizes
  /// are floats.
  lists: RefCell<FxHashMap<u64, Bucket>>,
}

impl FontFallbackMap {
  /// Blink's `Get`.
  pub(crate) fn get(&self, fonts: &Fonts, key: &FontFallbackKey) -> Rc<FontFallbackList> {
    let mut hasher = FxHasher::default();

    key.family.hash_tokens(&mut hasher);
    key.selection.font_selection_request.hash(&mut hasher);
    hasher.write_u32(key.computed_size.to_bits());

    let mut lists = self.lists.borrow_mut();
    let bucket = lists.entry(hasher.finish()).or_default();

    if let Some((_, list)) = bucket.iter().find(|(cached, _)| cached == key) {
      return Rc::clone(list);
    }

    let list = Rc::new(FontFallbackList::new(fonts, key));

    bucket.push((key.clone(), Rc::clone(&list)));
    list
  }
}
