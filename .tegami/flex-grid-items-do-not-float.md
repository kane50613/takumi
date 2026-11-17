---
packages:
  takumi-core: patch
  takumi-pdf: patch
---

# A floated flex or grid item breaks across pages as any other item

`float` does not apply to a flex or grid item, but in a PDF an item with `float: left` or `float: right` was still treated as a float when pages broke. Its `break-after` was dropped, and the container's background could stop short on the next page. Such an item now lays out and breaks as any other item, as in Chromium.
