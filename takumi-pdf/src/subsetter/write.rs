/// A writable stream of binary data.
pub struct Writer(Vec<u8>);

impl Writer {
  /// Create a new writable stream of binary data.
  #[inline]
  pub fn new() -> Self {
    Self(Vec::with_capacity(1024))
  }

  /// Create a new writable stream of binary data with a capacity.
  #[inline]
  pub fn with_capacity(capacity: usize) -> Self {
    Self(Vec::with_capacity(capacity))
  }

  /// Write `T` into the data.
  #[inline]
  pub fn write<T: Writeable>(&mut self, data: T) {
    data.write(self);
  }

  /// Give bytes into the writer.
  #[inline]
  pub fn extend(&mut self, bytes: &[u8]) {
    self.0.extend(bytes);
  }

  /// Align the contents to a byte boundary.
  #[inline]
  pub fn align(&mut self, to: usize) {
    while self.0.len() % to != 0 {
      self.0.push(0);
    }
  }

  /// The number of written bytes.
  #[inline]
  pub fn len(&self) -> usize {
    self.0.len()
  }

  /// Return the written bytes.
  #[inline]
  pub fn finish(self) -> Vec<u8> {
    self.0
  }
}

/// Trait for an object that can be written into a byte stream.
pub trait Writeable: Sized {
  fn write(&self, w: &mut Writer);
}

impl<T: Writeable, const N: usize> Writeable for [T; N] {
  fn write(&self, w: &mut Writer) {
    for i in self {
      w.write(i);
    }
  }
}

impl Writeable for u8 {
  fn write(&self, w: &mut Writer) {
    w.extend(&self.to_be_bytes());
  }
}

impl<T> Writeable for &[T]
where
  T: Writeable,
{
  fn write(&self, w: &mut Writer) {
    for el in *self {
      w.write(el);
    }
  }
}

impl<T> Writeable for &T
where
  T: Writeable,
{
  fn write(&self, w: &mut Writer) {
    T::write(self, w)
  }
}

impl Writeable for u16 {
  fn write(&self, w: &mut Writer) {
    w.write::<[u8; 2]>(self.to_be_bytes());
  }
}

impl Writeable for i16 {
  fn write(&self, w: &mut Writer) {
    w.write::<[u8; 2]>(self.to_be_bytes());
  }
}

impl Writeable for u32 {
  fn write(&self, w: &mut Writer) {
    w.write::<[u8; 4]>(self.to_be_bytes());
  }
}

impl Writeable for i32 {
  fn write(&self, w: &mut Writer) {
    w.write::<[u8; 4]>(self.to_be_bytes());
  }
}
