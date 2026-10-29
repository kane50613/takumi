import { expect, mock, test } from "bun:test";
import { render, renderAnimation } from "../src";

const node = (
  <div style={{ width: 10, height: 10, backgroundColor: "rgb(0, 0, 255)" }}>
    <img src="https://example.com/missing.png" style={{ width: 10, height: 10 }} />
  </div>
);
const raw = { width: 10, height: 10, format: "raw" } as const;
const notFound = async () => new Response(null, { status: 404 });

test("a failed image fetch rejects the render by default", async () => {
  await expect(render(node, { ...raw, images: { fetch: notFound } })).rejects.toThrow("HTTP 404");
});

test("throwOnError: false leaves a failed image out", async () => {
  const image = await render(node, { ...raw, images: { fetch: notFound, throwOnError: false } });

  expect(Array.from(image.slice(0, 4))).toEqual([0, 0, 255, 255]);
});

test("fetches an image only a stylesheet names", async () => {
  const fetch = mock(async (_url: string | URL | Request) => new Response(null, { status: 404 }));

  await render(<div className="bg" />, {
    ...raw,
    css: ".bg { width: 10px; height: 10px; background-image: url(https://example.com/bg.png) }",
    images: { fetch, throwOnError: false },
  });

  expect(fetch.mock.calls.map(([url]) => String(url))).toEqual(["https://example.com/bg.png"]);
});

test("fetches what a scene draws under another HTML scene's <style>", async () => {
  const fetch = mock(async (_url: string | URL | Request) => new Response(null, { status: 404 }));

  await renderAnimation({
    width: 10,
    height: 10,
    fps: 10,
    scenes: [
      {
        node: "<style>.bg { width: 10px; height: 10px; background-image: url(https://example.com/bg.png) }</style><div></div>",
        durationMs: 100,
      },
      { node: '<div class="bg"></div>', durationMs: 100 },
    ],
    images: { fetch, throwOnError: false },
    emoji: "from-font",
  });

  expect(fetch.mock.calls.map(([url]) => String(url))).toEqual(["https://example.com/bg.png"]);
});
