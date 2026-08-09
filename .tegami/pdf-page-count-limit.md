---
packages:
  takumi-pdf:
    type: minor
---

### Stop counting pages at twenty thousand

Content tall enough to cut into millions of pages walked the whole document once per page, with nothing to stop it. A render taking untrusted markup could be handed a document whose only purpose was to spend the renderer's memory. Rendering now fails with `TooManyPages` rather than trying.
