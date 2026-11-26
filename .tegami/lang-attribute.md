---
packages:
  takumi-core: patch
  "@takumi-rs/helpers": patch
---

# `lang` keeps the text it is written with

An element keeps its `lang` attribute as written, so `[lang]` selectors and `attr(lang)` read the same text as in a browser. A `lang` that is not a language tag takumi reads, such as `lang=""`, stays an attribute instead of being dropped or failing the render. A number `id` or `lang` in JSX is written as text, as React DOM writes it.

```html
<p lang="EN_us">Hello</p>
<!-- Before: `[lang="EN_us"]` misses it, and `attr(lang)` reads `en-US` -->
<!-- After: both read `EN_us` -->
```
