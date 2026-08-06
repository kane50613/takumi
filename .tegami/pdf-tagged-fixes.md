---
packages:
  takumi-pdf:
    type: patch
---

### Tagged-output fixes

Link elements now follow the text they annotate instead of preceding it in reading order. `<img alt="">` is emitted as an artifact, so decorative images pass PDF/UA-1 instead of failing alt-text validation. An invalid `creationDate` rejects the render instead of disappearing from the output, and tag-tree faults surface as errors instead of panics.
