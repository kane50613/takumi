---
packages:
  takumi-core:
    type: patch
---

### Report a capped WebP animation as unfinished

A WebP stream cut at the frame cap now reports the same unfinished flag as APNG and GIF instead of claiming it ended.
