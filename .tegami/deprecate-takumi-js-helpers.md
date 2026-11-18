---
packages:
  "takumi-js": patch
---

# Editors flag the `takumi-js/helpers` entries, `RenderInput`, and `ImageResponseResult`

These keep working and will be removed in v3:

- `takumi-js/helpers`, `takumi-js/helpers/jsx`, `takumi-js/helpers/html`, and `takumi-js/helpers/emoji`. Import from `@takumi-rs/helpers` and its matching entries instead. `render` and `measure` take JSX and HTML, the `fonts` option takes font URLs, and the `emoji` option replaces `extractEmojis`.
- `RenderInput`. Use `DocumentInput`, which `takumi-js` now exports.
- `ImageResponseResult` from `takumi-js/response`. Use `ImageResponse`.

#### Migration

```diff
- import { googleFonts } from "takumi-js/helpers";
+ import { googleFonts } from "@takumi-rs/helpers";
```
