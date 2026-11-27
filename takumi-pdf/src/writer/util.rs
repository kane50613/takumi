use std::any::Any;
use std::fmt;
use std::fmt::Debug;
use std::hash::{Hash, Hasher};
use std::ops::Deref;

use crate::writer::geom::Rect;
use crate::writer::graphics::paint::Stroke;
use base64::Engine;
pub(crate) use deferred::*;
use pdf_writer::Name;
use siphasher::sip128::{Hasher128, SipHasher13};
use takumi_core::path::PathBuilder as CorePathBuilder;
use tiny_skia_path::{Path, PathSegment};

pub(crate) trait NameExt {
  fn to_pdf_name(&self) -> Name<'_>;
}

impl NameExt for String {
  fn to_pdf_name(&self) -> Name<'_> {
    Name(self.as_bytes())
  }
}

impl NameExt for &str {
  fn to_pdf_name(&self) -> Name<'_> {
    Name(self.as_bytes())
  }
}

/// The bounds of `stroke` along `path` from core's Skia stroker, measured undashed since the viewer
/// dashes it and the solid outline holds every dash.
pub(crate) fn calculate_stroke_bbox(stroke: &Stroke, path: &Path) -> Option<Rect> {
  let mut builder = CorePathBuilder::new();

  for segment in path.segments() {
    match segment {
      PathSegment::MoveTo(to) => builder.move_to(to.x, to.y),
      PathSegment::LineTo(to) => builder.line_to(to.x, to.y),
      PathSegment::QuadTo(control, to) => builder.quad_to(control.x, control.y, to.x, to.y),
      PathSegment::CubicTo(first, second, to) => {
        builder.cubic_to(first.x, first.y, second.x, second.y, to.x, to.y);
      }
      PathSegment::Close => builder.close(),
    }
  }

  let bounds = builder
    .finish()?
    .stroke(&stroke.core_outline(), 1.0)?
    .compute_tight_bounds()?;

  Some(Rect::from_tsp(tiny_skia_path::Rect::from_ltrb(
    bounds.left(),
    bounds.top(),
    bounds.right(),
    bounds.bottom(),
  )?))
}

pub(crate) struct Prehashed<T: ?Sized> {
  hash: u128,
  value: T,
}

impl<T: Hash + 'static> Prehashed<T> {
  #[inline]
  pub fn new(value: T) -> Self {
    let hash = value.sip_hash();
    Self { hash, value }
  }
}

impl<T: Hash + ?Sized + 'static> Eq for Prehashed<T> {}

impl<T: Hash + ?Sized + 'static> PartialEq for Prehashed<T> {
  #[inline]
  fn eq(&self, other: &Self) -> bool {
    self.hash == other.hash
  }
}

impl<T: ?Sized> Deref for Prehashed<T> {
  type Target = T;

  #[inline]
  fn deref(&self) -> &Self::Target {
    &self.value
  }
}

impl<T: Hash + Clone + 'static> Clone for Prehashed<T> {
  fn clone(&self) -> Self {
    Self {
      hash: self.hash,
      value: self.value.clone(),
    }
  }
}

impl<T: Debug> Debug for Prehashed<T> {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    self.value.fmt(f)
  }
}

impl<T: Hash + ?Sized + 'static> Hash for Prehashed<T> {
  #[inline]
  fn hash<H: Hasher>(&self, state: &mut H) {
    state.write_u128(self.hash);
  }
}

/// Extra methods for [`[T]`](slice).
pub(crate) trait SliceExt<T> {
  /// Split a slice into consecutive runs with the same key and yield for
  /// each such run the key and the slice of elements with that key.
  fn group_by_key<K, F>(&self, f: F) -> GroupByKey<'_, T, F>
  where
    F: FnMut(&T) -> K,
    K: PartialEq;
}

impl<T> SliceExt<T> for [T] {
  fn group_by_key<K, F>(&self, f: F) -> GroupByKey<'_, T, F> {
    GroupByKey { slice: self, f }
  }
}

