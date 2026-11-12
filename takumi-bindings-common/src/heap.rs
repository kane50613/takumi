//! The heap a wasm binding allocates from: the standard library's dlmalloc, grown a part of the
//! memory at a time. Each `memory.grow` costs about 100 µs in V8, and growing only what one
//! allocation needs, as the standard library does, takes tens of thousands of them for a long
//! document.

use core::{
  alloc::{GlobalAlloc, Layout},
  arch::wasm32,
  cell::UnsafeCell,
  ptr,
};

use dlmalloc::{Allocator, Dlmalloc};

/// A WebAssembly page.
const PAGE: usize = 64 * 1024;

/// The fewest pages a growth adds: 16 MiB.
const MIN_STEP: usize = 256;

/// The most pages a growth adds beyond what an allocation needs: 64 MiB, which bounds the memory
/// grown and never used. A larger step renders no faster, since a 1 GiB heap already grows in about
/// 30 steps.
const MAX_STEP: usize = 1024;

/// A global allocator for a single-threaded wasm binding.
pub struct Heap(UnsafeCell<Dlmalloc<GrowInSteps>>);

// SAFETY: a wasm module without atomics runs on one thread, so no two calls reach the heap at once.
unsafe impl Sync for Heap {}

impl Heap {
  /// The heap, to declare as a binding's `#[global_allocator]`.
  pub const fn new() -> Self {
    Self(UnsafeCell::new(Dlmalloc::new_with_allocator(GrowInSteps)))
  }
}

impl Default for Heap {
  fn default() -> Self {
    Self::new()
  }
}

// SAFETY: each call forwards to dlmalloc, which upholds `GlobalAlloc`'s contract, and the module
// has one thread, so the `UnsafeCell` is never borrowed twice.
unsafe impl GlobalAlloc for Heap {
  unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
    // SAFETY: see the impl.
    unsafe { (*self.0.get()).malloc(layout.size(), layout.align()) }
  }

  unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
    // SAFETY: see the impl.
    unsafe { (*self.0.get()).calloc(layout.size(), layout.align()) }
  }

  unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
    // SAFETY: see the impl.
    unsafe { (*self.0.get()).free(ptr, layout.size(), layout.align()) }
  }

  unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
    // SAFETY: see the impl.
    unsafe { (*self.0.get()).realloc(ptr, layout.size(), layout.align(), new_size) }
  }
}

/// Grows the memory an eighth at a time, as the standard library's wasm `System` grows only
/// what is asked.
pub struct GrowInSteps;

// SAFETY: `alloc` hands dlmalloc only memory it has just grown, and the memory never shrinks, so
// nothing is released.
unsafe impl Allocator for GrowInSteps {
  fn alloc(&self, size: usize) -> (*mut u8, usize, u32) {
    let needed = size.div_ceil(PAGE);
    let step = (wasm32::memory_size::<0>() / 8).clamp(MIN_STEP, MAX_STEP);

    // A memory near its limit may not take a whole step, but can still take the allocation.
    for pages in [needed.max(step), needed] {
      let previous = wasm32::memory_grow::<0>(pages);

      if previous == usize::MAX {
        continue;
      }

      let base = previous * PAGE;
      let size = pages * PAGE;

      // A region that ends at the top of the address space would wrap a pointer past it to 0.
      return if base.wrapping_add(size) == 0 {
        (base as *mut u8, size - 16, 0)
      } else {
        (base as *mut u8, size, 0)
      };
    }

    (ptr::null_mut(), 0, 0)
  }

  fn remap(&self, _ptr: *mut u8, _old_size: usize, _new_size: usize, _can_move: bool) -> *mut u8 {
    ptr::null_mut()
  }

  fn free_part(&self, _ptr: *mut u8, _old_size: usize, _new_size: usize) -> bool {
    false
  }

  fn free(&self, _ptr: *mut u8, _size: usize) -> bool {
    false
  }

  fn can_release_part(&self, _flags: u32) -> bool {
    false
  }

  fn allocates_zeros(&self) -> bool {
    true
  }

  fn page_size(&self) -> usize {
    PAGE
  }
}
