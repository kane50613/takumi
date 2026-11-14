//! What each font registration added, by the key its loader gives, and the `@font-face` files a
//! renderer loaded, by URL. Each holds its font file in the renderer's [`Cache`], which evicts the
//! files renders used least recently, and a render leases the files it uses until it settles.

use std::{
  collections::HashMap,
  mem::take,
  sync::{
    Mutex, MutexGuard, PoisonError,
    atomic::{AtomicU32, Ordering},
  },
};

use takumi_core::{
  Cache, Fonts,
  resources::{
    font::{FontError, FontSource, RegisteredFamily},
    held_fonts::FontLease,
  },
};

use crate::input::RegisteredFont;

/// A family as the registry matches its name: each character lowercased on its own.
fn fold(family: &str) -> String {
  family.chars().flat_map(char::to_lowercase).collect()
}

/// What one registration added, from the font file `id` names.
#[derive(Clone)]
struct Registration {
  id: u64,
  families: Vec<RegisteredFamily>,
  /// Made by `registerFont`, so its file is never evicted.
  pinned: bool,
}

impl Registration {
  fn has_family(&self, folded: &str) -> bool {
    self
      .families
      .iter()
      .any(|family| fold(&family.name) == folded)
  }

  /// Removes the faces it added from `fonts`.
  fn unregister(&self, fonts: &mut Fonts) {
    for family in &self.families {
      fonts.unregister_source(&family.name, self.id);
    }
  }
}

/// The registrations of a renderer's fonts and the `@font-face` files it loaded, each by the font
/// file it holds in the renderer's cache. Clones share no state, so a binding can change a copy and
/// publish it.
#[derive(Clone, Default)]
pub struct FontRegistrations {
  by_key: HashMap<String, Registration>,
  /// Registrations made without a key, all of them by `registerFont`.
  unkeyed: Vec<Registration>,
  files: HashMap<String, u64>,
}

impl FontRegistrations {
  /// The families a registration under `key` added, counting as no lookup.
  pub fn families(&self, key: &str) -> Option<&[RegisteredFamily]> {
    self
      .by_key
      .get(key)
      .map(|registration| registration.families.as_slice())
  }

  /// The families a registration under `key` added, with a lease on its file; none once the file
  /// was evicted.
  pub fn lease(&self, cache: &Cache, key: &str) -> Option<(Vec<RegisteredFamily>, FontLease)> {
    let registration = self.by_key.get(key)?;
    let lease = cache.fonts().lease(registration.id)?;

    Some((registration.families.clone(), lease))
  }

  /// The families a registration under `key` added, its file kept from eviction from now on, as
  /// `registerFont` keeps a font a render loaded first.
  pub fn pin(&mut self, cache: &Cache, key: &str) -> Option<Vec<RegisteredFamily>> {
    let registration = self.by_key.get_mut(key)?;

    if !cache.fonts().holds(registration.id) {
      return None;
    }

    registration.pinned = true;
    cache.fonts().pin(registration.id, true);

    Some(registration.families.clone())
  }

  /// Records `font`, which `fonts` registered under `key`, returning its families with a lease on
  /// its file. A `pinned` file is never evicted. A file `key` held before goes once nothing else
  /// holds it.
  pub fn admit(
    &mut self,
    fonts: &mut Fonts,
    cache: &Cache,
    key: Option<&str>,
    font: RegisteredFont,
    pinned: bool,
  ) -> (Vec<RegisteredFamily>, FontLease) {
    let lease = cache.fonts().keep(font.id, font.bytes);
    let pinned = pinned
      || key
        .and_then(|key| self.by_key.get(key))
        .is_some_and(|held| held.id == font.id && held.pinned);
    let families = font.families.clone();
    let registration = Registration {
      id: font.id,
      families: font.families,
      pinned,
    };

    if pinned {
      cache.fonts().pin(font.id, true);
    }

    match key {
      Some(key) => {
        if let Some(replaced) = self.by_key.insert(key.to_owned(), registration) {
          self.release_replaced(fonts, cache, &replaced);
        }
      }
      None => self.unkeyed.push(registration),
    }

    (families, lease)
  }

  /// Unregisters what `replaced` added once nothing else holds its file.
  fn release_replaced(&self, fonts: &mut Fonts, cache: &Cache, replaced: &Registration) {
    if self.drop_unheld(cache, replaced.id) {
      replaced.unregister(fonts);
    }
  }

  /// Loads the `@font-face` file `url`, `bytes` of font data, into `fonts`, and holds it leased.
  pub fn admit_file(
    &mut self,
    fonts: &mut Fonts,
    cache: &Cache,
    url: &str,
    source: FontSource<'static>,
    bytes: u64,
  ) -> Result<FontLease, FontError> {
    let id = source.id().ok_or(FontError::UnsupportedFormat)?;

    fonts.load_font_file(url, source)?;

    let lease = cache.fonts().keep(id, bytes);

    if let Some(replaced) = self.files.insert(url.to_owned(), id) {
      self.drop_unheld(cache, replaced);
    }

    Ok(lease)
  }

