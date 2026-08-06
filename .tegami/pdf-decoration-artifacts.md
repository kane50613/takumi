---
packages:
  takumi-pdf:
    type: patch
---

### Mark backgrounds and borders as artifacts

Backgrounds and borders were painted outside any tagged content sequence, so PDF/UA-1 validators reported untagged content on every page that drew one. They are now artifacts, like the header and footer bands.
