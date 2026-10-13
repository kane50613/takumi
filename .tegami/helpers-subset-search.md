---
packages:
  "@takumi-rs/helpers": patch
---

# Pick font subsets faster

`subsetFonts`, which every render with `fonts` runs, finds the subsets the text needs by binary search instead of checking every character against every range. With Noto Sans JP, Noto Sans TC, and Inter from Google Fonts, it takes 0.3 ms per render instead of 3.6 ms.
