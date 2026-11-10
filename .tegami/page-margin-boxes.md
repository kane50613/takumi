---
packages:
  takumi-core: minor
  takumi-pdf: minor
---

# `@page` rules style the page and draw page-margin boxes, such as `@bottom-center`

An `@page` rule now gives the page its `background`, `border`, `padding`, `width`, and `height`, and a page-margin rule nested in it, such as `@bottom-center { content: "Page " counter(page) " of " counter(pages) }`, draws its `content` in that part of the margin. Before, only `size` and `margin` were read.
