---
packages:
  takumi-core: patch
---

# A `currentColor` text decoration follows the text's stroke and fill colours

As in Chrome, a text decoration whose color is `currentColor` takes the stroke colour of stroked text, unless the stroke is `currentColor` or transparent, and the `-webkit-text-fill-color` otherwise. `-webkit-text-stroke-width` also takes `thin`, `medium`, and `thick`.
