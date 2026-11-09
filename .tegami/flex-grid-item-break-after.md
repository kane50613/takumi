---
packages:
  takumi-core: patch
  takumi-pdf: patch
---

# A forced break inside a flex or grid item reaches past the container

A `break-after: page` on the last child of a flex or grid item was dropped, so the content after the container stayed on the same page. It now propagates out through the item, as Chromium does, and the content after the container starts a new page.
