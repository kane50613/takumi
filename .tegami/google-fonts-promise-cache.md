---
packages:
  npm:@takumi-rs/helpers:
    type: minor
---

### Cache the Google Fonts CSS promise

`googleFonts`'s `cache` now stores the in-flight `Promise<string>` instead of
the resolved CSS, so concurrent calls for the same URL share one request
instead of each missing and fetching. A failed fetch evicts itself, so the
next call retries. The cache type is now
`Pick<Map<string, Promise<string>>, "get" | "set" | "delete">`.
