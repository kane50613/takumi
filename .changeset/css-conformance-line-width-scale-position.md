---
"takumi-css": major
"takumi": major
---

Align CSS initial values and value semantics with the spec:

- `border-*-width`/`outline-width` default to `medium` (3px) and accept `thin|medium|thick`
- `scale`/`scaleX`/`scaleY`/`scale()` accept negative factors
- `position` defaults to `static`
- split `line-clamp` into the `max-lines`/`block-ellipsis`/`continue` longhands with correct per-longhand inheritance
