---
packages:
  "cargo:takumi-core": patch
---

### Serialize `text-fit: none` without its target and limit

The computed value of `text-fit` kept the target and limit keywords after `none`, so `none per-line 50%` round-tripped verbatim. Chromium drops both, since neither scales anything when the value is `none`. The serializer now stops after `none`.
