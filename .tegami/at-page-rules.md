---
packages:
  takumi-core: minor
  takumi-pdf: minor
---

# `@page` rules size and margin the pages of a PDF

A `@page` rule sets the page `size` and margins and wins over the `size`, `landscape`, and `margin` options, as CSS does in Chromium with `preferCSSPageSize`. `:first` styles the first page, and `:left` and `:right` style pages by the side they fall on, so under `direction: rtl` the first page is a left page. Each page lays out its content, bands, and repeated boxes at its own size. `em`, `rem`, and `var()` in a rule read the root element, as Chromium's page style inherits it. `@media`, `@layer`, and `!important` order the rules as they order any other. `StyleSheet::page_descriptors` returns the descriptors that win for a page in a `PageContext`, which `PageContext::from_root` and `PageContext::from_initial_containing_block` take from a tree's root element. Page names and margin boxes such as `@top-center` are not read yet.
