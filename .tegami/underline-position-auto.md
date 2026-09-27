---
packages:
  "takumi": patch
  "takumi-pdf": patch
---

# Place `auto` underlines just under the baseline, as Chrome does

`text-underline-position: auto` now puts the underline half its thickness, at least a pixel, below the baseline instead of at the font's underline position, which `from-font` still uses, and `under` leaves a pixel below the em box. A set `text-underline-offset` drops the `auto` gap.
