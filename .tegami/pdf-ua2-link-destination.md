---
packages:
  takumi-pdf:
    type: patch
---

### Give a link target something to point at under PDF/UA-2

A link to `#some-id` names a structure element, and PDF/UA-2 requires every link inside a document to do so. Markup with nothing to say for itself, a plain `div` holding an id, left no element behind, so the link named one that was never written and the file failed validation while the render reported success.
