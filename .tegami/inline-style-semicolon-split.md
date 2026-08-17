---
packages:
  "@takumi-rs/helpers":
    type: patch
---

### Keep a `;` inside an inline style value

`fromHtml` / `fromStaticMarkup` split the `style` attribute on every `;`, so a value carrying one of its own was truncated at the first and the remainder dropped. `style="background-image:url(data:image/png;base64,...)"` resolved to `url(data:image/png` — a data URL is the usual way markup embeds an image without a fetch. A quoted `font-family: "Foo; Bar"` was cut the same way. Declarations are now split on top-level `;` only, outside `url()` and quoted strings.
