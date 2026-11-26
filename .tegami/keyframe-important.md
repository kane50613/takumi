---
packages:
  takumi-core: patch
---

# A keyframe drops a declaration marked `!important`

A declaration in a `@keyframes` rule that ends in `!important` is invalid, so the animation leaves that property alone, as browsers do.

```css
@keyframes fade {
  to {
    opacity: 0 !important;
  }
}
/* Before: the element fades out */
/* After: the element stays opaque */
```
