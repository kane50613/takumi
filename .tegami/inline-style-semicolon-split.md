---
packages:
  "@takumi-rs/helpers":
    type: patch
---

### Keep a `;` inside an inline style value

`fromHtml` and `fromStaticMarkup` split the `style` attribute on every `;`, so a value carrying one of its own lost everything after it. `style="background-image:url(data:image/png;base64,...)"` resolved to `url(data:image/png` and rendered nothing. A quoted `font-family: "Foo; Bar"` was cut the same way. Only a `;` outside `url()` and a quoted string now ends a declaration.
