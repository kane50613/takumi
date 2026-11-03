---
packages:
  takumi-core: minor
  takumi-pdf: minor
---

# `@page` rules size and margin the pages of a PDF

A `@page` rule sets the page `size` and margins, and wins over the `size`, `landscape`, and `margin` options, as CSS does in Chromium with `preferCSSPageSize`. `:first` styles the first page, and `:left` and `:right` the pages after it, each page laying its content out at its own width. `@media`, `@layer`, and `!important` order the rules as they order any other. Page names and margin boxes such as `@top-center` are not read yet.

`StyleSheet::page_descriptors` returns the `@page` descriptors that win for a page.
