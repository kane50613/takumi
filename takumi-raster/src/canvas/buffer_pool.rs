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

#[inline]
fn bucket_index_clamped(capacity: usize) -> usize {
  let raw = if capacity == 0 {
    0
  } else {
    capacity.next_power_of_two().trailing_zeros() as usize
  };
  raw.min(BUCKET_COUNT - 1)
}

#[inline]
fn pop_from_bucket<T>(pools: &mut [Vec<Vec<T>>; BUCKET_COUNT], index: usize) -> Option<Vec<T>> {
  pools[index..].iter_mut().find_map(Vec::pop)
}

#[inline]
fn allocate_capacity(index: usize, capacity: usize) -> usize {
  (1_usize.checked_shl(index as u32).unwrap_or(capacity)).max(capacity)
}

impl BufferPool {
  pub(crate) fn acquire(&mut self, capacity: usize) -> Vec<u8> {
    let index = bucket_index_clamped(capacity);
    if let Some(mut buf) = pop_from_bucket(&mut self.pools, index) {
      self.current_size -= buf.capacity();
      buf.clear();
      buf.resize(capacity, 0);
      return buf;
    }
    let mut buf = Vec::with_capacity(allocate_capacity(index, capacity));
    buf.resize(capacity, 0);
    buf
  }

  #[allow(clippy::uninit_vec)]
  pub(crate) fn acquire_dirty(&mut self, capacity: usize) -> Vec<u8> {
    let index = bucket_index_clamped(capacity);
    let mut buf = match pop_from_bucket(&mut self.pools, index) {
      Some(b) => {
        self.current_size -= b.capacity();
        b
      }
      None => Vec::with_capacity(allocate_capacity(index, capacity)),
    };
    buf.clear();
    unsafe {
      buf.set_len(capacity);
    }
    buf
  }

  pub(crate) fn release(&mut self, buffer: Vec<u8>) {
    let cap = buffer.capacity();
    if cap == 0 || self.current_size + cap > self.max_size {
      return;
    }
    self.current_size += cap;
    self.pools[bucket_index_clamped(cap)].push(buffer);
  }

  pub(crate) fn acquire_u32(&mut self, capacity: usize) -> Vec<u32> {
    let index = bucket_index_clamped(capacity);
    if let Some(mut buf) = pop_from_bucket(&mut self.u32_pools, index) {
      self.current_size -= buf.capacity() * size_of::<u32>();
      buf.clear();
      buf.resize(capacity, 0);
      return buf;
    }
    let mut buf = Vec::with_capacity(allocate_capacity(index, capacity));
    buf.resize(capacity, 0);
    buf
  }

  pub(crate) fn release_u32(&mut self, buffer: Vec<u32>) {
    let cap = buffer.capacity();
    if cap == 0 {
      return;
    }
    let cap_bytes = cap * size_of::<u32>();
    if self.current_size + cap_bytes > self.max_size {
      return;
    }
    self.current_size += cap_bytes;
    self.u32_pools[bucket_index_clamped(cap)].push(buffer);
  }
}
