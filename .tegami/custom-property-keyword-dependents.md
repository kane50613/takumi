---
packages:
  takumi-core: patch
---

# Custom properties that read a substituted keyword

A custom property that reads one whose `var()` substituted `inherit`, `initial`, or `unset` now reads the value it resolved to, as in Chrome, where it copied the keyword.
