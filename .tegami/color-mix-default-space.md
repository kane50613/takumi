---
packages:
  takumi-core: patch
---

# `color-mix()` without `in <space>`

`color-mix(red, blue)` now mixes in Oklab as in Chrome, and a mix in Oklab serializes without the `in oklab,` it implies.
