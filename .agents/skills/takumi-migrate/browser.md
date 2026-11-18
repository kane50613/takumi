# From Puppeteer, Playwright, or node-html-to-image

Takumi lays out the page's HTML and CSS without starting a browser, and runs no JavaScript.

## Swap

1. **Get the data first.** A page that fetches data, renders on the client, or waits for a selector must receive its data before the render. The swap can start when each template builds as JSX or an HTML string from data you already hold.

2. **Replace each browser call.**
   - `page.setContent(html)` plus `page.screenshot()` becomes `render(html, { width, height })` from `takumi-js`, which also takes JSX.
   - `page.pdf()` becomes `prepare(html)` and then `render(prepared)` from `takumi-pdf`. `prepare` fetches the remote images and loads the fonts the text uses. [pdf/migration/puppeteer.md](https://takumi.kane.tw/docs/pdf/migration/puppeteer.md) maps each `page.pdf()` option.
   - A loop of screenshots encoded into a GIF or video becomes `renderAnimation`. Frames that differ through CSS become keyframes, and frames that page JavaScript draws become one scene each. See [animation.md](https://takumi.kane.tw/docs/animation.md).

3. **Move stylesheets and images.** Pass `<link>`ed CSS through the `css` option, or keep it in a `<style>` element. Pass local image files as `images: [{ src, data }]`. The swap is done when no stylesheet or image loads from the local file system by path.

4. **Restrict the image URLs.** Takumi fetches each remote image from your server and allows every URL by default, so removing the browser's request limits can open your network to the markup. When user input can reach an image URL, set `images.allowUrl`. To filter by resolved address, pass your own `fetch`, as [images.md](https://takumi.kane.tw/docs/images.md#urls-from-user-input) describes. The swap is done when every URL that user input reaches passes through `allowUrl`.

## Workarounds to delete

- browser launch, page pools, and their timeouts
- Chromium binaries in the deploy, and serverless Chromium packages
- waits for network idle or for fonts to load

Takumi cannot draw `<canvas>`, `<iframe>`, or video. Give each one a static replacement, such as an image or inline SVG, before you compare the output.
