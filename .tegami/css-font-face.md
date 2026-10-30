---
packages:
  takumi-core: minor
  takumi: minor
---

# Load fonts from `@font-face` rules

With the `font-face` feature, on by default in `takumi`, `@font-face` rules add fonts to a render. `needs` reports the files the text uses in `RenderNeeds::fonts`. Pass each to `Fonts::load_font_file`, or to `Fonts::fail_font_file` to fall back to the next `src`.
