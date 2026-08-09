---
packages:
  takumi-pdf:
    type: patch
---

### Let a stroke be as transparent as what it outlines

Faux bold outlines a glyph in the colour it fills, and `-webkit-text-stroke` outlines it in its own. Both took the colour without its alpha, so translucent text came out ringed in solid colour. Text under `background-clip: text` is transparent by design, which made this a black outline around every gradient-filled glyph.
