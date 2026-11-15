---
name: migrate-from-puppeteer
description: Migrate screenshot code from Puppeteer, Playwright, or node-html-to-image to Takumi. Use when code launches a headless browser to turn HTML or a page into an image or PDF.
---

# Migrate headless browser screenshots to Takumi

Takumi lays out the HTML and CSS the page already uses, the way Chromium does, and draws it without starting a browser. Pair this skill with `takumi-usage` for the API rules.

## Steps

1. **Find what the page needs at runtime.** Takumi runs no JavaScript, so a page that fetches data, renders on the client, or waits for a selector must get its data first. Done when each screenshot's template can be built as a JSX tree or HTML string from data you already hold.

2. **Swap the render.** `page.setContent(html)` plus `page.screenshot()` becomes `render(html, { width, height })` from `takumi-js`, which also takes JSX. `page.pdf()` becomes `prepare(html)` then `render(prepared)` from `takumi-pdf`, where `prepare` downloads the remote images and `@font-face` files ([PDF migration](https://takumi.kane.tw/docs/pdf/migration/puppeteer.md)). A loop of screenshots encoded into a GIF or video becomes `renderAnimation` with CSS keyframes ([Animation](https://takumi.kane.tw/docs/animation.md)). Done when no `puppeteer`, `playwright`, or `node-html-to-image` import remains in the code that makes images.

3. **Load the fonts the page used.** The browser read system fonts. Takumi reads none, and its one built-in font, Geist, covers only Latin. Pass each font through `fonts`, or `googleFonts` from `takumi-js/helpers`. Done when every `font-family` the template names is loaded.

4. **Move stylesheets and images.** Pass `<link>`ed CSS through `css`, or keep it in a `<style>` element. Remote images and `url()` load during the render, or in `prepare` for a PDF. Pass local files as `images: [{ src, data }]`. Done when no stylesheet or image comes from the local file system by path.

5. **Delete the browser plumbing.** Remove browser launch and pooling, Chromium binaries in the deploy, timeouts for page loads, and serverless Chromium packages. Done when the deploy no longer ships a browser.

6. **Compare the output.** Render each template with the old and new code and compare the images. Features Takumi does not draw, such as `<canvas>`, `<iframe>`, and video, need a static replacement. Done when each difference is fixed or replaced.
