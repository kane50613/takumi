---
name: migrate-from-satori
description: Migrate code from satori, `next/og`, or `@vercel/og` to Takumi. Use when the user wants to replace satori or `next/og`, or code imports `satori`, `next/og`, `@vercel/og`, or `@resvg/resvg-js` to make an image.
---

# Migrate satori and `next/og` to Takumi

Takumi takes the same JSX and returns the image in one call. The migration is a swap plus a cleanup: most of what satori code does around the render is a workaround Takumi does not need. The [migration guide](https://takumi.kane.tw/docs/migration/satori.md) holds the details. Pair this skill with `takumi-usage` for the API rules.

## Steps

1. **Swap the entry.** `new ImageResponse(...)` from `next/og` or `@vercel/og` becomes the same call from `takumi-js/response`. `satori(...)` becomes `renderSvg(...)` from `takumi-js`, and `satori` followed by resvg or sharp becomes one `render(...)` call. Done when no import of `satori`, `next/og`, `@vercel/og`, or `@resvg/resvg-js` remains in the code that makes images.

2. **Restore Flexbox where `next/og` implied it.** `next/og` makes every element a flex container. In Takumi a `<div>` is `display: block`, as in a browser. Add `display: flex` or `tw="flex"` to each container whose children sat in a row or used flex alignment. Done when every container that relied on implicit flex declares it, and no other container gained one.

3. **Move fonts to `fonts`.** Keep Geist, the built-in Latin font, or pass the brand font. Replace hand-written Google Fonts fetching with `googleFonts` from `takumi-js/helpers`. Keep `fonts` entries as URL strings, bytes, or `{ name, data, weight, style }`, and drop `weight` for a variable font. Done when every font the old code read, fetched, or embedded is a `fonts` entry or `googleFonts` call, and no code parses Google Fonts CSS.

4. **Delete the workarounds.** Remove each piece that existed only because satori lacked a feature:
   - base64 data URLs for images, replaced by `images: [{ src, data }]` or the remote URL itself
   - hand-drawn SVG standing in for gradients, masks, `background-image`, or `clip-path`
   - colors converted from `oklch()` or `var()` tokens to hex
   - static font instances chosen because variable fonts did not work
   - `loadAdditionalAsset` callbacks for emoji, replaced by the `emoji` option

   Done when each remaining helper around the render does something Takumi cannot.

5. **Handle errors on the response.** A `try`/`catch` around `new ImageResponse(...)` never sees a render error, because the body streams. Await `response.ready` and return a fallback when it rejects. Done when every route that had error handling awaits `ready`.

6. **Compare the output.** Render each template with the old and new code and compare the images. Spacing can move where satori's layout differed from a browser. Done when each difference is either fixed or confirmed to match a browser.
