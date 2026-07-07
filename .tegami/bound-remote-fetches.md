---
packages:
  npm:@takumi-rs/helpers:
    type: minor
---

### Bound remote fetches with byte caps and default timeouts

Remote image, font, and Google Fonts CSS fetches now reject bodies past a byte
cap (`maxBytes`, default 32 MiB; 2 MiB for CSS) and apply the 5 s timeout to
every fetch, not just images. Set `timeout: 0` to disable it. A new `allowUrl`
hook on `FetchOptions` skips URLs it rejects.
