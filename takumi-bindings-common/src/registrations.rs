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
    font::{FontError, FontSource, RegisteredFamily, RegisteredSource},
    held_fonts::FontLease,
  },
};

use crate::input::RegisteredFont;

/// A family as the registry matches its name: each character lowercased on its own.
fn fold(family: &str) -> String {
  family.chars().flat_map(char::to_lowercase).collect()
}

/// What one registration added from its source.
#[derive(Clone)]
struct Registration {
  source: RegisteredSource,
  families: Vec<RegisteredFamily>,
  /// Made by `registerFont`, so its file is never evicted.
  pinned: bool,
}

impl Registration {
  /// The id of the font file it holds.
  fn id(&self) -> u64 {
    self.source.id()
  }

  fn has_family(&self, folded: &str) -> bool {
    self
      .families
      .iter()
      .any(|family| fold(&family.name) == folded)
  }

  /// Whether it added the faces `other` added to the folded `family`.
  fn shares(&self, other: &Self, folded: &str) -> bool {
    self.source == other.source && self.has_family(folded)
  }

  /// Removes the faces it added from `fonts`.
  fn unregister(&self, fonts: &mut Fonts) {
    for family in &self.families {
      fonts.unregister_source(&family.name, &self.source);
    }
  }
}

/// Whether a registered font stays until it is unregistered, as `registerFont` asks, or leaves
/// when the cache evicts it, as a render's `fonts` list does.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Retention {
  Pinned,
  Evictable,
}

/// The registrations of a renderer's fonts and the `@font-face` files it loaded, each by the font
/// file it holds in the renderer's cache. Clones share no state, so a binding can change a copy and
/// publish it.
#[derive(Clone, Default)]
pub struct FontRegistrations {
  by_key: HashMap<String, Registration>,
  /// Registrations no key finds: the ones `registerFont` made without a key, and what a keyed one
  /// keeps once `unregisterFont` removed another of its families.
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
    let lease = cache.fonts().lease(registration.id())?;

