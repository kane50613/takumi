use image::{
  ImageError, RgbaImage,
  error::{ParameterError, ParameterErrorKind},
};

use crate::Result;

const BUCKET_COUNT: usize = 32;

/// A pool of reusable RGBA image buffers to avoid repeated heap allocations.
pub(crate) struct BufferPool {
  pools: [Vec<Vec<u8>>; BUCKET_COUNT],
  u32_pools: [Vec<Vec<u32>>; BUCKET_COUNT],
  current_size: usize,
  max_size: usize,
}

impl Default for BufferPool {
  fn default() -> Self {
    const EMPTY_VEC: Vec<Vec<u8>> = Vec::new();
    const EMPTY_U32_VEC: Vec<Vec<u32>> = Vec::new();
    Self {
      pools: [EMPTY_VEC; BUCKET_COUNT],
      u32_pools: [EMPTY_U32_VEC; BUCKET_COUNT],
      current_size: 0,
      max_size: 64 * 1024 * 1024,
    }
  }
}

impl BufferPool {
  fn bucket_index(capacity: usize) -> usize {
    if capacity == 0 {
      return 0;
    }
    capacity.next_power_of_two().trailing_zeros() as usize
  }

  pub(crate) fn acquire(&mut self, capacity: usize) -> Vec<u8> {
    let mut index = Self::bucket_index(capacity);
    if index >= BUCKET_COUNT {
      index = BUCKET_COUNT - 1;
    }

    for i in index..BUCKET_COUNT {
      if let Some(mut buf) = self.pools[i].pop() {
        self.current_size -= buf.capacity();
        buf.clear();
        buf.resize(capacity, 0);
        return buf;
      }
    }

    let alloc_cap = (1_usize.checked_shl(index as u32).unwrap_or(capacity)).max(capacity);
    let mut buf = Vec::with_capacity(alloc_cap);
    buf.resize(capacity, 0);
    buf
  }

  #[allow(clippy::uninit_vec)]
  pub(crate) fn acquire_dirty(&mut self, capacity: usize) -> Vec<u8> {
    let mut index = Self::bucket_index(capacity);
    if index >= BUCKET_COUNT {
      index = BUCKET_COUNT - 1;
    }

    for i in index..BUCKET_COUNT {
      if let Some(mut buf) = self.pools[i].pop() {
        self.current_size -= buf.capacity();
        buf.clear();
        unsafe {
          buf.set_len(capacity);
        }
        return buf;
      }
    }

    let alloc_cap = (1_usize.checked_shl(index as u32).unwrap_or(capacity)).max(capacity);
    let mut buf = Vec::with_capacity(alloc_cap);
    unsafe {
      buf.set_len(capacity);
    }
    buf
  }

  pub(crate) fn release(&mut self, buffer: Vec<u8>) {
    if buffer.capacity() == 0 {
      return;
    }

    let cap = buffer.capacity();
    if self.current_size + cap > self.max_size {
      return;
    }

    let mut index = Self::bucket_index(cap);
    if index >= BUCKET_COUNT {
      index = BUCKET_COUNT - 1;
    }

    self.current_size += cap;
    self.pools[index].push(buffer);
  }

  pub(crate) fn acquire_u32(&mut self, capacity: usize) -> Vec<u32> {
    let mut index = Self::bucket_index(capacity);
    if index >= BUCKET_COUNT {
      index = BUCKET_COUNT - 1;
    }

    for i in index..BUCKET_COUNT {
      if let Some(mut buf) = self.u32_pools[i].pop() {
        self.current_size -= buf.capacity() * size_of::<u32>();
        buf.clear();
        buf.resize(capacity, 0);
        return buf;
      }
    }

    let alloc_cap = (1_usize.checked_shl(index as u32).unwrap_or(capacity)).max(capacity);
    let mut buf = Vec::with_capacity(alloc_cap);
    buf.resize(capacity, 0);
    buf
  }

  pub(crate) fn release_u32(&mut self, buffer: Vec<u32>) {
    if buffer.capacity() == 0 {
      return;
    }

    let cap_bytes = buffer.capacity() * size_of::<u32>();
    if self.current_size + cap_bytes > self.max_size {
      return;
    }

    let mut index = Self::bucket_index(buffer.capacity());
    if index >= BUCKET_COUNT {
      index = BUCKET_COUNT - 1;
    }

    self.current_size += cap_bytes;
    self.u32_pools[index].push(buffer);
  }

  pub(crate) fn acquire_image(&mut self, width: u32, height: u32) -> Result<RgbaImage> {
    let raw = self.acquire((width * height * 4) as usize);

    RgbaImage::from_raw(width, height, raw).ok_or_else(|| {
      ImageError::Parameter(ParameterError::from_kind(
        ParameterErrorKind::DimensionMismatch,
      ))
      .into()
    })
  }

  pub(crate) fn release_image(&mut self, image: RgbaImage) {
    self.release(image.into_raw());
  }
}
