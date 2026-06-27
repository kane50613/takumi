---
packages:
  cargo:takumi-css:
    replay:
      - 'exit prerelease: cargo:takumi-css'
  npm:@takumi-rs/helpers:
    replay:
      - 'exit prerelease: npm:@takumi-rs/helpers'
---

### Match the Chromium UA stylesheet for default element styles

Parse the relative font keywords `bolder`/`lighter` (`font-weight`) and
`larger`/`smaller` (`font-size`), resolving to the values Chromium uses. Expand
the default element presets to cover lists, `sub`/`sup`, `ins`/`del`, forms,
`details`/`summary`, and `search`.
