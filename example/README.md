# Takumi examples

Each example is a small project that runs against the packages in this repository, not the npm releases. Build those packages once before you run an example.

To add Takumi to your own app, start with the [image guide](https://takumi.kane.tw/docs) or the [PDF guide](https://takumi.kane.tw/docs/pdf).

## Build the packages

Install [Bun](https://bun.sh), [Rust](https://www.rust-lang.org/tools/install), the WebAssembly target, and [wasm-pack](https://rustwasm.github.io/wasm-pack/installer/):

```bash
rustup target add wasm32-unknown-unknown
```

Install the workspace dependencies from the repository root:

```bash
bun install
```

### Image examples

Examples that use `takumi-js` need the helpers, both bindings, and `takumi-js` itself:

```bash
bun run --filter ./takumi-helpers build
bun run --filter ./takumi-napi build
bun run --filter ./takumi-napi build:js
bun run --filter ./takumi-wasm build
bun run --filter ./takumi-js build
```

### PDF examples

Examples that use `takumi-pdf` need the helpers and `takumi-pdf`:

```bash
bun run --filter ./takumi-helpers build
bun run --filter ./takumi-pdf-js build
```

The [Rust](./rust) example needs no package build.

## Images and frameworks

| Example                                    | What it shows                                               |
| ------------------------------------------ | ----------------------------------------------------------- |
| [Next.js](./nextjs)                        | An App Router route that renders a card from a query.       |
| [Cloudflare Workers](./cloudflare-workers) | A Worker that renders with WebAssembly.                     |
| [TanStack Start](./tanstack-start)         | A server route running on the Cloudflare Vite plugin.       |
| [SvelteKit](./svelte)                      | A Svelte component and its CSS rendered as an image.        |
| [Waku](./waku-ssr)                         | A dynamic image route using `ImageResponse`.                |
| [Rust](./rust)                             | Font registration, a node tree, and WebP output in Rust.    |
| [CSS libraries](./css-library-integration) | Compiled Tailwind CSS and UnoCSS stylesheets.               |
| [satori-html](./satori-html)               | A `satori-html` template rendered by Takumi.                |
| [Social images](./twitter-images)          | The cards, animations, and PDFs used on the Takumi website. |
| [Liquid glass](./liquid-glass)             | A WebGPU shader applied to raw Takumi pixels.               |

## PDF documents

| Example                                     | What it shows                                                    |
| ------------------------------------------- | ---------------------------------------------------------------- |
| [Invoices and receipts](./generate-invoice) | A paged A4 invoice and a single-page receipt.                    |
| [Factur-X invoice](./e-invoice)             | An XML invoice embedded in a PDF/A-3 and PDF/UA-1 file.          |
| [PDF benchmark](./pdf-bench)                | The same invoice rendered with Takumi, react-pdf, and Puppeteer. |

## Animation and video

| Example                               | What it shows                            |
| ------------------------------------- | ---------------------------------------- |
| [ffmpeg](./ffmpeg-keyframe-animation) | CSS keyframes encoded as an H.265 video. |
| [ffplay](./ffplay)                    | Raw RGBA frames played live as a clock.  |
