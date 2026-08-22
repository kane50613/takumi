---
packages:
  "@takumi-rs/core":
    type: patch
---

### Keep `direction: ltr` blocks left-to-right

Blocks with `direction: ltr` now keep an LTR layout when text starts with an RTL script. The bidi base direction follows `direction`, matching browsers.
