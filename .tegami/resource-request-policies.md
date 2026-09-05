---
packages:
  "@takumi-rs/helpers":
    type: patch
---

### Honor request policies across redirects and cache hits

Strip sensitive headers on cross-origin redirects, apply redirect method and body rules, and honor image cache readers' cancellation and timeout without aborting shared downloads.
