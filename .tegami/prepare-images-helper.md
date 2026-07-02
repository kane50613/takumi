---
packages:
  npm:takumi-js: major
  npm:@takumi-rs/helpers: major
  npm:@takumi-rs/core: major
  npm:@takumi-rs/wasm: major
---

### Replace `fetchResources`/`extractResourceUrls` with `prepareImages`

`takumi-js` exports `prepareImages({ node, sources?, fetchCache?, fetch?, timeout? })`, which
collects a node tree's remote images and fetches the ones not already supplied. Pass a
`fetchCache` (a `Map<string, Promise<ArrayBuffer>>`, or any `Map`-like store) to coalesce
concurrent fetches of the same URL and reuse the bytes across renders; a failed fetch is
evicted so a later call retries.

The `extractResourceUrls` and `fetchResources` helpers are removed. The `images` render option
takes the same group form: `{ sources, fetchCache, fetch, timeout }`.
