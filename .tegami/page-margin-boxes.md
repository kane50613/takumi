---
packages:
  takumi-core: minor
  takumi-pdf: minor
---

# `@page` rules draw page-margin boxes, such as `@top-left` and `@bottom-center`

A page-margin rule nested in `@page` with `content` now draws in that part of the page's margin, so a document can carry its own running headers and page numbers in CSS:

```css
@page {
  @bottom-center {
    content: "Page " counter(page) " of " counter(pages);
  }
}
```

All 16 boxes are read. `content` takes strings, images, `counter(page)`, and `counter(pages)`, and the boxes are sized and placed as Chromium prints them. Before, these rules were dropped.

Two parts of the page model are not drawn yet: the `@page` rule's own `background`, `border`, `padding`, `width`, and `height`, and counters other than `page` and `pages`, which read 0.
