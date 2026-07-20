---
packages:
  "@takumi-rs/helpers": patch
---

### Enforce allowUrl on every redirect hop

`fetchOk` with an `allowUrl` policy now follows redirects manually (capped at 5 hops) and re-checks the resolved target of each hop, so an allowed URL can no longer redirect to a blocked address. Callers without a policy keep default redirect handling.
