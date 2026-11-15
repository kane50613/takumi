---
name: takumi-usage
description: Write or review code that renders images, PDFs, or animations with Takumi. Use when code imports `takumi-js`, `@takumi-rs/*`, `takumi-pdf`, or the `takumi` crate, or when the user wants Open Graph images, social cards, or HTML/JSX screenshots without a browser.
---

# Takumi

Takumi lays out JSX, HTML, and CSS the way Chromium does and draws the result without a browser. It is **not Satori**: most Satori workarounds (a `display: flex` on every div, hand-drawn SVG instead of CSS, base64 data URLs, fonts read by hand) are dead weight here. Before writing a workaround, check the docs page for the feature. The docs are Markdown at `https://takumi.kane.tw/docs/<page>.md`, indexed at `https://takumi.kane.tw/llms.txt`.

## Pick the entry

| Task                              | Use                                                          |
| --------------------------------- | ------------------------------------------------------------ |
| An image from a route handler     | `new ImageResponse(node, options)` from `takumi-js/response` |
| Image bytes in a script or server | `render(node, options)` from `takumi-js`                     |
| SVG output                        | `renderSvg` from `takumi-js`                                 |
| Animated WebP, GIF, or APNG       | `renderAnimation` from `takumi-js`                           |
| A box's size before drawing       | `measure` from `takumi-js`                                   |
| A PDF                             | `takumi-pdf` ([docs](https://takumi.kane.tw/docs/pdf.md))    |
| Rust                              | the `takumi` crate, see [Rust](#rust)                        |

`takumi-js` picks the native addon on Node.js and Bun and WebAssembly on edge runtimes, and loads it itself. Import from `takumi-js`, not from `@takumi-rs/core` or `@takumi-rs/wasm`, and leave out `initSync`, `?module` imports, and `readFile` of a `.wasm`. `render` and `ImageResponse` already reuse one renderer across calls, so a module-level `Renderer` singleton is unnecessary.

## Rules

<!-- keep in sync with the rules in context7.json -->

- `render`, `renderSvg`, `renderAnimation`, and `measure` are async. Await them.
- `ImageResponse` streams its body, so a `try`/`catch` around `new ImageResponse(...)` never sees a render error. Await `response.ready` and return a fallback when it rejects.
- The root element does not fill the canvas on its own. Give it `width: 100%` and `height: 100%`.
- Takumi never reads system fonts. Its one built-in font is Geist, Latin only, weights 300 to 800. Pass every other font per call through `fonts`, and load Google Fonts with `googleFonts` from `takumi-js/helpers`, which fetches only the subsets the text needs.
- A `fonts` entry is a URL string, raw bytes, or `{ name, data, weight, style, generic, unicodeRange }`, where `data` is bytes or a function returning them. Leave `weight` out for a variable font, which then serves its whole `wght` axis. Register it once, not once per weight.
- Pass images through `images: [{ src: "logo", data }]` and use the key as `src` or in `url()`. Remote URLs are fetched for you. Do not inline them as base64.
- Pass CSS, including compiled Tailwind, through `css`. `stylesheets` is deprecated.
- The `tw` prop is a built-in Tailwind subset without Preflight, so elements keep their UA margins.
- `emoji` is `"twemoji"` (default), `"blobmoji"`, `"noto"`, `"openmoji"`, `"fluent"`, `"fluentFlat"`, or `"from-font"`.
- Docs code samples carry Fumadocs annotations such as `// [!code --]` and `// [!code highlight]`. Never copy them, and treat `[!code --]` lines as the removed API.

## What works that Satori lacks

Use these directly instead of working around them:

- Block, inline, float, flex, grid, and table layout. A `div` is a block, as in a browser.
- `<style>` rules, selectors, pseudo-elements, `@media`, `var()` and registered custom properties, `calc()` and other math functions.
- Colors in `oklch()`, `lab()`, `color-mix()`, and the other CSS Color 4 forms.
- `background-image` with gradients and `url()`, `mask`, `clip-path`, `filter`, `backdrop-filter`, blend modes, `box-shadow`, and transforms.
- Variable fonts with `font-variation-settings`, OpenType features, and `unicode-range` subsets.
- Inline `<svg>`, including text in the registered fonts.
- `text-fit`, `text-wrap: balance`, `line-clamp`, and `offset-path`.

## Look it up

| Question                                           | Page                                                              |
| -------------------------------------------------- | ----------------------------------------------------------------- |
| Fonts, fallback order, Google Fonts, subsets       | [fonts](https://takumi.kane.tw/docs/fonts.md)                     |
| Remote and pre-fetched images, emoji, fetch limits | [images](https://takumi.kane.tw/docs/images.md)                   |
| Inline styles, CSS, Tailwind, design tokens        | [styles](https://takumi.kane.tw/docs/styles.md)                   |
| Text wrapping, truncation, `text-fit`              | [typography](https://takumi.kane.tw/docs/typography.md)           |
| Formats, quality, cancellation                     | [formats](https://takumi.kane.tw/docs/formats.md)                 |
| Renderer reuse, caches, memory                     | [performance](https://takumi.kane.tw/docs/performance.md)         |
| Next.js, Nuxt, SvelteKit, Astro, and other setups  | [frameworks](https://takumi.kane.tw/docs/frameworks.md)           |
| Bundler and render errors                          | [troubleshooting](https://takumi.kane.tw/docs/troubleshooting.md) |
| Every node type and CSS property                   | [reference](https://takumi.kane.tw/docs/reference.md)             |

## Rust

Import from `takumi::prelude` and build options with `RenderOptions::builder()`. Fonts live in a `Fonts` value you pass in.

<!-- keep in sync with the doctest in takumi/src/lib.rs -->

```rust
use takumi::prelude::*;
use takumi::render;

let node = Node::container([Node::text("Hello, world!").with_style(
  Style::default().with(StyleDeclaration::font_size(Length::Px(32.0).into())),
)]);

let mut fonts = Fonts::default();
fonts.register(FontResource::new(include_bytes!(
  "../../assets/fonts/geist/Geist[wght].woff2"
)))?;

let options = RenderOptions::builder()
  .viewport(Viewport::new((1200, 630)))
  .node(node)
  .fonts(&fonts)
  .build();

let image = render(options)?;
```

With the `from-html` feature, `from_html(html, FromHtmlOptions::default())?` builds the node from HTML instead.
