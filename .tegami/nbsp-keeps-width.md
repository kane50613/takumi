---
packages:
  "takumi": patch
  "takumi-pdf": patch
---

# Keep non-breaking and ideographic spaces from collapsing

`&nbsp;`, U+3000, and other spaces outside CSS's document white space keep their width under `white-space-collapse: collapse`, as browsers render them. Runs of spaces, tabs, and line breaks still collapse to one space.
