---
packages:
  npm:@takumi-rs/helpers:
    replay:
      - exit-prerelease(npm:@takumi-rs/helpers)
  npm:@takumi-rs/core:
    replay:
      - exit-prerelease(npm:@takumi-rs/core)
  npm:@takumi-rs/wasm:
    replay:
      - exit-prerelease(npm:@takumi-rs/wasm)
---

### Accept a bare URL string in `fonts`

`fonts` entries can now be a URL string, e.g. `fonts: ["https://example.com/Inter.woff2"]`.
The bytes are fetched on demand and keyed by the URL; family name, weight, and style come
from the font file. The object form stays for overriding those. Adds `fontFromUrl` to
`@takumi-rs/helpers`.
