---
packages:
  "@takumi-rs/core": minor
  "@takumi-rs/wasm": minor
  "@takumi-rs/helpers": minor
  takumi-js: minor
  takumi-paint: minor
  takumi-pdf: minor
  takumi-core: minor
---

# Renderers split emoji and parse HTML themselves

Every renderer takes an `emoji` option, `twemoji`, `openmoji`, `blobmoji`, `noto`, `fluent`, `fluentFlat`, or `from-font`, and replaces each emoji with an image from that CDN the way `extractEmojis` does, so `needs` reports the emoji images too. `takumi-paint` and `takumi-pdf` take HTML strings as `@takumi-rs/core` and `@takumi-rs/wasm` do, PDF header and footer bands included, and `takumi-pdf` picks font subsets for page counters from the classes the cascade sees. `takumi-js` hands HTML and the `emoji` option to the renderer instead of parsing and splitting in JavaScript, and keeps doing both for a custom `renderer` without `needs`. `extractEmojis` is deprecated.
