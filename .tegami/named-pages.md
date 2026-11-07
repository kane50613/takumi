---
packages:
  takumi-core: minor
  takumi-pdf: minor
---

# The `page` property puts content on named pages

An element with `page: cover` starts a page that `@page cover` rules size and margin, and the content after it that names no page goes back to the unnamed pages, as in Chromium. `PageSizer::new` now takes the page name with the page index, and `PageSizes::name` returns the name each page laid out as.
