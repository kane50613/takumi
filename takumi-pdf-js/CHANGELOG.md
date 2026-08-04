## takumi-pdf@0.1.1

### Auto-height viewport

`viewport.height` is now optional. Omitting it sizes the single page to the laid-out content, like a thermal receipt.

### Render SVG image sources

SVG images passed via `images` came out as blank space; the backend only embedded bitmap sources. SVG sources now rasterize at twice their displayed size and embed like other images.

### Ship the `tw` prop type

Importing only `takumi-pdf` left JSX `tw` props failing to typecheck; the react module augmentation now ships with the package types.

## takumi-pdf@0.1.0

### Publish takumi-pdf, the wasm PDF package

`render(jsx)` turns a node tree or JSX into a paged PDF with selectable text and embedded subset fonts, on Node, Bun, and Cloudflare Workers. Options mirror Puppeteer's `page.pdf()`: `size` (`"a4"`, `"letter"`, `{ width, height }`), `landscape`, per-side margins, and repeating header/footer bands with Chromium-style `pageNumber`/`totalPages` class hooks and CSS counter styles, while `viewport` renders a fixed single page instead. Fonts, images, and stylesheets round out the options.
