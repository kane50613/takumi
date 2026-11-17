---
packages:
  takumi-core: patch
  "@takumi-rs/wasm": patch
---

# A WOFF font in `@takumi-rs/wasm` fails with an error that says to convert it

`@takumi-rs/wasm` cannot read WOFF fonts. Registering one now fails with an error that names the format, instead of `Unsupported font format`.

```ts
await renderer.registerFont(woffBytes);
// Before: Unsupported font format
// After: This build cannot read WOFF fonts. Convert the font to a format it reads, such as TTF
```

The fonts docs now list the formats each package reads. In Rust, a `takumi-core` build without the `woff` or `woff2` feature returns `FontError::FormatLeftOut` for that format.
