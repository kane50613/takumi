---
packages:
  takumi-core: patch
---

# Clamp with `-webkit-line-clamp` only in a vertical `-webkit-box`

As in browsers, `-webkit-line-clamp` now cuts lines only together with `display: -webkit-box` and `-webkit-box-orient: vertical`. Use `line-clamp` to cut lines on any block.
