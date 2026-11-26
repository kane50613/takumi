---
packages:
  takumi-core: patch
---

# A registered custom property in a cycle with `font-size` takes its inherited value

A registered custom property whose font-relative units form a cycle with `font-size` or `line-height` now takes the parent's value when it inherits, as in Chrome, instead of its initial value.
