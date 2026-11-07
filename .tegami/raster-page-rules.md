---
packages:
  takumi-core: minor
  takumi-raster: minor
---

# Raster pages can take their sizes from `@page` rules

`render_pages` takes anything that converts into a `PageSizer`. A `Size` still gives every page the same area, and each page's image is now that page's own size. `PageRules::new(stylesheet, page_box, context)` converts into a `PageSizer` that gives each page the area the stylesheet's `@page` rules give it, including `:first`, `:left`, `:right`, and page names. `context` is the `PageContext` the root element gives, from `PageContext::from_root`. `PageRules::page_box` returns a page's size and margins, and `Viewport::print_page` is the viewport an `@page` rule resolves against.
