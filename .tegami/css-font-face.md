---
packages:
  takumi-core: minor
  takumi: minor
---

# Load fonts from `@font-face` rules

With the `font-face` feature, on by default in `takumi`, an `@font-face` rule adds a face to the render the way Chrome does. It takes the rule's `font-family`, `font-weight`, `font-style`, `font-stretch`, `unicode-range`, and `font-variation-settings`, and its first `src` that has not failed to load. `needs` reports the `src` URLs that the text reaches in `RenderNeeds::fonts`. Hand each file to `Fonts::load_font_file`, or call `Fonts::fail_font_file` so the rule moves on to its next `src`. `takumi` now exports `needs` and `RenderNeeds`.
