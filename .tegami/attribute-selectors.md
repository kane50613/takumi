---
packages:
  takumi-core: patch
  "@takumi-rs/helpers": patch
---

# `[dir]` and `img[src]` match elements parsed from HTML

Attribute selectors on `dir` and on an `<img>`'s `src` used to match only nodes built with JSX. `dir="auto"` in JSX also no longer fails the render.

```tsx
<div dir="auto">{name}</div>
// Before: the render fails with `unknown variant auto`
// After: it renders, with its direction taken from its text
```
