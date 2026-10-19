# Generate images in TanStack Start

A TanStack Start server route that returns a 1200 × 630 WebP card with `ImageResponse` from `takumi-js`. The dev server runs the route in the Cloudflare Workers runtime through the Cloudflare Vite plugin.

## Run

Build the [image example packages](../README.md#image-examples) first. Then start the dev server with [portless](https://github.com/vercel-labs/portless):

```bash
cd example/tanstack-start
portless run --name takumi-tanstack vite dev
```

Open `https://takumi-tanstack.localhost/` to see the card. Its title shows the request's host.

## Notes

- The route is [`src/routes/index.tsx`](./src/routes/index.tsx).
- The route loads Geist from `takumi.kane.tw`, so rendering needs network access.
- `bun run test` builds the app and checks that the server bundle includes the WebAssembly binary and leaves out the native `@takumi-rs/core` binding.
