---
packages:
  npm:@takumi-rs/helpers:
    replay:
      - exit-prerelease(npm:@takumi-rs/helpers)
---

### Add `baseUrl` to `googleFonts`

`googleFonts` takes an optional `baseUrl`, defaulting to Google Fonts, so an API-compatible
css2 mirror can be used instead, e.g. `baseUrl: "https://fonts.bunny.net/css2"` for Bunny Fonts.
