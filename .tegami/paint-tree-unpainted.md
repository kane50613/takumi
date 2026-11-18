---
packages:
  takumi-core: minor
---

# The paint tree lists the elements painting skips

`PaintDocument::unpainted` names each element that `opacity: 0` hides along with everything inside it. Those elements have no node, so the list is where a reader finds why one is absent.
