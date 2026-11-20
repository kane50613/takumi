---
packages:
  takumi-pdf: patch
---

# `render()` returns bytes you can pass to `Response`

`render()` was typed to return `Uint8Array<ArrayBufferLike>`, which TypeScript 5.7 and later reject as a `BodyInit`, so `new Response(pdf)` failed to typecheck. It now returns `Uint8Array<ArrayBuffer>`, as `takumi-js` does.

```ts
const pdf = await render(html);

return new Response(pdf, { headers: { "content-type": "application/pdf" } });
```
