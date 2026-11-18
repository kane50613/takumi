# @takumi-rs/helpers

**Deprecated. Import these helpers from the Takumi package you render with.**

[Documentation](https://takumi.kane.tw/docs/reference/helpers) · [GitHub](https://github.com/kane50613/takumi)

Takumi's packages share the code in this package and re-export what you need from it. It becomes internal in v3, so do not install it yourself. Every export still works in 2.x, and your editor flags it as deprecated.

| Package                                 | Exports                                                                                              |
| :-------------------------------------- | :--------------------------------------------------------------------------------------------------- |
| `takumi-js`                             | `googleFonts`, `container`, `text`, `image`, and `Bitmap`                                            |
| `takumi-pdf`                            | `googleFonts`                                                                                        |
| `@takumi-rs/core` and `@takumi-rs/wasm` | `fromJsx`, `googleFonts`, `fontFromUrl`, `extractEmojis`, `container`, `text`, `image`, and `Bitmap` |

```diff
- import { googleFonts } from "@takumi-rs/helpers";
- import { fromJsx } from "@takumi-rs/helpers/jsx";
+ import { fromJsx, googleFonts } from "@takumi-rs/core";
```

`render` in `takumi-js`, `takumi-pdf`, and `takumi-paint` takes JSX and HTML, so it needs no `fromJsx`. Its `fonts` option takes font URLs, so it needs no `fontFromUrl`. See [Helpers](https://takumi.kane.tw/docs/reference/helpers).

## License

MIT or Apache-2.0
