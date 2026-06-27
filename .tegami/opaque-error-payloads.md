---
packages:
  "cargo:takumi-core": minor
---

### Stop leaking dependency error types through the public error enums

`Error`, `ImageResourceError`, and `FontError` carried `png`/`gif`/`image`/`taffy`/`usvg`/`wuff` payloads. They now hold owned message strings.
