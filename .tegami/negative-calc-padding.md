---
packages:
  takumi-core: patch
---

# A negative `calc()` padding clamps to zero

A `padding` written as a `calc()` that comes out negative was applied as a negative padding. It now clamps to zero, as browsers do.

```css
padding: calc(0px - 4px);
/* Before: a padding of -4px */
/* After: a padding of 0 */
```
