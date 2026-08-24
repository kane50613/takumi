---
packages:
  "@takumi-rs/helpers":
    type: patch
---

### Enforce fetch policies on shared image cache entries

A shared `fetchCache` hit returned cached bytes without running the calling render's `allowUrl` or `maxBytes`. Cache hits now check both, including every recorded redirect hop. Entries fetched without `allowUrl` have no recorded chain, so a later `allowUrl` only checks their entry URL.
