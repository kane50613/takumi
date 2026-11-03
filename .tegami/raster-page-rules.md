---
packages:
  takumi-core: minor
  takumi-raster: minor
---

# Raster pages can take their sizes from `@page` rules

`render_pages` takes anything that converts into a `PageSizer`, so a `Size` still gives every page the same area, and each page's image is now that page's own size. `PageSizer::from_page_rules(stylesheet, page, margin)` gives each page the area the stylesheet's `@page` rules give it, including `:first`, `:left`, `:right`, and page names. `PageSize::resolve` and `PageSizeName::size` resolve a `size` value, `Viewport::print_page` is the viewport an `@page` rule resolves against, and `Rect::all` is public.