    Some((registration.families.clone(), lease))
  }

  /// The families a registration under `key` added, its file kept from eviction from now on, as
  /// `registerFont` keeps a font a render loaded first.
  pub fn pin(&mut self, cache: &Cache, key: &str) -> Option<Vec<RegisteredFamily>> {
    let registration = self.by_key.get_mut(key)?;

    if !cache.fonts().pin(registration.id(), true) {
      return None;
    }

    registration.pinned = true;

    Some(registration.families.clone())
  }

  /// Records `font`, which `fonts` registered under `key`, returning its families with a lease on
  /// its file. A [`Retention::Pinned`] file is never evicted. The faces `key` added before go unless another
  /// registration added them too, and their file goes once nothing holds it.
  pub fn admit(
    &mut self,
    fonts: &mut Fonts,
    cache: &Cache,
    key: Option<&str>,
    font: RegisteredFont,
    retention: Retention,
  ) -> (Vec<RegisteredFamily>, FontLease) {
    let id = font.source.id();
    let lease = cache.fonts().keep(id, font.bytes);
    let pinned = retention == Retention::Pinned
      || key
        .and_then(|key| self.by_key.get(key))
        .is_some_and(|held| held.id() == id && held.pinned);
    let families = font.families.clone();
    let registration = Registration {
      source: font.source,
      families: font.families,
      pinned,
    };

    if pinned {
      cache.fonts().pin(id, true);
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

  /// Unregisters the faces `replaced` added that no other registration added, and drops its file
  /// once nothing holds it.
  fn release_replaced(&self, fonts: &mut Fonts, cache: &Cache, replaced: &Registration) {
    for family in &replaced.families {
      let folded = fold(&family.name);

      if !self
        .registrations()
        .any(|registration| registration.shares(replaced, &folded))
      {
        fonts.unregister_source(&family.name, &replaced.source);
      }
    }

    self.drop_unheld(cache, replaced.id());
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
    let is_evicted = |registration: &Registration| evicted.contains(&registration.id());

    for registration in self.registrations().filter(|each| is_evicted(each)) {
      registration.unregister(fonts);
    }

    self
      .by_key
      .retain(|_, registration| !is_evicted(registration));
    self
      .unkeyed
      .retain(|registration| !is_evicted(registration));
    self.files.retain(|url, id| {
      let kept = !evicted.contains(id);

      if !kept {
        fonts.forget_font_file(url);
      }

      kept
    });
  }

  /// Forgets the registrations under `keys` whose files no render leases and nothing pins, as a
  /// font with `cache: "none"` asks once its render settles, unregistering their faces once no
  /// other registration holds their file. Returns those keys and the keys nothing is under.
  pub fn forget_unleased(
    &mut self,
    fonts: &mut Fonts,
    cache: &Cache,
    keys: &[String],
  ) -> Vec<String> {
    let mut forgotten = Vec::new();

    for key in keys {
      if let Some(registration) = self.by_key.get(key) {
        if registration.pinned || cache.fonts().is_held(registration.id) {
          continue;
        }

        if let Some(registration) = self.by_key.remove(key) {
          self.release_replaced(fonts, cache, &registration);
        }
      }

      forgotten.push(key.clone());
    }

    forgotten
  }

  /// Gives the faces the registrations under `keys` added precedence in their families, a later key
  /// over an earlier one, as a render's `fonts` list orders them; false when they already have it.
  pub fn prefer(&self, fonts: &mut Fonts, keys: &[String]) -> bool {
    let sources: Vec<(&str, &RegisteredSource)> = keys
      .iter()
      .filter_map(|key| self.by_key.get(key))
      .flat_map(|registration| {
        registration
          .families
          .iter()
          .map(|family| (family.name.as_str(), &registration.source))
      })
      .collect();

    fonts.prefer_sources(&sources)
  }

  /// Forgets `family` in the registrations that added to it, as `unregisterFont` removes it, so
  /// registering one of them again reaches the registry. What one keeps of other families stays
  /// held under no key, and the files no registration holds drop.
  pub fn forget_family(&mut self, cache: &Cache, family: &str) {
    let folded = fold(family);
    let keys: Vec<String> = self
      .by_key
      .iter()
      .filter(|(_, registration)| registration.has_family(&folded))
      .map(|(key, _)| key.clone())
      .collect();
    let (mut forgotten, unkeyed): (Vec<_>, Vec<_>) = take(&mut self.unkeyed)
      .into_iter()
      .partition(|registration| registration.has_family(&folded));

    self.unkeyed = unkeyed;
    forgotten.extend(keys.iter().filter_map(|key| self.by_key.remove(key)));

    for mut registration in forgotten {
      let id = registration.id();

      registration
        .families
        .retain(|each| fold(&each.name) != folded);

      if !registration.families.is_empty() {
        self.unkeyed.push(registration);
      }

      self.drop_unheld(cache, id);
    }
  }

  /// Forgets every registration and file, dropping their files from `cache`.
  pub fn clear(&mut self, cache: &Cache) {
    let registrations = take(&mut self.by_key)
      .into_values()
      .chain(take(&mut self.unkeyed));

    for id in registrations
      .map(|registration| registration.id())
      .chain(take(&mut self.files).into_values())
    {
      cache.fonts().forget(id);
    }
  }

  /// Drops the file `id` names from `cache` once no registration or file holds it, or else pins it
  /// while a registration from `registerFont` holds it.
  fn drop_unheld(&self, cache: &Cache, id: u64) {
    let held = self
      .registrations()
      .any(|registration| registration.id() == id)
      || self.files.values().any(|&file| file == id);

    if !held {
      cache.fonts().forget(id);

      return;
    }

    let pinned = self
      .registrations()
      .any(|registration| registration.id() == id && registration.pinned);

    cache.fonts().pin(id, pinned);
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

  use takumi_core::resources::{
    cache::CacheOptions,
    font::{FontOverride, FontResource},
    unicode_range::{UnicodeRange, UnicodeRangeSet},
  };

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

  /// `bytes` as a font of `family`.
  fn named(family: &str, bytes: Vec<u8>) -> FontResource<'static> {
    FontResource::new(bytes).override_info(FontOverride {
      family_name: Some(family.into()),
      ..Default::default()
    })
  }

  /// `resource` registered under `key`, its lease dropped at once.
  fn register(
    fonts: &mut Fonts,
    registrations: &mut FontRegistrations,
    cache: &Cache,
    key: &str,
    resource: FontResource<'static>,
  ) {
    let registered = register_resource(fonts, resource).unwrap();

    drop(registrations.admit(fonts, cache, Some(key), registered, Retention::Evictable));
    registrations.release(fonts, &cache.fonts().take_evicted());
  }

  fn duo() -> Vec<u8> {
    fs::read(concat!(
      env!("CARGO_MANIFEST_DIR"),
      "/../assets/fonts/geist/GeistDuo.ttc"
    ))
    .unwrap()
  }

  fn source(resource: FontResource<'static>) -> RegisteredSource {
    resource
      .into_resolved()
      .unwrap()
      .registered_source()
      .unwrap()
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
        named("Inter", geist(tail)),
      );
    }

    let held = registrations.by_key.len();
    let registered = (0..64)
      .filter(|&tail| {
        fonts
          .clone()
          .unregister_source("Inter", &source(named("Inter", geist(tail))))
      })
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

    register(
      &mut fonts,
      &mut registrations,
      &cache,
      "a",
      named("A", geist(0)),
    );
    register(
      &mut fonts,
      &mut registrations,
      &cache,
      "b",
      named("B", geist(0)),
    );

    assert_eq!(cache.stats().fonts.current, geist(0).len() as u64);
  }

  #[test]
  fn a_key_registered_again_releases_the_file_it_held() {
    let cache = Cache::default();
    let mut fonts = Fonts::default();
    let mut registrations = FontRegistrations::default();

    register(
      &mut fonts,
      &mut registrations,
      &cache,
      "k",
      named("A", geist(0)),
    );
    register(
      &mut fonts,
      &mut registrations,
      &cache,
      "k",
      named("B", geist(1)),
    );

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
    let (_, held) = registrations.admit(
      &mut fonts,
      &cache,
      Some("k"),
      registered,
      Retention::Evictable,
    );

    leases.hold(Some(lease), held);
    registrations.release(&mut fonts, &cache.fonts().take_evicted());

    assert!(registrations.lease(&cache, "k").is_some());

    leases.close(lease);
    registrations.release(&mut fonts, &cache.fonts().take_evicted());

    assert!(registrations.families("k").is_none());
    assert_eq!(cache.stats().fonts.current, 0);
  }

  #[test]
  fn forgetting_unleased_keys_skips_a_leased_file() {
    let cache = Cache::default();
    let mut fonts = Fonts::default();
    let mut registrations = FontRegistrations::default();

    register(
      &mut fonts,
      &mut registrations,
      &cache,
      "free",
      "A",
      geist(0),
    );

    let registered = register_resource(&mut fonts, FontResource::new(geist(1))).unwrap();
    let (_, lease) = registrations.admit(&mut fonts, &cache, Some("leased"), registered, false);
    let keys = ["free", "leased", "absent"].map(String::from);

    assert_eq!(
      registrations.forget_unleased(&mut fonts, &cache, &keys),
      ["free", "absent"]
    );
    assert!(registrations.families("free").is_none());
    assert!(!fonts.unregister_family("A"));
    assert!(registrations.families("leased").is_some());

    drop(lease);

    assert_eq!(
      registrations.forget_unleased(&mut fonts, &cache, &keys[1..2]),
      ["leased"]
    );
    assert_eq!(cache.stats().fonts.current, 0);
  }

  #[test]
  fn a_pinned_registration_stays_past_the_budget() {
    let cache = Cache::builder().fonts(0).build();
    let mut fonts = Fonts::default();
    let mut registrations = FontRegistrations::default();

    register(
      &mut fonts,
      &mut registrations,
      &cache,
      "k",
      named("A", geist(0)),
    );

    let registered = register_resource(&mut fonts, FontResource::new(geist(1))).unwrap();

    drop(registrations.admit(
      &mut fonts,
      &cache,
      Some("pinned"),
      registered,
      Retention::Pinned,
    ));
    registrations.release(&mut fonts, &cache.fonts().take_evicted());

    assert!(registrations.families("k").is_none());
    assert!(registrations.lease(&cache, "pinned").is_some());

    registrations.forget_family(&cache, "geist");

    assert!(!cache.fonts().holds(id(geist(1))));
  }

  #[test]
  fn a_replaced_key_removes_its_faces_while_another_key_holds_the_file() {
    let cache = Cache::default();
    let mut fonts = Fonts::default();
    let mut registrations = FontRegistrations::default();

    register(
      &mut fonts,
      &mut registrations,
      &cache,
      "a",
      named("A", geist(0)),
    );
    register(
      &mut fonts,
      &mut registrations,
      &cache,
      "b",
      named("B", geist(0)),
    );
    register(
      &mut fonts,
      &mut registrations,
      &cache,
      "a",
      named("C", geist(1)),
    );

    assert!(!fonts.clone().unregister_family("A"));
    assert!(fonts.clone().unregister_family("B"));
    assert!(cache.fonts().holds(id(geist(0))));
  }

  /// `[X latin, Y, X cyrillic]`, each subset of X from one file, already in the order they were
  /// registered.
  #[test]
  fn each_registration_of_one_file_keeps_its_own_place() {
    let cache = Cache::default();
    let mut fonts = Fonts::default();
    let mut registrations = FontRegistrations::default();
    let subset = |from, to| {
      named("X", geist(0)).unicode_range(UnicodeRangeSet::from_iter([UnicodeRange::new(from, to)]))
    };

    register(
      &mut fonts,
      &mut registrations,
      &cache,
      "x latin",
      subset(0, 0x24f),
    );
    register(
      &mut fonts,
      &mut registrations,
      &cache,
      "y",
      named("X", geist(1)),
    );
    register(
      &mut fonts,
      &mut registrations,
      &cache,
      "x cyrillic",
      subset(0x400, 0x4ff),
    );

    let keys = ["x latin", "y", "x cyrillic"].map(String::from);

    assert!(!registrations.prefer(&mut fonts, &keys));
  }

  #[test]
  fn unregistering_one_family_of_a_collection_keeps_the_other_held() {
    let cache = Cache::default();
    let mut fonts = Fonts::default();
    let mut registrations = FontRegistrations::default();

    register(
      &mut fonts,
      &mut registrations,
      &cache,
      "duo",
      FontResource::new(duo()),
    );

    let families: Vec<String> = registrations
      .families("duo")
      .unwrap()
      .iter()
      .map(|family| family.name.clone())
      .collect();
    let [removed, kept] = families.as_slice() else {
      panic!("{families:?}");
    };

    registrations.forget_family(&cache, removed);
    fonts.unregister_family(removed);

    assert!(registrations.families("duo").is_none());
    assert!(cache.fonts().holds(id(duo())));

    let mut options = CacheOptions::default();

    options.fonts = Some(0);
    cache.set_options(options);
    registrations.release(&mut fonts, &cache.fonts().take_evicted());

    assert!(!fonts.clone().unregister_family(kept));

    cache.set_options(CacheOptions::default());
    register(
      &mut fonts,
      &mut registrations,
      &cache,
      "duo",
      FontResource::new(duo()),
    );

    assert!(fonts.clone().unregister_family(removed));
  }
}
