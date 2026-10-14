---
packages:
  "takumi-core": patch
  "takumi-pdf": patch
---

# Embed fonts at their `font-variation-settings` in PDF

PDF output and the paint tree now instance a variable font at the axes `font-variation-settings` sets. Before, they used the default instance, so glyphs drew at the wrong weight or width while the layout used the right one.
