# Generate images in a Next.js route

An App Router route that returns a 1200 × 630 WebP card with `ImageResponse` from `takumi-js`. The `name` query parameter sets the title.

## Run

Build the [image example packages](../README.md#image-examples) first. Then start Next.js with [portless](https://github.com/vercel-labs/portless):

```bash
cd example/nextjs
portless run --name takumi-nextjs next dev
```

Open `https://takumi-nextjs.localhost/?name=Kane` to see the card.

## Notes

- The route is [`app/route.tsx`](./app/route.tsx). It sets `runtime = "nodejs"`, so `takumi-js` uses the native binding.
- [`next.config.ts`](./next.config.ts) lists `@takumi-rs/core` in `serverExternalPackages`. Next.js then loads the native addon from `node_modules` instead of bundling it.
