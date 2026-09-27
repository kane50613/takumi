---
packages:
  "takumi": patch
  "takumi-pdf": patch
---

# Give every line the block's own line height

Each line of text now grows to the block's own `line-height` and font, even when it only holds a span with a smaller line height, as browsers do. Under `line-height: normal`, a fallback font, such as an emoji font, grows the line by its own line spacing. A font family listed first sets the line box even when it lacks a space glyph.
