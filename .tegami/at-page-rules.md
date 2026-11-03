---
packages:
  takumi-core: minor
  takumi-pdf: minor
---

# `@page` rules size and margin the pages of a PDF

A `@page` rule sets the page `size` and margins and wins over the `size`, `landscape`, and `margin` options, as CSS does in Chromium with `preferCSSPageSize`. `:first` styles the first page, and `:left` and `:right` style the pages after it, each page laying out its content, bands, and repeated boxes at its own size. `@media`, `@layer`, and `!important` order the rules as they order any other, and `StyleSheet::page_descriptors` returns the descriptors that win for a page. Page names and margin boxes such as `@top-center` are not read yet.
