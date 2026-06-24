---
packages:
  "npm:@takumi-rs/helpers": patch
---

### Render every weight of a variable Google Font

A variable font is served as one woff2 reused across weights. `googleFonts` now
collapses those faces into a single weightless face so the renderer drives the
`wght` axis, instead of pinning every weight to the file's default and leaving
`font-weight: 700` looking regular.
