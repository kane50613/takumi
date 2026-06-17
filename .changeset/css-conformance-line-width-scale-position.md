---
"takumi": major
---

Align CSS initial values and value semantics with the spec:

- `border-*-width`/`outline-width` default to `medium` (3px) and accept `thin|medium|thick`
- `scale`/`scaleX`/`scaleY`/`scale()` accept negative factors
- `position` defaults to `static`
- `line-clamp` no longer inherits
