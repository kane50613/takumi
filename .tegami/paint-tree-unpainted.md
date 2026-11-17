---
packages:
  takumi-core: minor
---

# The paint tree lists the elements painting skips

`PaintDocument::unpainted` names each element that painting skips along with everything inside it, and why. Those elements have no node, so the list is where you find why one is absent.

```json
"unpainted": [
  { "element": { "path": [0, 1], "tagName": "section" }, "reason": "opacity-zero" }
]
```

- `opacity-zero` marks an element that `opacity: 0` hides.
- `display-none` marks the root element when it has `display: none`.