  /// A lease on the `@font-face` file `url`, none once it was evicted.
  pub fn lease_file(&self, cache: &Cache, url: &str) -> Option<FontLease> {
    cache.fonts().lease(*self.files.get(url)?)
  }

  /// Unregisters from `fonts` what the files `evicted` names held, each registration's own faces
  /// alone, and forgets the registrations and files.
  pub fn release(&mut self, fonts: &mut Fonts, evicted: &[u64]) {
    let is_evicted = |registration: &Registration| evicted.contains(&registration.id);

    for registration in self.by_key.values().filter(|each| is_evicted(each)) {
      registration.unregister(fonts);
    }

    self
      .by_key
      .retain(|_, registration| !is_evicted(registration));
    self.files.retain(|url, id| {
      let kept = !evicted.contains(id);

      if !kept {
        fonts.forget_font_file(url);
      }

      kept
    });
  }

  /// Gives the faces the registrations under `keys` added precedence in their families, a later key
  /// over an earlier one, as a render's `fonts` list orders them; false when they already have it.
  pub fn prefer(&self, fonts: &mut Fonts, keys: &[String]) -> bool {
    let sources: Vec<(&str, u64)> = keys
      .iter()
      .filter_map(|key| self.by_key.get(key))
      .flat_map(|registration| {
        registration
          .families
          .iter()
          .map(|family| (family.name.as_str(), registration.id))
      })
      .collect();

    fonts.prefer_sources(&sources)
  }

  /// Forgets the registrations that added to `family`, as `unregisterFont` removes it, so
  /// registering one of them again reaches the registry, and drops the files no registration holds.
  pub fn forget_family(&mut self, cache: &Cache, family: &str) {
    let folded = fold(family);
    let mut forgotten: Vec<u64> = self
      .by_key
      .values()
      .chain(&self.unkeyed)
      .filter(|registration| registration.has_family(&folded))
      .map(|registration| registration.id)
      .collect();

    self
      .by_key
      .retain(|_, registration| !registration.has_family(&folded));
    self
      .unkeyed
      .retain(|registration| !registration.has_family(&folded));
    forgotten.sort_unstable();
    forgotten.dedup();

    for id in forgotten {
      self.drop_unheld(cache, id);
    }
  }

  /// Forgets every registration and file, dropping their files from `cache`.
  pub fn clear(&mut self, cache: &Cache) {
    let registrations = take(&mut self.by_key)
      .into_values()
      .chain(take(&mut self.unkeyed));

    for id in registrations
      .map(|registration| registration.id)
      .chain(take(&mut self.files).into_values())
    {
      cache.fonts().forget(id);
    }
  }

  /// Drops the file `id` names from `cache` once no registration or file holds it, returning true,
  /// or else pins it while a registration from `registerFont` holds it.
  fn drop_unheld(&self, cache: &Cache, id: u64) -> bool {
    let held = self
      .registrations()
      .any(|registration| registration.id == id)
      || self.files.values().any(|&file| file == id);

    if !held {
      cache.fonts().forget(id);

      return true;
    }

    let pinned = self
      .registrations()
      .any(|registration| registration.id == id && registration.pinned);

    cache.fonts().pin(id, pinned);

    false
  }

  fn registrations(&self) -> impl Iterator<Item = &Registration> {
    self.by_key.values().chain(&self.unkeyed)
  }
}

/// The leases each render in flight holds on the font files it uses, by the number a binding hands
/// to JavaScript, released together when the render settles.
#[derive(Default)]
pub struct RenderLeases {
  next: AtomicU32,
  held: Mutex<HashMap<u32, Vec<FontLease>>>,
}

impl RenderLeases {
  /// A new, empty set of leases.
  pub fn open(&self) -> u32 {
    let lease = self.next.fetch_add(1, Ordering::Relaxed);

    self.lock().insert(lease, Vec::new());

    lease
  }

  /// Adds `font` to the set `lease` names, or releases it at once without one.
  pub fn hold(&self, lease: Option<u32>, font: FontLease) {
    let Some(lease) = lease else {
      return;
    };

    if let Some(held) = self.lock().get_mut(&lease) {
      held.push(font);
    }
  }

  /// Releases every lease of the set `lease` names.
  pub fn close(&self, lease: u32) {
    let released = self.lock().remove(&lease);

    // Released after the lock, since releasing can evict.
    drop(released);
  }

  fn lock(&self) -> MutexGuard<'_, HashMap<u32, Vec<FontLease>>> {
    self.held.lock().unwrap_or_else(PoisonError::into_inner)
  }
}

#[cfg(test)]
mod tests {
  use std::fs;

