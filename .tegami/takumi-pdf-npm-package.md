---
packages:
  "takumi-pdf": minor
---

### Publish takumi-pdf, the wasm PDF package

`render(jsx)` turns a node tree or JSX into a paged PDF with selectable text and embedded subset fonts, on Node, Bun, and Cloudflare Workers. `page` holds the paged settings — `size` (`"a4"`, `"letter"`, `{ width, height }`), `landscape`, per-side margins, and repeating header/footer bands with `{page}`/`{pages}` counters — while `viewport` renders a fixed single page instead. Fonts, images, and stylesheets round out the options.
