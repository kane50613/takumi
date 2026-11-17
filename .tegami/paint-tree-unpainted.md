---
packages:
  takumi-core: minor
---

# The paint tree lists the elements painting skips

`PaintDocument::unpainted` names each element that painting skips, and why. It lists only the outermost one, since painting skips everything inside it too. These elements have no node, so the list is where you find why one is absent.

```diff
  {
    "steps": [ … ],
+   "unpainted": [
+     { "element": { "path": [0, 1], "tagName": "section" }, "reason": "opacity-zero" }
+   ]
  }
```

- `opacity-zero` marks an element that `opacity: 0` hides.
- `display-none` marks the root element when it has `display: none`.
