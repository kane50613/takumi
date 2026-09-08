# Takumi examples

Choose an example by the output or framework you need. These projects use local workspace packages, so install dependencies from the repository root and follow each example's build instructions.

## Images and frameworks

| Example                                    | What it shows                                              |
| ------------------------------------------ | ---------------------------------------------------------- |
| [Next.js](./nextjs)                        | An App Router image response with a query-string title.    |
| [Cloudflare Workers](./cloudflare-workers) | Image generation with WebAssembly.                         |
| [TanStack Start](./tanstack-start)         | A server image route with the Cloudflare Vite plugin.      |
| [SvelteKit](./svelte)                      | A Svelte component and its CSS rendered as an image.       |
| [Waku](./waku-ssr)                         | A dynamic image endpoint using `ImageResponse`.            |
| [Rust](./rust)                             | Font registration, node layout, and WebP encoding.         |
| [CSS libraries](./css-library-integration) | Compiled Tailwind CSS and UnoCSS stylesheets.              |
| [satori-html](./satori-html)               | HTML template conversion before image rendering.           |
| [Social images](./twitter-images)          | The cards and animation frames used on the Takumi website. |
| [Liquid glass](./liquid-glass)             | A WebGPU shader applied to raw Takumi pixels.              |

## PDF documents

| Example                                     | What it shows                                                    |
| ------------------------------------------- | ---------------------------------------------------------------- |
| [Invoices and receipts](./generate-invoice) | Paged A4 and fixed-width single-page output.                     |
| [Factur-X invoice](./e-invoice)             | An embedded XML invoice and PDF conformance validation.          |
| [PDF benchmark](./pdf-bench)                | The same invoice rendered with Takumi, react-pdf, and Puppeteer. |

## Animation and video

| Example                               | What it shows                             |
| ------------------------------------- | ----------------------------------------- |
| [ffmpeg](./ffmpeg-keyframe-animation) | CSS keyframes encoded as H.265 video.     |
| [ffplay](./ffplay)                    | Raw RGBA frames played as a moving clock. |

For installation in your own application, start with the [image guide](https://takumi.kane.tw/docs) or [PDF guide](https://takumi.kane.tw/docs/pdf).
