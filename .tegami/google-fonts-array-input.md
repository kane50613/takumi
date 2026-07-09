---
packages:
  npm:@takumi-rs/helpers: patch
---

### Accept a families array in `googleFonts`

Pass the families directly instead of wrapping them in an options object:
`googleFonts(["Inter", "Noto Sans JP"])`. The object form stays for `text`,
`display`, and the other options.
