//! Blink's [exclusions](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/layout/exclusions/):
//! the floats in a block formatting context, and the space they leave, without `shape-outside`
//! or block fragmentation.

pub(crate) mod exclusion_area;
pub(crate) mod exclusion_space;
pub(crate) mod layout_opportunity;
