---
packages:
  takumi-core: minor
  takumi-pdf: minor
---

# `@page` rules draw page-margin boxes, such as `@top-left` and `@bottom-center`

A page-margin rule nested in `@page`, such as `@bottom-center { content: "Page " counter(page) " of " counter(pages) }`, now draws its `content` in that part of the page's margin. Before, these rules were dropped.
