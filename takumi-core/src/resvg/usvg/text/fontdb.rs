//! Stand-in for the `fontdb` crate, backed by the faces already registered in
//! the renderer's fontique collection. Font bytes are shared `Blob`s, so the
//! tree stays `Send + Sync` and no second font store is built.

use std::collections::HashMap;
use std::fmt;

use parley::fontique::Blob;

/// Identifier of a face inside a [`Database`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ID(pub(crate) u32);

/// CSS font stretch keywords.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub enum Stretch {
  UltraCondensed,
  ExtraCondensed,
  Condensed,
  SemiCondensed,
  #[default]
  Normal,
  SemiExpanded,
  Expanded,
  ExtraExpanded,
  UltraExpanded,
}

/// CSS font style.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Style {
  #[default]
  Normal,
  Italic,
  Oblique,
}

/// CSS font weight.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Weight(pub u16);

/// A font family in a [`Query`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Family<'a> {
  Serif,
  SansSerif,
  Cursive,
  Fantasy,
  Monospace,
  Name(&'a str),
}

/// A face selection request, mirroring `fontdb::Query`.
#[derive(Debug, Clone, Copy)]
pub struct Query<'a> {
  pub families: &'a [Family<'a>],
  pub weight: Weight,
  pub stretch: Stretch,
  pub style: Style,
}

/// A single face extracted from the fontique collection.
pub struct FaceInfo {
  pub id: ID,
  /// Primary family name the face was registered under.
  pub family: String,
  pub style: Style,
  pub weight: Weight,
  pub stretch: Stretch,
  pub(crate) has_opsz: bool,
  data: Blob<u8>,
  index: u32,
}

impl fmt::Debug for FaceInfo {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.debug_struct("FaceInfo")
      .field("id", &self.id)
      .field("family", &self.family)
      .finish_non_exhaustive()
  }
}

/// Face store consulted while converting `<text>` elements, mirroring the
/// `fontdb::Database` surface the vendored layout code uses.
///
/// Family names fold case ASCII-only (a deliberate size tradeoff over the
/// Unicode caseless matching CSS specifies): a non-ASCII name that differs
/// from its registration only in case misses and falls to the fallback face.
#[derive(Debug, Default)]
pub struct Database {
  faces: Vec<FaceInfo>,
  /// Face indices per lowercased family name, in registration order.
  by_family: HashMap<String, Vec<usize>>,
  /// Face indices per generic family, `Family` discriminant order.
  generic: [Vec<usize>; 5],
}

impl Database {
  pub(crate) fn push_face(
    &mut self,
    family: String,
    style: Style,
    weight: Weight,
    stretch: Stretch,
    has_opsz: bool,
    data: Blob<u8>,
    index: u32,
  ) -> ID {
    let key = family.to_ascii_lowercase();
    let existing = self
      .faces
      .iter()
      .position(|face| face.data.data().as_ptr() == data.data().as_ptr() && face.index == index);

    let position = existing.unwrap_or_else(|| {
      let id = ID(self.faces.len() as u32);

      self.faces.push(FaceInfo {
        id,
        family,
        style,
        weight,
        stretch,
        has_opsz,
        data,
        index,
      });
      self.faces.len() - 1
    });

    let bucket = self.by_family.entry(key).or_default();

    if !bucket.contains(&position) {
      bucket.push(position);
    }

    self.faces[position].id
  }

  pub(crate) fn register_generic(&mut self, family: Family<'_>, name: &str) {
    let Some(bucket) = self.by_family.get(&name.to_ascii_lowercase()) else {
      return;
    };
    let slot = generic_slot(family);

    for index in bucket {
      if !self.generic[slot].contains(index) {
        self.generic[slot].push(*index);
      }
    }
  }

  /// Selects the face closest to `query`, walking its families in order.
  pub fn query(&self, query: &Query) -> Option<ID> {
    for family in query.families {
      let candidates = match family {
        Family::Name(name) => self.by_family.get(&name.to_ascii_lowercase()),
        generic => Some(&self.generic[generic_slot(*generic)]),
      };

      let best = candidates
        .into_iter()
        .flatten()
        .min_by_key(|&&index| score(&self.faces[index], query));

      if let Some(&index) = best {
        return Some(self.faces[index].id);
      }
    }

    None
  }

  /// Runs `f` over the face's raw bytes and TTC index.
  pub fn with_face_data<P, T>(&self, id: ID, f: P) -> Option<T>
  where
    P: FnOnce(&[u8], u32) -> T,
  {
    let face = self.face(id)?;

    Some(f(face.data.data(), face.index))
  }

  pub fn face(&self, id: ID) -> Option<&FaceInfo> {
    self.faces.get(id.0 as usize)
  }

  /// All faces in registration order.
  pub fn faces(&self) -> impl Iterator<Item = &FaceInfo> {
    self.faces.iter()
  }
}

fn generic_slot(family: Family<'_>) -> usize {
  match family {
    Family::Serif => 0,
    Family::SansSerif => 1,
    Family::Cursive => 2,
    Family::Fantasy => 3,
    Family::Monospace => 4,
    Family::Name(_) => unreachable!("named families use the by_family index"),
  }
}

/// Distance from the query; lower is better. Stretch dominates, then style,
/// then weight, per the CSS font matching order.
fn score(face: &FaceInfo, query: &Query) -> u32 {
  let stretch = (face.stretch as i32 - query.stretch as i32).unsigned_abs() * 1_000_000;

  let style = match (query.style, face.style) {
    (a, b) if a == b => 0,
    (Style::Italic, Style::Oblique) | (Style::Oblique, Style::Italic) => 10_000,
    _ => 20_000,
  };

  let weight = (i32::from(face.weight.0) - i32::from(query.weight.0)).unsigned_abs();

  stretch + style + weight
}
