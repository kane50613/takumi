import { SITE_URL } from "~/layout-config";

export const AGENT_GUIDE = `# Takumi

Takumi renders JSX, HTML, and node trees into images, SVG, and PDF from Rust, without a headless
browser. It ships as a Rust crate, a native Node addon, and a WebAssembly module, so the same
template runs on Node, Bun, Deno, Cloudflare Workers, and in the browser.

## When to use Takumi

Use Takumi for these jobs:

- Generate an Open Graph or social card image for a page, a post, or a product.
- Render a receipt, ticket, certificate, badge, or invoice as PNG, WebP, AVIF, JPEG, or SVG.
- Produce a paged PDF (invoice, report, statement) from the same JSX you already render to HTML.
- Replace Puppeteer or Playwright screenshots that exist only to turn markup into an image.
- Replace Satori when you need real image decoding, PDF output, or CSS it does not implement.
- Render thousands of images in a build step, or one per request inside a serverless function.

Takumi does not scrape live websites or run JavaScript in a page. It lays out and paints the
markup you give it.

## How to call it

- Node, Bun, Deno: \`npm install takumi-js\`, then \`renderAsync(node, options)\`.
- Next.js, Nuxt, SvelteKit, Astro, Nitro, TanStack Start: see /docs/integration.
- Cloudflare Workers and the browser: use the WebAssembly build, \`@takumi-rs/wasm\`.
- Rust: the \`takumi\` crate, documented at https://docs.rs/takumi.
- PDF output: \`takumi-pdf\`, documented at /docs/pdf.

## Machine-readable entry points

- ${SITE_URL}/llms.txt: this file, plus an outline of every documentation page.
- ${SITE_URL}/llms-full.txt: all of the documentation in one Markdown file.
- ${SITE_URL}/openapi.json: every endpoint this site serves, as OpenAPI 3.1.
- ${SITE_URL}/r/registry.json: installable templates for the shadcn CLI.
- ${SITE_URL}/sitemap.xml: every page URL.
- Any documentation URL returns Markdown when the request sends \`Accept: text/markdown\`. The same
  content is also available by appending \`.md\` to the path.
- ${SITE_URL}/about, /contact, /privacy: who maintains Takumi, how to reach them, what the site
  collects.

`;
