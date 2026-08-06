use std::convert::TryInto;

#[derive(Clone, Debug)]
/// A readable stream of binary data.
pub struct Reader<'a> {
  /// The underlying data of the reader.
  data: &'a [u8],
  /// The current offset in bytes. Is not guaranteed to be in range.
  offset: usize,
}

impl<'a> Reader<'a> {
  /// Create a new readable stream of binary data.
  #[inline]
  pub fn new(data: &'a [u8]) -> Self {
    Self { data, offset: 0 }
  }

  /// Create a new readable stream of binary data at a specific position.
  #[inline]
  pub fn new_at(data: &'a [u8], offset: usize) -> Self {
    Self { data, offset }
  }

  /// The remaining data from the current offset.
  #[inline]
  pub fn tail(&self) -> Option<&'a [u8]> {
    self.data.get(self.offset..)
  }

  /// Returns the current offset.
  #[inline]
  pub fn offset(&self) -> usize {
    self.offset
  }

  /// Try to read `T` from the data.
  #[inline]
  pub fn read<T: Readable<'a>>(&mut self) -> Option<T> {
    T::read(self)
  }

  /// Try to read `T` from the data.
  #[inline]
  pub fn peak<T: Readable<'a>>(&mut self) -> Option<T> {
    let mut r = self.clone();
    T::read(&mut r)
  }

  /// Read a certain number of bytes.
  #[inline]
  pub fn read_bytes(&mut self, len: usize) -> Option<&'a [u8]> {
    let end = self.offset.checked_add(len)?;
    let v = self.data.get(self.offset..end)?;
    self.offset = end;
    Some(v)
  }

  /// Reads the next `count` types as a slice.
  #[inline]
  pub fn read_array16<T: Readable<'a>>(&mut self, count: u16) -> Option<LazyArray16<'a, T>> {
    let len = usize::from(count) * T::SIZE;
    self.read_bytes(len).map(LazyArray16::new)
  }

  /// Advances by `Readable::SIZE`.
  #[inline]
  pub fn skip<T: Readable<'a>>(&mut self) {
    self.skip_bytes(T::SIZE);
  }

  /// Check whether the reader is at the end of the buffer.
  #[inline]
  pub fn at_end(&self) -> bool {
    self.offset >= self.data.len()
  }

  /// Jump to a specific location.
  #[inline]
  pub fn jump(&mut self, offset: usize) {
    self.offset = offset;
  }

  /// Skip the next `n` bytes from the stream.
  #[inline]
  pub fn skip_bytes(&mut self, n: usize) {
    self.read_bytes(n);
  }
}

/// Trait for an object that can be read from a byte stream with a fixed size.
pub trait Readable<'a>: Sized {
  const SIZE: usize;

  fn read(r: &mut Reader<'a>) -> Option<Self>;
}

impl<const N: usize> Readable<'_> for [u8; N] {
  const SIZE: usize = N;

  fn read(r: &mut Reader) -> Option<Self> {
    Some(r.read_bytes(N)?.try_into().unwrap_or([0; N]))
  }
}

impl Readable<'_> for u8 {
  const SIZE: usize = 1;

  fn read(r: &mut Reader) -> Option<Self> {
    r.read::<[u8; 1]>().map(Self::from_be_bytes)
  }
}

impl Readable<'_> for u16 {
  const SIZE: usize = 2;

  fn read(r: &mut Reader) -> Option<Self> {
    r.read::<[u8; 2]>().map(Self::from_be_bytes)
  }
}

impl Readable<'_> for i16 {
  const SIZE: usize = 2;

  fn read(r: &mut Reader) -> Option<Self> {
    r.read::<[u8; 2]>().map(Self::from_be_bytes)
  }
}

impl Readable<'_> for u32 {
  const SIZE: usize = 4;

  fn read(r: &mut Reader) -> Option<Self> {
    r.read::<[u8; 4]>().map(Self::from_be_bytes)
  }
}

impl Readable<'_> for i32 {
  const SIZE: usize = 4;

  fn read(r: &mut Reader) -> Option<Self> {
    r.read::<[u8; 4]>().map(Self::from_be_bytes)
  }
}

/// A slice-like container that converts internal binary data only on access.
///
/// Array values are stored in a continuous data chunk.
#[derive(Clone, Copy)]
pub struct LazyArray16<'a, T> {
  data: &'a [u8],
  data_type: core::marker::PhantomData<T>,
}

impl<T> Default for LazyArray16<'_, T> {
  #[inline]
  fn default() -> Self {
    LazyArray16 {
      data: &[],
      data_type: core::marker::PhantomData,
    }
  }
}

impl<'a, T: Readable<'a>> LazyArray16<'a, T> {
  /// Creates a new `LazyArray`.
  #[inline]
  pub fn new(data: &'a [u8]) -> Self {
    LazyArray16 {
      data,
      data_type: core::marker::PhantomData,
    }
  }

  /// Returns a value at `index`.
  #[inline]
  pub fn get(&self, index: u16) -> Option<T> {
    if index < self.len() {
      let start = usize::from(index) * T::SIZE;
      let end = start + T::SIZE;
      self
        .data
        .get(start..end)
        .map(Reader::new)
        .and_then(|mut r| T::read(&mut r))
    } else {
      None
    }
  }

  /// Returns array's length.
  #[inline]
  pub fn len(&self) -> u16 {
    (self.data.len() / T::SIZE) as u16
  }
}

impl<'a, T: Readable<'a> + core::fmt::Debug + Copy> core::fmt::Debug for LazyArray16<'a, T> {
  fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
    f.debug_list().entries(*self).finish()
  }
}

impl<'a, T: Readable<'a>> IntoIterator for LazyArray16<'a, T> {
  type Item = T;
  type IntoIter = LazyArrayIter16<'a, T>;

  #[inline]
  fn into_iter(self) -> Self::IntoIter {
    LazyArrayIter16 {
      data: self,
      index: 0,
    }
  }
}

/// An iterator over `LazyArray16`.
#[derive(Clone, Copy)]
#[allow(missing_debug_implementations)]
pub struct LazyArrayIter16<'a, T> {
  data: LazyArray16<'a, T>,
  index: u16,
}

impl<'a, T: Readable<'a>> Default for LazyArrayIter16<'a, T> {
  #[inline]
  fn default() -> Self {
    LazyArrayIter16 {
      data: LazyArray16::new(&[]),
      index: 0,
    }
  }
}

impl<'a, T: Readable<'a>> Iterator for LazyArrayIter16<'a, T> {
  type Item = T;

  #[inline]
  fn next(&mut self) -> Option<Self::Item> {
    self.index += 1;
    self.data.get(self.index - 1)
  }

  #[inline]
  fn count(self) -> usize {
    usize::from(self.data.len().saturating_sub(self.index))
  }
}
