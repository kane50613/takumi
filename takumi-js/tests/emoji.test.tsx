import { expect, mock, test } from "bun:test";
import { render } from "../src";

const twemoji = "https://cdn.jsdelivr.net/gh/jdecked/twemoji@17.0.2/assets/svg/1f600.svg";
const red =
  '<svg xmlns="http://www.w3.org/2000/svg" width="1" height="1"><rect width="1" height="1" fill="red"/></svg>';
const raw = { width: 20, height: 20, format: "raw" } as const;

function stubFetch() {
  return mock(
    async (_url: string | URL | Request) =>
      new Response(red, { headers: { "content-type": "image/svg+xml" } }),
  );
}

/** The RGBA of the pixel at the center of a raw 20 by 20 render. */
function center(image: Uint8Array): number[] {
  const offset = (10 * 20 + 10) * 4;

  return Array.from(image.slice(offset, offset + 4));
}

test("draws the emoji in an HTML string as an image from the CDN", async () => {
  const fetch = stubFetch();
  const image = await render('<div style="font-size: 20px">😀</div>', {
    ...raw,
    images: { fetch },
  });

  expect(fetch.mock.calls.map(([url]) => String(url))).toEqual([twemoji]);
  expect(center(image)).toEqual([255, 0, 0, 255]);
});

test("draws the emoji in JSX as an image from the CDN", async () => {
  const fetch = stubFetch();
  const image = await render(<div style={{ fontSize: 20 }}>😀</div>, {
    ...raw,
    images: { fetch },
  });

  expect(fetch.mock.calls.map(([url]) => String(url))).toEqual([twemoji]);
  expect(center(image)).toEqual([255, 0, 0, 255]);
});
