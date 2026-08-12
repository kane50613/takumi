---
packages:
  "@takumi-rs/helpers":
    type: patch
---

### Follow Unicode emoji presentation in `extractEmojis`

`extractEmojis` replaced every pictograph with a CDN image, so `‼` and `▶` came back as color emoji. Pictographs that default to text presentation now stay text, `U+FE0F` forces the emoji image, and `U+FE0E` forces the text glyph.
