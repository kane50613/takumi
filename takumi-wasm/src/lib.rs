//! WebAssembly bindings for Takumi.

#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]
#![deny(missing_docs)]

mod helper;
mod model;
mod renderer;

pub use helper::*;
pub use model::*;
pub use renderer::*;
use wasm_bindgen::prelude::wasm_bindgen;

/// The heap, grown a part of the memory at a time.
#[cfg(all(target_arch = "wasm32", not(target_feature = "atomics")))]
#[global_allocator]
static HEAP: takumi_bindings_common::heap::Heap = takumi_bindings_common::heap::Heap::new();

#[wasm_bindgen(typescript_custom_section)]
const TS_APPEND_CONTENT: &'static str = include_str!("./dts-header.d.ts");
