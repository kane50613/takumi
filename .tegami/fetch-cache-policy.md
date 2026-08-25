---
packages:
  "@takumi-rs/helpers":
    type: patch
---

### Enforce fetch policies on shared image cache entries

A shared `fetchCache` hit returned cached bytes without running the calling render's `allowUrl` or `maxBytes`. Cache hits now recheck both. `allowUrl` runs against the entry URL only, not the redirect hops the original fetch followed.

Google Fonts CSS cache hits now apply the current caller's `allowUrl` and `maxBytes`. Policy rejection keeps the shared entry available.