/// This struct is created by [`SliceExt::group_by_key`].
pub(crate) struct GroupByKey<'a, T, F> {
  slice: &'a [T],
  f: F,
}

impl<'a, T, K, F> Iterator for GroupByKey<'a, T, F>
where
  F: FnMut(&T) -> K,
  K: PartialEq,
{
  type Item = (K, &'a [T]);

  fn next(&mut self) -> Option<Self::Item> {
    let mut iter = self.slice.iter();
    let key = (self.f)(iter.next()?);
    let count = 1 + iter.take_while(|t| (self.f)(t) == key).count();
    let (head, tail) = self.slice.split_at(count);
    self.slice = tail;
    Some((key, head))
  }
}

pub(crate) trait SipHashable {
  fn sip_hash(&self) -> u128;
}

impl<T> SipHashable for T
where
  T: Hash + ?Sized + 'static,
{
  fn sip_hash(&self) -> u128 {
    let mut state = SipHasher13::new();
    self.type_id().hash(&mut state);
    self.hash(&mut state);
    state.finish128().as_u128()
  }
}

/// Create a base64-encoded hash of the value.
pub(crate) fn stable_hash_base64<T: Hash + ?Sized>(value: &T) -> String {
  base64::engine::general_purpose::STANDARD.encode(stable_hash128(value).to_be_bytes())
}

/// Calculate a 128-bit siphash of a value.
pub(crate) fn stable_hash128<T: Hash + ?Sized>(value: &T) -> u128 {
  /// A hasher that hashes usize as u64 to produce stable hashes between
  /// 32-bit and 64-bit architectures.
  struct StableHasher(SipHasher13);

  impl Hasher for StableHasher {
    fn finish(&self) -> u64 {
      self.0.finish()
    }

    fn write(&mut self, bytes: &[u8]) {
      self.0.write(bytes);
    }

    fn write_usize(&mut self, i: usize) {
      self.0.write_u64(i as u64);
    }
  }

  let mut state = StableHasher(SipHasher13::new());
  value.hash(&mut state);
  state.0.finish128().as_u128()
}

mod deferred {
  pub(crate) struct Deferred<T>(T);

  impl<T: Send + Sync + 'static> Deferred<T> {
    pub fn new<F>(f: F) -> Self
    where
      F: FnOnce() -> T + Send + Sync + 'static,
    {
      Self(f())
    }

    pub fn wait(&self) -> &T {
      &self.0
    }
  }
}

#[cfg(test)]
pub(crate) mod test_utils {
  use std::path::PathBuf;
  use std::sync::Arc;
  use std::sync::LazyLock;

  use crate::writer::Data;

  static FONT_PATH: LazyLock<PathBuf> =
    LazyLock::new(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../assets/fonts/noto-sans"));

  macro_rules! lazy_font {
    ($name:ident, $path:expr) => {
      pub static $name: LazyLock<Data> =
        LazyLock::new(|| Arc::new(std::fs::read($path).unwrap()).into());
    };
  }

  lazy_font!(
    NOTO_COLOR_EMOJI_COLR,
    FONT_PATH.join("NotoColorEmoji.COLR.subset.ttf")
  );
}

#[cfg(test)]
mod tests {
  use tiny_skia_path::PathBuilder;

  use super::*;
  use crate::writer::graphics::paint::StrokeDash;

  #[test]
  fn a_dashed_stroke_is_bounded_by_its_whole_outline() {
    let mut builder = PathBuilder::new();

    builder.move_to(0.0, 0.0);
    builder.line_to(100.0, 0.0);

    let path = builder.finish().unwrap();
    let stroke = Stroke {
      dash: Some(StrokeDash {
        array: vec![5.0, 95.0],
        offset: 0.0,
      }),
      ..Stroke::default()
    };

    let bounds = calculate_stroke_bbox(&stroke, &path).unwrap();

    assert_eq!((bounds.left(), bounds.right()), (0.0, 100.0));
  }
}
