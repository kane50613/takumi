//! What each font registration added, by the key its loader gives, so a renderer can skip loading a
//! font it still holds and release the families renders used least recently past a byte budget.

use std::collections::HashMap;

use takumi_core::{
  Fonts,
  resources::font::{FontError, RegisteredFamily},
};

#[cfg(doc)]
use crate::input::FontOptions;
use crate::register_embedded;

/// A family as the registry matches its name: each character lowercased on its own.
fn fold(family: &str) -> String {
  family.chars().flat_map(char::to_lowercase).collect()
}

struct CachedFamily {
  /// Bytes of font data its registrations hold. A file holding several families counts in each.
  bytes: u64,
  /// Added to by an explicit registration, so never released for space.
  pinned: bool,
  last_used: u64,
  keys: Vec<String>,
}

/// The registrations of a renderer's fonts, and the budget that bounds the unpinned ones.
pub struct FontCache {
  max_bytes: u64,
  by_key: HashMap<String, Vec<RegisteredFamily>>,
  /// The key of a registration of each font content; see [`FontOptions::content_hash`].
  by_content: HashMap<u64, String>,
  families: HashMap<String, CachedFamily>,
  clock: u64,
}

impl FontCache {
  /// A cache keeping at most `max_bytes` of unpinned font data, unbounded without one.
  pub fn new(max_bytes: Option<u64>) -> Self {
    Self {
      max_bytes: max_bytes.unwrap_or(u64::MAX),
      by_key: HashMap::new(),
      by_content: HashMap::new(),
      families: HashMap::new(),
      clock: 0,
    }
  }

  /// The families `key` registered, marked used and pinned when `pin` is set; `None` once any of
  /// them was released.
  pub fn lookup(&mut self, key: &str, pin: bool) -> Option<Vec<RegisteredFamily>> {
    let registered = self.by_key.get(key)?.clone();
    let now = self.tick();

    for family in &registered {
      let cached = self.families.get_mut(&fold(&family.name))?;

      cached.last_used = now;
      cached.pinned |= pin;
    }

    Some(registered)
  }

  /// The families a registration of the same `content` added, now also under `key`, so a copy of
  /// a font the renderer holds is not parsed again; `None` once that registration was released.
  pub fn lookup_content(
    &mut self,
    content: u64,
    key: Option<&str>,
    pin: bool,
  ) -> Option<Vec<RegisteredFamily>> {
    let held = self.by_content.get(&content)?.clone();
    let Some(registered) = self.lookup(&held, pin) else {
      self.by_content.remove(&content);
      return None;
    };

    if let Some(key) = key.filter(|key| *key != held) {
      self.record(Some(key), &registered, 0, pin);
    }

    Some(registered)
  }

  /// Remembers that the registration under `key` holds `content`.
  pub fn remember_content(&mut self, content: u64, key: Option<&str>) {
    if let Some(key) = key {
      self.by_content.insert(content, key.to_owned());
    }
  }

  /// Records a registration, then for a font that is not pinned releases what the budget no
  /// longer fits, other than that font.
  pub fn admit(
    &mut self,
    fonts: &mut Fonts,
    key: Option<&str>,
    registered: &[RegisteredFamily],
    bytes: u64,
    pinned: bool,
  ) -> Result<(), FontError> {
    self.record(key, registered, bytes, pinned);

    if pinned {
      return Ok(());
    }

    self.release_past_budget(fonts, registered)
  }

  /// Records what registering a font under `key` added to the registry, `bytes` of font data.
  pub fn record(
    &mut self,
    key: Option<&str>,
    registered: &[RegisteredFamily],
    bytes: u64,
    pinned: bool,
  ) {
    let now = self.tick();

    if let Some(key) = key {
      for previous in self.by_key.remove(key).into_iter().flatten() {
        if let Some(cached) = self.families.get_mut(&fold(&previous.name)) {
          cached.keys.retain(|each| each != key);
        }
      }
    }

    for family in registered {
      let cached = self
        .families
        .entry(fold(&family.name))
        .or_insert_with(|| CachedFamily {
          bytes: 0,
          pinned,
          last_used: now,
          keys: Vec::new(),
        });

      cached.bytes += bytes;
      cached.pinned |= pinned;
      cached.last_used = now;
      cached.keys.extend(key.map(str::to_owned));
    }

    if let Some(key) = key {
      self.by_key.insert(key.to_owned(), registered.to_vec());
    }
  }

  /// Marks `families` used now, as a render that names them starts.
  pub fn touch(&mut self, families: &[String]) {
    let now = self.tick();

    for family in families {
      if let Some(cached) = self.families.get_mut(&fold(family)) {
        cached.last_used = now;
      }
    }
  }

  /// Releases from `fonts` what the budget no longer fits, other than the families `kept`.
  pub fn release_past_budget(
    &mut self,
    fonts: &mut Fonts,
    kept: &[RegisteredFamily],
  ) -> Result<(), FontError> {
    let keep: Vec<String> = kept.iter().map(|family| fold(&family.name)).collect();

    self.evict(fonts, &keep)
  }

  /// Releases the least recently used unpinned families but `keep` from `fonts` until the rest fit
  /// the budget, with every family the same registrations added. An embedded last-resort font
  /// that goes with a family is registered again. Renders in progress keep the fonts they started
  /// with.
  fn evict(&mut self, fonts: &mut Fonts, keep: &[String]) -> Result<(), FontError> {
    while self.unpinned_bytes() > self.max_bytes {
      let Some(oldest) = self
        .families
        .iter()
        .filter(|(folded, family)| !family.pinned && !keep.contains(folded))
        .min_by_key(|(_, family)| family.last_used)
        .map(|(folded, _)| folded.clone())
      else {
        return Ok(());
      };

      for family in self.registered_with(&oldest) {
        if keep.contains(&family)
          || self
            .families
            .get(&family)
            .is_some_and(|cached| cached.pinned)
        {
          continue;
        }

        fonts.unregister_family(&family);
        register_embedded(fonts, |name| fold(name) == family)?;
        self.forget(&family);
      }
    }

    Ok(())
  }

