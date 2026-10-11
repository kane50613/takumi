---
packages:
  "takumi-pdf": patch
---

# Write each page's clips and groups on that page only

A paginated PDF used to write the clip paths and transparency groups of every box on every page, empty ones included for boxes on other pages. Each page now writes only what it shows, so documents with `overflow: hidden` or `opacity` come out smaller. Long documents also render faster, since a page no longer walks the whole document.
