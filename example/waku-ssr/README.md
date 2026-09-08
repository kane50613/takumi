# Generate images in Waku

Return a PNG from a Waku server route with `ImageResponse`. The route in [`src/pages/_api/image.png.tsx`](./src/pages/_api/image.png.tsx) renders a 1200 × 630 card on each request.

## Run

Install dependencies and build the workspace packages from the repository root:

```bash
bun install
bun --filter '*' run build
```

Start the example with portless:

```bash
cd example/waku-ssr
portless run --name takumi-waku waku dev
```

Open `https://takumi-waku.localhost/api/image.png` to view the generated image.
