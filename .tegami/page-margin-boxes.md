---
packages:
  takumi-core: minor
  takumi-pdf: minor
---

# `@page` rules style the page and draw page-margin boxes, such as `@bottom-center`

A paged PDF now reads more of an `@page` rule than `size` and `margin`. The rule styles the page itself, and a page-margin rule nested in it draws its `content` in that part of the margin.

```css
@page {
  margin: 20mm 15mm;
  border: 0.5pt solid #94a3b8;
  background: #f8fafc;

  @bottom-center {
    content: "Page " counter(page) " of " counter(pages);
  }
}
```

- `background` covers the whole page, margins included.
- `border` and `padding` lie inside the margins, around the area the content flows in.
- `width` and `height` size the page's box, and `auto` margins center a smaller box on the page. They include the border and padding. Set `box-sizing: content-box` to measure them as Chromium does.
- Each of the 16 page-margin rules, such as `@top-left` and `@bottom-right-corner`, takes strings, images, `counter(page)`, and `counter(pages)` in `content`. The boxes inherit `font` and `color` from the `@page` rule and draw in the margin without growing it.
