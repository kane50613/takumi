---
packages:
  npm:takumi-js:
    replay:
      - exit-prerelease(npm:takumi-js)
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

### Replace `fetchResources`/`extractResourceUrls` with `prepareImages`

`@takumi-rs/helpers` exports `prepareImages({ node, sources?, fetchCache?, fetch?, timeout? })`,
which collects a node tree's remote images and fetches the ones not already supplied. Pass a
`fetchCache` (a `Map<string, Promise<ArrayBuffer>>`, or any `Map`-like store) to coalesce
concurrent fetches of the same URL and reuse the bytes across renders; a failed fetch is
evicted so a later call retries.

The `extractResourceUrls` and `fetchResources` helpers are removed. The `images` render option
takes the same group form: `{ sources, fetchCache, fetch, timeout }`.
