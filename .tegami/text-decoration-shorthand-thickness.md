---
packages:
  "takumi": patch
---

# Keep a `text-decoration` thickness written right after the line

`text-decoration: overline 12px red` now draws a 12px line. Before, a length that directly followed the line keyword was dropped, so the line fell back to `auto` thickness.
