<div align="center">
  <img src="./assets/images/takumi.svg" alt="Takumi" width="64" />

# Takumi

**Turn JSX into production-ready images fast.**  
OG cards, banners, and lightweight animations from one Rust engine for Node.js and WebAssembly.

[Docs](https://takumi.kane.tw/docs/) · [Playground](https://takumi.kane.tw/playground)

</div>

Takumi is inspired by [satori](https://github.com/vercel/satori), with a stronger focus on portability and performance.

## Why teams pick Takumi

- **Direct image rendering** with no SVG-to-image conversion step.
- **One JSX pipeline across runtimes** for Node.js, browsers, and edge workers.
- **Native performance on Node.js** with WebAssembly fallback when portability matters.
- **Production-grade text and font support** including variable fonts, COLR, WOFF2, and RTL.
- **Flexible output targets** from WebP, PNG, JPEG, and GIF to raw frames for FFmpeg pipelines.

## Performance

Takumi is built for fast image rendering across Node.js and WebAssembly runtimes.
See current benchmark runs and templates on [Image Bench](https://image-bench.kane.tw/), including comparisons with `next/og`.

## First render in 30 seconds

```bash
bun i takumi-js
```

```tsx
import { ImageResponse } from "takumi-js/response";
import { serve } from "bun";

serve({
  fetch() {
    return new ImageResponse(
      <div tw="w-full h-full flex items-center justify-center bg-[linear-gradient(to_bottom,#dbf4ff,#fff1f1)]">
        <h1 tw="text-6xl font-bold">Hello from Takumi 👋😁</h1>
      </div>,
      {
        width: 1200,
        height: 630,
      },
    );
  },
  port: 3000,
});
```

## What you can build

- Open Graph and social cards
- Blog covers and marketing banners
- JSX-powered dynamic snapshots
- Component-based motion graphics

## Showcase

|                                 Takumi OG image [(source)](./example/twitter-images/components/og-image.tsx)                                 |                Package OG card [(ssource)](./example/twitter-images/components/package-og-image.tsx)                |
| :------------------------------------------------------------------------------------------------------------------------------------------: | :-----------------------------------------------------------------------------------------------------------------: |
|                                       ![Takumi OG Image](./example/twitter-images/output/og-image.png)                                       |                      ![Package OG Image](./example/twitter-images/output/package-og-image.png)                      |
|                        **Prisma-style API card** [(source)](./example/twitter-images/components/prisma-og-image.tsx)                         |              **X-style social post** [(source)](./example/twitter-images/components/x-post-image.tsx)               |
|                                   ![Prisma OG Image](./example/twitter-images/output/prisma-og-image.png)                                    |                       ![X-style Post Image](./example/twitter-images/output/x-post-image.png)                       |
|                             **Keyframe Animation** [(source)](./example/ffmpeg-keyframe-animation/src/index.tsx)                             |                                **[shiki-image](https://github.com/pi0/shiki-image)**                                |
| [![Keyframe Animation](./example/ffmpeg-keyframe-animation/output/thumbnail.webp)](./example/ffmpeg-keyframe-animation/output/animation.mp4) | ![Shiki Image Example](https://raw.githubusercontent.com/pi0/shiki-image/refs/heads/main/test/.snapshot/image.webp) |

- [(Unofficial) Takumi Playground](https://takumi-playground.kapadiya.net/)

> [!NOTE]
> Showcase submissions are welcome via PR to [showcase.ts](./docs/app/data/showcase.ts).

## Contributing

PRs are welcome. See [CONTRIBUTING.md](./CONTRIBUTING.md) for setup, tests, fixtures, and changesets.
By participating, you agree to the [Code of Conduct](./CODE_OF_CONDUCT.md).

## Credits

Takumi builds on excellent OSS:
[taffy](https://github.com/DioxusLabs/taffy),
[image](https://github.com/image-rs/image),
[parley](https://github.com/linebender/parley),
[swash](https://github.com/linebender/swash),
[wuff](https://github.com/nicoburns/wuff),
[resvg](https://github.com/linebender/resvg).

## License

Licensed under either of:

- [Apache License, Version 2.0](./LICENSE-APACHE)
- [MIT license](./LICENSE-MIT)

<br/>
<a href="https://vercel.com/oss">
  <img alt="Vercel OSS Program" src="https://vercel.com/oss/program-badge.svg" />
</a>
