---
packages:
  takumi-core:
    type: patch
---

### Honor emoji variation selectors in font selection

When emoji render from registered fonts (`emoji: "from-font"`), a codepoint followed by `U+FE0F` now picks a registered color font and one followed by `U+FE0E` a registered monochrome font. `font-family` order does not affect either. Bare codepoints keep following the stack, matching browsers.
