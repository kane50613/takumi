//! The bound on the faces a long-running renderer keeps, `fontCacheMaxBytes`: decoded font bytes
//! per family, released least recently used first, the policy of `takumi-bindings-common`'s
//! `FontCache` kept next to the registry it releases from.

use std::{collections::HashMap, sync::Arc};

use super::{font_face::FontFace, font_face_cache::fold_family_name};
use crate::resources::font::SharedBytes;

/// What the registrations of one family hold.
#[derive(Clone, Debug)]
struct BudgetedFamily {
  /// Decoded bytes its registrations hold; a file holding several families counts in each.
  bytes: u64,
  /// Added to by an explicit registration, so never released for space.
  pinned: bool,
  last_used: u64,
  keys: Vec<String>,
  font_faces: Vec<Arc<FontFace>>,
}

/// The registrations a renderer recorded, and the budget that bounds the unpinned ones.
#[derive(Clone, Debug)]
pub(crate) struct FontBudget {
  max_bytes: u64,
  by_key: HashMap<String, Vec<Arc<str>>>,
  families: HashMap<String, BudgetedFamily>,
  clock: u64,
}

impl Default for FontBudget {
  fn default() -> Self {
    Self::new(None)
  }
}

impl FontBudget {
  /// A budget keeping at most `max_bytes` of unpinned font data, unbounded without one.
  pub(crate) fn new(max_bytes: Option<u64>) -> Self {
    Self {
      max_bytes: max_bytes.unwrap_or(u64::MAX),
      by_key: HashMap::new(),
      families: HashMap::new(),
      clock: 0,
    }
  }

  /// The families `key` registered, marked used and pinned when `pin` is set; `None` once any of
  /// them was released.
  pub(crate) fn lookup(&mut self, key: &str, pin: bool) -> Option<Vec<Arc<str>>> {
    let registered = self.by_key.get(key)?.clone();
    let now = self.tick();

    for family in &registered {
      let budgeted = self.families.get_mut(&fold_family_name(family))?;

      budgeted.last_used = now;
      budgeted.pinned |= pin;
    }

    Some(registered)
  }

  /// Records the faces one registration under `key` added, each family charged the decoded
  /// bytes of every file the registration read.
  pub(crate) fn record(&mut self, key: Option<&str>, font_faces: &[Arc<FontFace>], pinned: bool) {
    let now = self.tick();
    let mut files: Vec<&SharedBytes> = Vec::new();
    let mut families: Vec<Arc<str>> = Vec::new();

    for font_face in font_faces {
      let data = &font_face.custom_platform_data.data;

      if !files.iter().any(|file| Arc::ptr_eq(file, data)) {
        files.push(data);
      }

      if !families.contains(&font_face.family) {
        families.push(Arc::clone(&font_face.family));
      }
    }

    let bytes: u64 = files
      .iter()
      .map(|file| (***file).as_ref().len() as u64)
      .sum();

    if let Some(key) = key {
      for previous in self.by_key.remove(key).into_iter().flatten() {
        if let Some(budgeted) = self.families.get_mut(&fold_family_name(&previous)) {
          budgeted.keys.retain(|each| each != key);
        }
      }
    }

    for family in &families {
      let budgeted = self
        .families
        .entry(fold_family_name(family))
        .or_insert_with(|| BudgetedFamily {
          bytes: 0,
          pinned,
          last_used: now,
          keys: Vec::new(),
          font_faces: Vec::new(),
        });

      budgeted.bytes += bytes;
      budgeted.pinned |= pinned;
      budgeted.last_used = now;
      budgeted.keys.extend(key.map(str::to_owned));
      budgeted.font_faces.extend(
        font_faces
          .iter()
          .filter(|font_face| font_face.family == *family)
          .cloned(),
      );
    }

    if let Some(key) = key {
      self.by_key.insert(key.to_owned(), families);
    }
  }

  /// Marks `families` used now, as a render that names them starts.
  pub(crate) fn touch(&mut self, families: &[impl AsRef<str>]) {
    let now = self.tick();

    for family in families {
      if let Some(budgeted) = self.families.get_mut(&fold_family_name(family.as_ref())) {
        budgeted.last_used = now;
      }
    }
  }

  /// Forgets the least recently used unpinned families, other than those of `kept`, until the
  /// rest fit, with every family the same registrations added; the faces they held.
  pub(crate) fn release_past_budget(&mut self, kept: &[Arc<FontFace>]) -> Vec<Arc<FontFace>> {
    let keep: Vec<String> = kept
      .iter()
      .map(|font_face| fold_family_name(&font_face.family))
      .collect();
    let mut released = Vec::new();

    while self.unpinned_bytes() > self.max_bytes {
      let Some(oldest) = self
        .families
        .iter()
        .filter(|(folded, family)| !family.pinned && !keep.contains(folded))
        .min_by_key(|(_, family)| family.last_used)
        .map(|(folded, _)| folded.clone())
      else {
        break;
      };

      for family in self.registered_with(&oldest) {
        if keep.contains(&family)
          || self
            .families
            .get(&family)
            .is_some_and(|budgeted| budgeted.pinned)
        {
          continue;
        }

        released.extend(self.forget(&family));
      }
    }

    released
  }

  /// `family` and every family a registration that added to it also added.
  fn registered_with(&self, family: &str) -> Vec<String> {
    let mut names = vec![family.to_owned()];

    for key in self
      .families
      .get(family)
      .map(|budgeted| budgeted.keys.as_slice())
      .unwrap_or_default()
    {
      for registered in self.by_key.get(key).into_iter().flatten() {
        let folded = fold_family_name(registered);

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

  /// Bytes every recorded family holds.
  pub(crate) fn recorded_bytes(&self) -> u64 {
    self.families.values().map(|family| family.bytes).sum()
  }

  /// Forgets `family` and the registrations that added to it, so registering one of them again
  /// is recorded anew; the faces it held.
  pub(crate) fn forget(&mut self, family: &str) -> Vec<Arc<FontFace>> {
    let Some(budgeted) = self.families.remove(&fold_family_name(family)) else {
      return Vec::new();
    };

    for key in &budgeted.keys {
      self.by_key.remove(key);
    }

    budgeted.font_faces
  }

  /// Forgets every registration.
  pub(crate) fn clear(&mut self) {
    self.by_key.clear();
    self.families.clear();
  }

  fn tick(&mut self) -> u64 {
    self.clock += 1;
    self.clock
  }
}
