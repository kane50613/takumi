---
packages:
  takumi-pdf:
    type: patch
---

### Vendor krilla and subsetter into the PDF backend

The PDF writer now builds from vendored krilla and subsetter forks on one shared fontations stack, with the embedded-PDF, simple-text, threading and tagged-PDF subsystems removed. Output bytes are unchanged.
