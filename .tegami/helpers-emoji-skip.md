---
packages:
  "@takumi-rs/helpers": patch
---

# Skip emoji scanning for plain text

`extractEmojis` segments text into graphemes only when it holds an `Extended_Pictographic` or `Regional_Indicator` codepoint or a keycap mark (U+20E3). A 48-card dashboard without emoji goes through it in 0.02 ms instead of 0.33 ms per render.
