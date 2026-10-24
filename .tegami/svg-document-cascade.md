---
packages:
  takumi-core: minor
---

# Style SVG images with the full CSS cascade

An SVG image now resolves its `<style>` rules, `style` attributes, and presentation attributes through takumi's CSS cascade, so selectors, `!important`, `@media`, and `var()` work as in Chrome, and CSS can set geometry such as `r` or `d`. `stroke-linejoin: miter-clip` and `arcs` are invalid, as in Chrome. SVG images that contain `<text>` or filters are still drawn as before.
