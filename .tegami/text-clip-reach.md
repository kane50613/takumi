---
packages:
  "takumi": patch
  "takumi-pdf": patch
---

# Match Chrome on which boxes a `background-clip: text` background shows through

The background now shows through the text of a box inside it at zero opacity or under a `scale(0)` transform, as in Chrome. It no longer shows through a float with its own opacity, transform or position, or an absolutely positioned box whose containing block is outside it, which Chrome leaves out.
