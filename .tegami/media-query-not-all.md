---
packages:
  "takumi-core":
    type: patch
---

### Keep parsing a stylesheet past an unknown media query

An unknown media feature such as `(prefers-color-scheme: dark)` used to fail
the whole stylesheet. The query now matches nothing and the rest of the sheet
still applies, as the spec requires.
