---
packages:
  takumi-core: patch
---

# Content clipped by `overflow: hidden` no longer adds pages

A box with `overflow` other than `visible` on the block axis that ends on a page used to keep breaking its overflowing children, so the clipped content added blank pages after it. The box now stops breaking on the page where it ends, as Chromium does, and its clipped content stays on that page.
