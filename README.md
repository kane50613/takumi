<img src="./assets/images/takumi.svg" alt="Takumi" width="64" />

# Takumi

Takumi is an **image rendering engine** written in Rust. The design is mainly inspired by [satori](https://github.com/vercel/satori), but with a focus on portability and performance (2-10x compared to next/og in [Image Bench](https://image-bench.kane.tw/)).

You can try out Takumi in [Playground](https://takumi.kane.tw/playground) without installation. Or [Quick Start](https://takumi.kane.tw/docs/) page for usage.

> [!NOTE]
> The Showcase section is now open! Share your Takumi-powered projects by opening a pull request to [showcase.ts](./docs/app/data/showcase.ts)!

## Why build a satori alternative?

- All in one. No need to output SVG then have [resvg-js](https://github.com/thx/resvg-js) rendering it again to output PNG.
- Inline layout support (display `block` or `inline`).
- RTL support.
- Tailwind CSS support out of the box.
- Variable fonts support.
- COLR font support (e.g. twemoji-colr).
- WOFF2 font format support.
- PNG, JPEG, WebP output support.
- WebP, APNG animation rendering support.
- Low latency video rendering with [FFmpeg](https://ffmpeg.org/) ([example](./example/ffplay)).

## Goals

- Takes your existing JSX components and drops them in, it should just work.
- Minimal binary size targets to run everywhere.
- Be the go-to library for image/animation rendering.
- Have a great developer experience.

## Showcase

- Takumi's Open Graph Image is generated with Takumi [(source)](./example/twitter-images/components/og-image.tsx).

  ![Takumi OG Image](./example/twitter-images/output/og-image.png)

- X Open Graph Image [(source)](./example/twitter-images/components/x-post-image.tsx).

  ![X OG Image](./example/twitter-images/output/x-post-image.png)

- [shiki-image](https://github.com/pi0/shiki-image) Convert code snippets into images.

  ![Shiki Image Example](https://github.com/pi0/shiki-image/blob/main/test/.snapshot/image.webp)

- [(Unofficial) Takumi Playground](https://takumi-playground.kapadiya.net/) In-browser playground for designing and rendering Open Graph style images.

## Contributing

Contributions are welcome! Please feel free to open an [issue](https://github.com/kane50613/takumi/issues) or submit a pull request.

By participating in this project, you agree to abide by our [Code of Conduct](CODE_OF_CONDUCT.md).

You need to have Rust 1.89+ & Bun installed.

```bash
bun install
```

## Credits

Takumi wouldn't be possible without the following works:

- [taffy](https://github.com/DioxusLabs/taffy) for the flex & grid layout.
- [image](https://github.com/image-rs/image) for the image processing.
- [parley](https://github.com/linebender/parley) for text layout.
- [swash](https://github.com/linebender/swash) for font shaping.
- [wuff](https://github.com/nicoburns/wuff) for woff/woff2 decompression.
- [resvg](https://github.com/linebender/resvg) for SVG parsing & rasterization.

## License

Licensed under the terms in the [LICENSE](LICENSE) file.

<br/>
<a href="https://vercel.com/oss">
  <img alt="Vercel OSS Program" src="https://vercel.com/oss/program-badge.svg" />
</a>
