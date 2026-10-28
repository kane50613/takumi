---
packages:
  takumi-core: patch
---

# Content a clipping box pushes past a page end shows on the next page

A box with `overflow: hidden` that breaks across pages used to clip each page to its own piece of the box, so a child that a `transform` moved past the page end lost the part that belongs on the next page. Each page now clips to the whole box, as Chromium prints it. Rounded corners on such a box also curve over its whole height on every page, so a PDF holds back a page until the box that starts on it ends.
