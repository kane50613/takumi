---
packages:
  "cargo:takumi-core": patch
---

### Stop untrusted SVG from reading local files via `<image href>`

An `<image>` or `<feImage>` element whose href is a filesystem path is no longer read from disk when parsing untrusted SVG markup or applying SVG filters. The string href resolver is disabled at both entry points, matching the nested-SVG path that already ignored external references. `data:` URI images keep working.
