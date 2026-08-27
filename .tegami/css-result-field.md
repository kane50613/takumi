---
packages:
  "@takumi-rs/helpers":
    type: minor
---

### Return `css` from `fromJsx` and `fromHtml`

Reading the old `stylesheets` field returns the same array and warns once. It no longer appears in `Object.keys` or a spread of the result.
