---
packages:
  takumi-core: patch
  "@takumi-rs/helpers": patch
---

# A `lang` takumi cannot read no longer fails the render

A node whose `lang` is not a language tag, such as `lang=""`, keeps the text as an attribute instead of failing the render. A number `id` or `lang` in JSX is written as text, as React DOM writes it.

```tsx
<p lang="">Hello</p>
// Before: the render fails with `expected a valid BCP-47 language tag`
// After: it renders, and `[lang=""]` matches it
```
