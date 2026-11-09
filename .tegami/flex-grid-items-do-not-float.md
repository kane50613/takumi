---
packages:
  takumi-core: patch
  takumi-pdf: patch
---

# A floated flex or grid item no longer floats

`float` does not apply to a flex or grid item, but an item with `float: left` or `right` was still treated as a float when breaking, so its `break-after` was dropped and the container's background stopped short on the next page. Such an item now lays out as any other item, as in Chromium.
