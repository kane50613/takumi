---
packages:
  "@takumi-rs/helpers":
    type: minor
---

### Return `css` from `fromJsx` and `fromHtml`

Both results now carry the extracted CSS in a `css` field, matching the render option's name. Reading the old `stylesheets` field still returns the same array and warns once. It no longer shows up in `Object.keys` or a spread of the result.
