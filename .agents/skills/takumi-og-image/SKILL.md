---
name: takumi-og-image
description: Write or review code that renders an image with takumi-js, such as an Open Graph image route, a social card, or a script that writes PNG, WebP, SVG, or an animation. Use when code calls `render`, `renderAnimation`, or `ImageResponse` from `takumi-js`, or the user wants an OG image without a headless browser.
---

# Takumi image routes

Takumi lays out a template the way Chromium does, so write it as you would for a browser. The docs are Markdown at `https://takumi.kane.tw/docs/<page>.md`, indexed at `https://takumi.kane.tw/llms.txt`. Read the page a step names before you write the code it covers, and leave out the `// [!code ...]` annotations in its samples.

## Steps

1. **Pick the entry.**
   - A route handler returns `new ImageResponse(node, options)` from `takumi-js/response`. Follow the setup in the framework's guide, listed at [frameworks.md](https://takumi.kane.tw/docs/frameworks.md).
   - A script or server function awaits `render(node, options)` from `takumi-js`. For SVG, pass `format: "svg"`, which returns the document as UTF-8 bytes.
   - An animated WebP, APNG, or GIF comes from `renderAnimation`. See [animation.md](https://takumi.kane.tw/docs/animation.md).

   Done when the code imports only from `takumi-js`, `takumi-js/response`, and `takumi-js/node`, plus the `takumi-js/wasm-url` that `takumi-deploy` adds for a WebAssembly fallback. `takumi-js` loads its own backend, so any wasm init code or `@takumi-rs/*` import is a deployment problem for `takumi-deploy`.

2. **Size the canvas and the root.** Pass `width` and `height` to every call, because an omitted dimension follows the content. Give the root `width: 100%` and `height: 100%`, or `tw="w-full h-full"`, since it otherwise lays out as a block as tall as its content. Done when every call passes both dimensions and the root fills them.

3. **Write the layout as CSS.** A `<div>` is a block, as in a browser. Add `display: flex` only to an element whose children sit in a row or use flex alignment. Style with `style`, `tw`, or the `css` option, as [styles.md](https://takumi.kane.tw/docs/styles.md) describes. To fit a headline to its box, use `text-fit` from [typography.md](https://takumi.kane.tw/docs/typography.md). Done when each `display: flex` lays out children that need it, and no loop calls `measure` to pick a font size.

4. **Load fonts once.** Define the `fonts` list as a module-level constant and pass it to every call. When the text goes beyond Latin, uses a brand or variable font, or contains emoji, follow `takumi-fonts`. Done when no handler reads or fetches a font file.

5. **Pass images by URL or key.** Takumi fetches the remote `<img src>` and `background-image` URLs a template draws. Pass local bytes as `images: [{ src: "logo", data }]` and write `src="logo"`. When user input can reach a URL, set `images.allowUrl`, and keep `images.timeout` below the route's own time limit. [images.md](https://takumi.kane.tw/docs/images.md) covers both. Done when no code builds a base64 data URL from a file or a fetch, and every URL that user input reaches passes through `allowUrl`.

6. **Use the shared renderer.** `render` and `ImageResponse` share one renderer per process. Create a `Renderer` from `takumi-js/node` only to change `cacheMaxBytes` or `fontCacheMaxBytes`, and then create it once at module level and pass it as `renderer`. Done when the code has no `new Renderer()`, or has exactly one at module level that sets one of those budgets.

7. **Handle errors on `ready`.** `ImageResponse` streams its body, so a `try`/`catch` around `new ImageResponse(...)` never sees a render error. Await `response.ready` to send a fallback, or pass `onError` to log, as [reference/image-response.md](https://takumi.kane.tw/docs/reference/image-response.md) shows. Done when no `try` block holds only the constructor, and every `render`, `renderAnimation`, and `measure` call is awaited.

8. **Render it and look.** Run the script, or request the route and save the body to a file, then open the image. Done when every element shows at its intended size and position and every character draws. Otherwise follow `takumi-fonts` for missing characters and `takumi-debug` for the rest.
