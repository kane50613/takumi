---
packages:
  "@takumi-rs/helpers": patch
---

# JSX writes boolean attributes as React DOM does

`aria-*` and `data-*` props, `draggable`, `contentEditable`, and `spellCheck` keep `false` as the text `"false"` and write `true` as `"true"`, so attribute selectors match them as in the browser. `htmlFor` is written as `for`.

```tsx
<div aria-hidden={false} draggable />
// Before: `[aria-hidden]` misses it, and `[draggable="true"]` misses it
// After: both match
```
