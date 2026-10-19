# @takumi-rs/image-response

**This package moved into [`takumi-js`](https://www.npmjs.com/package/takumi-js). Import `ImageResponse` from `takumi-js/response`.**

`@takumi-rs/image-response` now only re-exports `takumi-js/response`. Install `takumi-js` for new projects:

```bash
npm install takumi-js
```

```tsx
import { ImageResponse } from "takumi-js/response";

export function GET() {
  return new ImageResponse(<div tw="w-full h-full flex items-center justify-center">Hello</div>, {
    width: 1200,
    height: 630,
  });
}
```

For existing code, change the import path. The options stay the same. See the [v1 upgrade guide](https://takumi.kane.tw/docs/upgrade/v1#unified-runtime-detection-and-fallback) and the [`ImageResponse` reference](https://takumi.kane.tw/docs/image-response).
