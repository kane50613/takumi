---
packages:
  takumi-core:
    type: patch
---

### Honor emoji variation selectors in font selection

A codepoint followed by `U+FE0F` now renders from a color font. One followed by `U+FE0E` renders from a monochrome font. `font-family` order does not affect either. Bare codepoints keep following the stack, matching browsers.
