# @takumi-rs/image-response

A universal `ImageResponse` implementation for Takumi in Next.js and other environments.

Checkout the migration guide [From Next.js ImageResponse](https://takumi.kane.tw/docs/migration/migrate-from-image-response) for more details.

## Installation

```bash
npm install @takumi-rs/image-response
```

## Usage

```tsx
import ImageResponse from "@takumi-rs/image-response";

export function GET(request: Request) {
  return new ImageResponse(<OgImage />, {
    width: 1200,
    height: 630,
    format: "webp",
    headers: {
      "Cache-Control": "public, immutable, max-age=31536000",
    },
  });
}
```

For shared configuration and cache isolation, create your own image response factory.

```tsx
import { createImageResponse } from "@takumi-rs/image-response";

const ogImage = createImageResponse({
  fonts: [
    {
      data: () => fetch("/fonts/Inter-Regular.ttf").then((res) => res.arrayBuffer()),
      key: "inter-regular",
      name: "Inter",
    },
  ],
});

export function GET() {
  return ogImage(<OgImage />);
}
```

### Fonts

Takumi comes with full-axis [Geist](https://vercel.com/font) and Geist Mono by default.

We have a global fonts cache to avoid loading the same fonts multiple times.

If your environment supports top-level await, you can load the fonts in global scope and reuse the fonts array.

```tsx
const fonts = [
  {
    name: "Inter",
    data: await fetch("/fonts/Inter-Regular.ttf").then((res) => res.arrayBuffer()),
    style: "normal",
    weight: 400,
  },
];

new ImageResponse(<OgImage />, { fonts });
```

If your environment doesn't support top-level await, you can pass a lazy `data` loader instead.

```tsx
export function GET(request: Request) {
  return new ImageResponse(<OgImage />, {
    fonts: [
      {
        name: "Inter",
        data: () => fetch("/fonts/Inter-Regular.ttf").then((res) => res.arrayBuffer()),
        style: "normal",
        weight: 400,
      },
    ],
  });
}
```

The same pattern also works for `persistentImages`. Caches are scoped to each `createImageResponse()` instance.

`loadDefaultFonts` is only supported by the native `@takumi-rs/core` renderer. It has no effect when using the WASM renderer.

### Bring-Your-Own-Renderer (BYOR)

If you want to use your own renderer instance, you can pass it to the `ImageResponse` constructor.

```tsx
import { Renderer } from "@takumi-rs/core";

const renderer = new Renderer();

new ImageResponse(<OgImage />, { renderer });
```

### JSX Options

You can pass the JSX options to the `ImageResponse` constructor.

```tsx
new ImageResponse(<OgImage />, {
  jsx: {
    defaultStyles: false,
  },
});
```

### Error Handling

`ImageResponse` exposes a `ready` promise that resolves when rendering succeeds and rejects when rendering fails.

```tsx
const response = new ImageResponse(<OgImage />);

await response.ready;
return response;
```

You can also provide `onError` to render a fallback image instead of failing the response stream.

```tsx
const response = new ImageResponse(<OgImage />, {
  onError: () => <div>Failed to generate image</div>,
});
```
