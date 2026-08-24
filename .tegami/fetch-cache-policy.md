---
packages:
  "@takumi-rs/helpers":
    type: patch
---

### Enforce fetch policies on shared image cache entries

`prepareImages` with a shared `fetchCache` returned cached bytes without running the calling render's `allowUrl` or `maxBytes`, so bytes fetched under one tenant's policy could serve a render whose policy forbids them. Cache hits now run `allowUrl` over the URL and its recorded redirect chain, re-check `maxBytes` against the resolved bytes, and refetch privately when the shared entry was capped tighter than the current call allows. Entries with no recorded redirect chain are not trusted by callers carrying `allowUrl`; those callers refetch them under their own policy.
