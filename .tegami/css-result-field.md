---
packages:
  "@takumi-rs/helpers":
    type: minor
---

### Return `css` from `fromJsx` and `fromHtml`

Both results now carry the extracted CSS in a `css` field, matching the render option's name. The old `stylesheets` field still works and is marked deprecated.
