---
packages:
  "@takumi-rs/core":
    type: minor
  "@takumi-rs/helpers":
    type: minor
---

### Support the `disclosure-open` and `disclosure-closed` counter styles

`list-style-type` now accepts `disclosure-open` and `disclosure-closed`, drawing the triangles CSS Counter Styles defines. `disclosure-closed` points the way the text runs, so it flips under `direction: rtl`. Font subsetting covers all three characters.
