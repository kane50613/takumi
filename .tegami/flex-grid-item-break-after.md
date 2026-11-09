---
packages:
  takumi-core: patch
  takumi-pdf: patch
---

# A forced break inside a flex or grid item reaches past the container

A `break-after: page` on the last child of a flex or grid item, or on the last row of a table inside one, was dropped, so the content after the container stayed on the same page. It now propagates out through the item and the table, as Chromium does, and the content after the container starts a new page.
