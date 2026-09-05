---
packages:
  "@takumi-rs/helpers":
    type: patch
---

### Retry transient font and image requests

Retry temporary GET and HEAD failures within the existing timeout, including Google Fonts metadata and font file downloads.
