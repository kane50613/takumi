---
packages:
  "cargo:takumi-core": patch
---

### Fix inline SVG data URIs truncated at `#`

Percent-escape `#` in data URI bodies so inline SVGs are not cut off at the first fragment delimiter.
