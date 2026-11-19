# From satori, `next/og`, or `@vercel/og`

[migration/satori.md](https://takumi.kane.tw/docs/migration/satori.md) shows each swap and compares the features.

## Swap

- `new ImageResponse(...)` from `next/og` or `@vercel/og` becomes the same call from `takumi-js/response`.
- `satori(...)` becomes `renderSvg(...)` from `takumi-js`, which returns the SVG as a string.
- `satori` followed by resvg or sharp becomes one `render(...)` call that returns PNG, JPEG, or WebP.

Pass `width` and `height` to every call. `next/og` defaults to 1200 × 630, while Takumi sizes an omitted dimension to the content.

`next/og` and `@vercel/og` make every element a flex container. In Takumi, a `<div>` is a block, as in a browser. Add `display: flex` or `tw="flex"` to each container whose children sat in a row or used flex alignment, and leave the rest as blocks. The swap is done when every container that relied on the implicit flex declares it.

## Workarounds to delete

- base64 data URLs for images, replaced by the remote URL or an `images: [{ src, data }]` entry. When a loader limited which URLs could become a data URL, set an equivalent `images.allowUrl` for the remote URL, or keep that loader and pass its bytes through `images`.
- hand-drawn SVG standing in for gradients, masks, `clip-path`, or `background-image`
- colors converted from `oklch()`, `color-mix()`, or `var()` tokens to hex
- static font instances chosen because variable fonts drew at their default
- `loadAdditionalAsset` callbacks for emoji, replaced by the `emoji` option
- loops that call `measure` or estimate text width to pick a font size, replaced by `text-fit`
- a `try`/`catch` around `new ImageResponse(...)`, replaced by awaiting `response.ready`
