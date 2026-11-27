//! Premultiplied RGBA8 pixmaps and A8 coverage masks, owned and borrowed.

use takumi_core::geometry::{Rect, Size};
use takumi_core::resources::image_buffer::ImageBuffer;

/// The byte length of a `size` buffer of `bytes_per_pixel` pixels, none where a row's byte count
/// leaves `i32`, as Skia's `SkImageInfo::validRowBytes` requires.
fn byte_len(size: Size<u32>, bytes_per_pixel: usize) -> Option<usize> {
  let row = (size.width as usize).checked_mul(bytes_per_pixel)?;

  i32::try_from(row).ok()?;
  row.checked_mul(size.height as usize)
}

/// A premultiplied RGBA8 image.
#[derive(Clone, PartialEq)]
pub(crate) struct Pixmap {
  data: Vec<u8>,
  size: Size<u32>,
}

/// A borrowed [`Pixmap`].
#[derive(Clone, Copy)]
pub(crate) struct PixmapRef<'a> {
  data: &'a [u8],
  size: Size<u32>,
}

/// A mutably borrowed [`Pixmap`].
pub(crate) struct PixmapMut<'a> {
  data: &'a mut [u8],
  size: Size<u32>,
}

impl Pixmap {
  /// A transparent `width` by `height` pixmap, none where it is empty or too large.
  pub(crate) fn new(width: u32, height: u32) -> Option<Self> {
    let size = Size::checked_new(width, height)?;

    Some(Self {
      data: vec![0; byte_len(size, 4)?],
      size,
    })
  }

  /// A `size` pixmap over `data`, none where its length does not match.
  pub(crate) fn from_vec(data: Vec<u8>, size: Size<u32>) -> Option<Self> {
    (Some(data.len()) == byte_len(size, 4)).then_some(Self { data, size })
  }

  pub(crate) fn width(&self) -> u32 {
    self.size.width
  }

  pub(crate) fn height(&self) -> u32 {
    self.size.height
  }

  pub(crate) fn data(&self) -> &[u8] {
    &self.data
  }

  pub(crate) fn data_mut(&mut self) -> &mut [u8] {
    &mut self.data
  }

  #[cfg(feature = "svg")]
  pub(crate) fn pixels(&self) -> &[[u8; 4]] {
    self.data.as_chunks().0
  }

  pub(crate) fn pixels_mut(&mut self) -> &mut [[u8; 4]] {
    self.data.as_chunks_mut().0
  }

  pub(crate) fn take(self) -> Vec<u8> {
    self.data
  }

  pub(crate) fn as_ref(&self) -> PixmapRef<'_> {
    PixmapRef {
      data: &self.data,
      size: self.size,
    }
  }

  pub(crate) fn as_mut(&mut self) -> PixmapMut<'_> {
    PixmapMut {
      data: &mut self.data,
      size: self.size,
    }
  }

  /// A copy of the pixels `rect` covers, clipped to the pixmap.
  pub(crate) fn clone_rect(&self, rect: Rect<i32>) -> Option<Self> {
    self.as_ref().clone_rect(rect)
  }
}

impl<'a> PixmapRef<'a> {
  /// A `width` by `height` pixmap over `data`, none where its length does not match.
  pub(crate) fn from_bytes(data: &'a [u8], width: u32, height: u32) -> Option<Self> {
    let size = Size::checked_new(width, height)?;

    (Some(data.len()) == byte_len(size, 4)).then_some(Self { data, size })
  }

  pub(crate) fn width(self) -> u32 {
    self.size.width
  }

  pub(crate) fn height(self) -> u32 {
    self.size.height
  }

  pub(crate) fn pixels(self) -> &'a [[u8; 4]] {
    self.data.as_chunks().0
  }

  pub(crate) fn to_owned(self) -> Pixmap {
    Pixmap {
      data: self.data.to_vec(),
      size: self.size,
    }
  }

  /// A copy of the pixels `rect` covers, clipped to the pixmap.
  pub(crate) fn clone_rect(self, rect: Rect<i32>) -> Option<Pixmap> {
    let rect = Rect::checked_from_size(0, 0, self.width(), self.height())?.intersection(&rect)?;
    let mut copy = Pixmap::new(rect.width(), rect.height())?;
    let row = rect.width() as usize * 4;

    for (y, target) in copy.data.chunks_exact_mut(row).enumerate() {
      let start = ((rect.top as usize + y) * self.width() as usize + rect.left as usize) * 4;

      target.copy_from_slice(&self.data[start..start + row]);
    }

    Some(copy)
  }
}

impl<'a> PixmapMut<'a> {
  /// A `width` by `height` pixmap over `data`, none where its length does not match.
  pub(crate) fn from_bytes(data: &'a mut [u8], width: u32, height: u32) -> Option<Self> {
    let size = Size::checked_new(width, height)?;

    (Some(data.len()) == byte_len(size, 4)).then_some(Self { data, size })
  }

  pub(crate) fn width(&self) -> u32 {
    self.size.width
  }

  pub(crate) fn height(&self) -> u32 {
    self.size.height
  }

  pub(crate) fn data_mut(&mut self) -> &mut [u8] {
    self.data
  }

  pub(crate) fn pixels_mut(&mut self) -> &mut [[u8; 4]] {
    self.data.as_chunks_mut().0
  }

  pub(crate) fn as_ref(&self) -> PixmapRef<'_> {
    PixmapRef {
      data: self.data,
      size: self.size,
    }
  }
}

impl<'a> TryFrom<&'a ImageBuffer> for PixmapRef<'a> {
  type Error = ();

  fn try_from(buffer: &'a ImageBuffer) -> Result<Self, Self::Error> {
    PixmapRef::from_bytes(buffer.data(), buffer.width(), buffer.height()).ok_or(())
  }
}

impl TryFrom<&ImageBuffer> for Pixmap {
  type Error = ();

  fn try_from(buffer: &ImageBuffer) -> Result<Self, Self::Error> {
    PixmapRef::try_from(buffer).map(PixmapRef::to_owned)
  }
}

/// An A8 coverage mask.
#[derive(Clone, PartialEq)]
pub(crate) struct Mask {
  data: Vec<u8>,
  size: Size<u32>,
}

impl Mask {
  /// An uncovered `width` by `height` mask, none where it is empty or too large.
  pub(crate) fn new(width: u32, height: u32) -> Option<Self> {
    let size = Size::checked_new(width, height)?;

    Some(Self {
      data: vec![0; byte_len(size, 1)?],
      size,
    })
  }

  /// A `size` mask over `data`, none where its length does not match.
  pub(crate) fn from_vec(data: Vec<u8>, size: Size<u32>) -> Option<Self> {
    (Some(data.len()) == byte_len(size, 1)).then_some(Self { data, size })
  }

  pub(crate) fn width(&self) -> u32 {
    self.size.width
  }

  pub(crate) fn height(&self) -> u32 {
    self.size.height
  }

  pub(crate) fn data(&self) -> &[u8] {
    &self.data
  }

  pub(crate) fn data_mut(&mut self) -> &mut [u8] {
    &mut self.data
  }
}
