---
packages:
  "cargo:takumi-core": patch
---

### Stop blend isolation from clipping text descenders

Include plain text nodes' glyph ink in scene paint bounds; `mix-blend-mode` on a text node no longer cuts glyphs that overflow the line box. Bounds now report unknown instead of underestimating for styles whose ink extent is not measured (shadows, outlines, text strokes), falling back to full-viewport isolation.
