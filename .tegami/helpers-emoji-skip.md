---
packages:
  "@takumi-rs/helpers": patch
---

# Skip emoji scanning for plain text

`extractEmojis` skips grapheme segmentation for text with no codepoint an emoji starts with. A 48-card dashboard without emoji goes through it in 0.02 ms instead of 0.33 ms per render.
