//! Blink's `SimpleFontData`: one face at one size, as shaping takes it.

use parley::fontique::Blob;

/// Blink's `SimpleFontData`. Runs compare it by identity, as Blink compares the pointer.
#[derive(Debug)]
pub(crate) struct SimpleFontData {
  blob: Blob<u8>,
  index: u32,
  size: f32,
}

impl SimpleFontData {
  pub(crate) fn new(blob: Blob<u8>, index: u32, size: f32) -> Self {
    Self { blob, index, size }
  }

  /// The font file.
  pub(crate) fn blob(&self) -> &Blob<u8> {
    &self.blob
  }

  /// The face's index within its collection.
  pub(crate) fn index(&self) -> u32 {
    self.index
  }

  /// The size in pixels.
  pub(crate) fn size(&self) -> f32 {
    self.size
  }
}
