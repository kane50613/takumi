# @takumi-rs/image-response

> **Use [`takumi-js/response`](https://www.npmjs.com/package/takumi-js).**

For new projects and migrations, install `takumi-js` and import `ImageResponse` from `takumi-js/response`:

```bash
npm install takumi-js
```

```tsx
import { ImageResponse } from "takumi-js/response";

export function GET() {
  return new ImageResponse(<OgImage />, { width: 1200, height: 630 });
}
```

See the [migration guide](https://takumi.kane.tw/docs/upgrade/v2) and the [`ImageResponse` reference](https://takumi.kane.tw/docs/image-response) for details.
