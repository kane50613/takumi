---
packages:
  takumi-core: patch
---

# `lang` keeps the text it is written with

An element keeps its `lang` attribute as written, so `[lang]` selectors and `attr(lang)` read the same text as in a browser. A `lang` that is not a language tag takumi reads is no longer dropped.

```html
<p lang="EN_us">Hello</p>
<!-- Before: `[lang="EN_us"]` misses it, and `attr(lang)` reads `en-US` -->
<!-- After: both read `EN_us` -->
```
