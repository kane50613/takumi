<div align="center">
  <img src="./assets/images/takumi.svg" alt="Takumi" width="64" />

# Takumi

**Turn JSX into production-ready images fast.**  
OG cards, banners, and lightweight animations from one Rust engine for Node.js and WebAssembly.

[Docs](https://takumi.kane.tw/docs/) · [Playground](https://takumi.kane.tw/playground)

</div>

## First render in 30 seconds

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
        format: "webp",
        emoji: "twemoji",
      },
    );
  },
  port: 3000,
});
```
