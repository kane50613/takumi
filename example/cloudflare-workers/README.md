# Render images on Cloudflare Workers

A Worker that returns a 1200 × 630 card with `ImageResponse` from `takumi-js`. On Workers, `takumi-js` renders with the WebAssembly binding. Try the [deployed example](https://takumi-on-workers-example.kane.tw/).

## Run

Build the [image example packages](../README.md#image-examples) first. Then start the Worker with Wrangler:

```bash
cd example/cloudflare-workers
bun run dev
```

Open the URL Wrangler prints. Add `?name=Kane` to change the title.

## Notes

- The request handler is in [`src/index.tsx`](./src/index.tsx).
- The card loads the Takumi logo from `takumi.kane.tw`, so rendering needs network access. A shared `fetchCache` keeps one copy of the logo across requests.
- `bun run test` builds the Worker, starts it, and checks that it returns a PNG.
