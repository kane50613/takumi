# Generate images in Waku

A Waku API route that returns a 1200 × 630 PNG with `ImageResponse` from `takumi-js`. The route is [`src/pages/_api/image.png.tsx`](./src/pages/_api/image.png.tsx), and it renders on each request.

## Run

Build the [image example packages](../README.md#image-examples) first. Then start Waku with [portless](https://github.com/vercel-labs/portless):

```bash
cd example/waku-ssr
portless run --name takumi-waku waku dev
```

Open `https://takumi-waku.localhost/api/image.png` to see the image.