  use takumi_core::resources::font::{FontOverride, FontResource};

  use super::*;
  use crate::input::register_resource;

  /// Geist TTF bytes made distinct by `tail`, which parsing ignores.
  fn geist(tail: u32) -> Vec<u8> {
    let mut bytes = fs::read(concat!(
      env!("CARGO_MANIFEST_DIR"),
      "/../assets/fonts/geist/geist-latin-wght-300-800.ttf"
    ))
    .unwrap();

    bytes.extend(tail.to_le_bytes());

    bytes
  }

  /// `bytes` registered as `family` under `key`, its lease dropped at once.
  fn register(
    fonts: &mut Fonts,
    registrations: &mut FontRegistrations,
    cache: &Cache,
    key: &str,
    family: &str,
    bytes: Vec<u8>,
  ) {
    let resource = FontResource::new(bytes).override_info(FontOverride {
      family_name: Some(family.into()),
      ..Default::default()
    });
    let registered = register_resource(fonts, resource).unwrap();

    drop(registrations.admit(fonts, cache, Some(key), registered, false));
    registrations.release(fonts, &cache.fonts().take_evicted());
  }

  fn id(bytes: Vec<u8>) -> u64 {
    FontResource::new(bytes)
      .into_resolved()
      .unwrap()
      .id()
      .unwrap()
  }

  #[test]
  fn one_family_fed_new_files_stays_within_the_budget() {
    let file = geist(0).len() as u64;
    let cache = Cache::builder().max_bytes(4 * file).build();
    let mut fonts = Fonts::default();
    let mut registrations = FontRegistrations::default();

    for tail in 0..64 {
      register(
        &mut fonts,
        &mut registrations,
        &cache,
        &format!("key {tail}"),
        "Inter",
        geist(tail),
      );
    }

    let held = registrations.by_key.len();
    let registered = (0..64)
      .filter(|&tail| fonts.clone().unregister_source("Inter", id(geist(tail))))
      .count();

    assert!(cache.stats().fonts.current <= 4 * file);
    assert!((1..=4).contains(&held), "{held}");
    assert!(registrations.families("key 63").is_some());
    assert_eq!(registered, held, "each evicted file left the registry");
  }

  #[test]
  fn the_same_bytes_under_two_keys_count_once() {
    let cache = Cache::default();
    let mut fonts = Fonts::default();
    let mut registrations = FontRegistrations::default();

    register(&mut fonts, &mut registrations, &cache, "a", "A", geist(0));
    register(&mut fonts, &mut registrations, &cache, "b", "B", geist(0));

    assert_eq!(cache.stats().fonts.current, geist(0).len() as u64);
  }

  #[test]
  fn a_key_registered_again_releases_the_file_it_held() {
    let cache = Cache::default();
    let mut fonts = Fonts::default();
    let mut registrations = FontRegistrations::default();

    register(&mut fonts, &mut registrations, &cache, "k", "A", geist(0));
    register(&mut fonts, &mut registrations, &cache, "k", "B", geist(1));

    assert_eq!(cache.stats().fonts.current, geist(1).len() as u64);
    assert!(!fonts.unregister_family("A"));
    assert!(fonts.unregister_family("B"));
  }

  #[test]
  fn a_leased_file_stays_until_its_render_settles() {
    let cache = Cache::builder().fonts(0).build();
    let leases = RenderLeases::default();
    let lease = leases.open();
    let mut fonts = Fonts::default();
    let mut registrations = FontRegistrations::default();
    let registered = register_resource(&mut fonts, FontResource::new(geist(0))).unwrap();
    let (_, held) = registrations.admit(&mut fonts, &cache, Some("k"), registered, false);

    leases.hold(Some(lease), held);
    registrations.release(&mut fonts, &cache.fonts().take_evicted());

    assert!(registrations.lease(&cache, "k").is_some());

    leases.close(lease);
    registrations.release(&mut fonts, &cache.fonts().take_evicted());

    assert!(registrations.families("k").is_none());
    assert_eq!(cache.stats().fonts.current, 0);
  }

  #[test]
  fn a_pinned_registration_stays_past_the_budget() {
    let cache = Cache::builder().fonts(0).build();
    let mut fonts = Fonts::default();
    let mut registrations = FontRegistrations::default();

    register(&mut fonts, &mut registrations, &cache, "k", "A", geist(0));

    let registered = register_resource(&mut fonts, FontResource::new(geist(1))).unwrap();

    drop(registrations.admit(&mut fonts, &cache, Some("pinned"), registered, true));
    registrations.release(&mut fonts, &cache.fonts().take_evicted());

    assert!(registrations.families("k").is_none());
    assert!(registrations.lease(&cache, "pinned").is_some());

    registrations.forget_family(&cache, "geist");

    assert!(!cache.fonts().holds(id(geist(1))));
  }
}
