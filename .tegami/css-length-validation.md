---
packages:
  takumi-core: patch
  "@takumi-rs/helpers": patch
---

# Validate and compute lengths as Chrome does

CSS text now reads lengths the way Chrome does.

- `letter-spacing: normal` and `word-spacing: normal` set no extra spacing. `auto` is invalid for them and for `text-indent`, so the declaration is dropped.
- A negative `line-height` is invalid, also inside the `font` shorthand.
- A `font-size` calculation that comes out negative is `0px`.
- A unitless number other than `0` is no longer a length in px. `font-size: 12` and `text-indent: 5` are dropped. SVG attributes and the properties Chrome parses like them, such as `stroke-width`, `r`, and `stroke-dasharray`, still take one.
- `calc(10% + 2em)` computes its `em` part to px on the element that declares it, so children inherit `calc(10% + 32px)`. A percentage or calculated `line-height` computes to px against the element's own font size.
- `lh` resolves against the element's own font size and line height, not its parent's.

Numbers in a JSX style object still work. A number becomes px unless the property takes a bare number, as React writes it. The SVG serializer in `@takumi-rs/helpers` now writes `px` after a number in a `style` object, as React DOM does.

```css
.card {
  font-size: 20px;
  line-height: 1.5;
  /* Before: 24px, from the parent's 16px font */
  /* After: 30px */
  height: 1lh;
}
```
