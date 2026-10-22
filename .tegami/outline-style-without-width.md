---
packages:
  takumi-core: patch
  takumi: patch
  "@takumi-rs/core": patch
  "@takumi-rs/wasm": patch
---

# Draw an outline that sets only `outline-style`

An element with `outline-style: solid` and no `outline-width` now draws a `medium` (3px) outline, as browsers do. It used to draw no outline unless another outline longhand, such as `outline-color`, was also set.