  /// `family` and every family a registration that added to it also added.
  fn registered_with(&self, family: &str) -> Vec<String> {
    let mut names = vec![family.to_owned()];

    for key in self
      .families
      .get(family)
      .map(|cached| cached.keys.as_slice())
      .unwrap_or_default()
    {
      for registered in self.by_key.get(key).into_iter().flatten() {
        let folded = fold(&registered.name);

        if !names.contains(&folded) {
          names.push(folded);
        }
      }
    }

    names
  }

  fn unpinned_bytes(&self) -> u64 {
    self
      .families
      .values()
      .filter(|family| !family.pinned)
      .map(|family| family.bytes)
      .sum()
  }

  /// Forgets `family` and the registrations that added to it, so registering one of them again
  /// reaches the registry.
  pub fn forget(&mut self, family: &str) {
    if let Some(cached) = self.families.remove(&fold(family)) {
      for key in cached.keys {
        self.by_key.remove(&key);
      }
    }
  }

  /// Forgets every registration.
  pub fn clear(&mut self) {
    self.by_key.clear();
    self.by_content.clear();
    self.families.clear();
  }

  fn tick(&mut self) -> u64 {
    self.clock += 1;
    self.clock
  }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
  use std::fs;

  use takumi_core::resources::font::{FontOverride, FontResource};

  use super::*;
  use crate::{default_fonts, input::register_resource};

  /// Geist registered under `family`, as a render's `fonts` list or `registerFont` would.
  fn register(fonts: &mut Fonts, cache: &mut FontCache, family: &str, pinned: bool) -> u64 {
    register_under(fonts, cache, family, family, pinned)
  }

  /// Geist registered as `family` under `key`.
  fn register_under(
    fonts: &mut Fonts,
    cache: &mut FontCache,
    key: &str,
    family: &str,
    pinned: bool,
  ) -> u64 {
    let bytes = fs::read(concat!(
      env!("CARGO_MANIFEST_DIR"),
      "/../assets/fonts/geist/Geist[wght].woff2"
    ))
    .unwrap();
    let resource = FontResource::new(bytes).override_info(FontOverride {
      family_name: Some(family.into()),
      ..Default::default()
    });
    let (registered, size) = register_resource(fonts, resource).unwrap();

    cache
      .admit(fonts, Some(key), &registered, size, pinned)
      .unwrap();

    size
  }

  /// A cache that fits `families` fonts the size of Geist, and an empty registry.
  fn fitting(families: u64) -> (Fonts, FontCache) {
    let mut fonts = Fonts::default();
    let size = register(&mut fonts, &mut FontCache::new(None), "Probe", true);

    fonts.unregister_family("Probe");

    (fonts, FontCache::new(Some(size * families + size / 2)))
  }

  #[test]
  fn releases_the_least_recently_used_family_past_the_budget() {
    let (mut fonts, mut cache) = fitting(1);

    register(&mut fonts, &mut cache, "A", false);
    register(&mut fonts, &mut cache, "B", false);

    assert!(cache.lookup("A", false).is_none());
    assert!(!fonts.unregister_family("A"), "A left the registry");
    assert!(cache.lookup("B", false).is_some());
  }

  #[test]
  fn a_lookup_counts_as_use() {
    let (mut fonts, mut cache) = fitting(2);

    register(&mut fonts, &mut cache, "A", false);
    register(&mut fonts, &mut cache, "B", false);
    cache.lookup("A", false);
    register(&mut fonts, &mut cache, "C", false);

    assert!(cache.lookup("A", false).is_some());
    assert!(cache.lookup("B", false).is_none());
  }

  #[test]
  fn keeps_pinned_families() {
    let (mut fonts, mut cache) = fitting(1);

    register(&mut fonts, &mut cache, "Pinned", true);
    register(&mut fonts, &mut cache, "A", false);
    cache.lookup("A", true);
    register(&mut fonts, &mut cache, "B", false);

    assert!(cache.lookup("Pinned", false).is_some());
    assert!(cache.lookup("A", false).is_some());
  }

  #[test]
  fn keeps_the_font_it_just_loaded_past_the_budget() {
    let (mut fonts, mut cache) = fitting(0);

    register(&mut fonts, &mut cache, "A", false);

    assert!(cache.lookup("A", false).is_some());
  }

  #[test]
  fn registers_the_embedded_font_again_after_releasing_its_family() {
    let (_, mut cache) = fitting(1);
    let mut fonts = default_fonts().unwrap();

    register(&mut fonts, &mut cache, "Geist", false);
    register(&mut fonts, &mut cache, "A", false);

    assert!(cache.lookup("Geist", false).is_none());
    assert!(
      fonts.unregister_family("Geist"),
      "the embedded Geist is back"
    );
  }

  #[test]
  fn a_key_registered_again_leaves_its_old_family() {
    let (mut fonts, mut cache) = fitting(2);

    register_under(&mut fonts, &mut cache, "K", "A", false);
    register_under(&mut fonts, &mut cache, "K", "B", false);
    register(&mut fonts, &mut cache, "C", false);

    assert!(cache.lookup("K", false).is_some());
  }

  #[test]
  fn forgets_a_released_family_case_insensitively() {
    let (mut fonts, mut cache) = fitting(4);

    register(&mut fonts, &mut cache, "Alpha", false);
    cache.forget("alpha");

    assert!(cache.lookup("Alpha", false).is_none());
  }
}
