import { expect, mock, spyOn, test } from "bun:test";
import { Renderer } from "@takumi-rs/core";
import { render, renderAnimation } from "../src";

const raw = { width: 10, height: 10, format: "raw" } as const;

test("hands HTML to the renderer as written when emoji come from the fonts", async () => {
  const renderer = new Renderer();
  const call = spyOn(renderer, "prepare");
  const html = "<div>a</div>";

  await render(html, { ...raw, renderer, emoji: "from-font" });

  expect(call.mock.calls[0]?.[0]).toBe(html);
});

test("parses HTML into a node tree to split its emoji from a CDN", async () => {
  const renderer = new Renderer();
  const call = spyOn(renderer, "render");

  await render("<div>a</div>", { ...raw, renderer });

  expect(typeof call.mock.calls[0]?.[0]).toBe("object");
});

test("fetches the emoji and stylesheet images HTML draws", async () => {
  const fetch = mock(async (_url: string | URL | Request) => new Response(null, { status: 404 }));

  await render(
    '<style>.bg { background-image: url(https://example.com/bg.png) }</style><div class="bg">Hi 😀</div>',
    { ...raw, images: { fetch, throwOnError: false } },
  );

  expect(fetch.mock.calls.map(([url]) => String(url)).sort()).toEqual([
    "https://cdn.jsdelivr.net/gh/jdecked/twemoji@17.0.2/assets/svg/1f600.svg",
    "https://example.com/bg.png",
  ]);
});

test("hands an HTML scene to the renderer as written when emoji come from the fonts", async () => {
  const renderer = new Renderer();
  const call = spyOn(renderer, "prepareAnimation");
  const html = "<style>p { color: red }</style><p>a</p>";

  await renderAnimation({
    renderer,
    width: 10,
    height: 10,
    fps: 1,
    format: "gif",
    emoji: "from-font",
    scenes: [{ node: html, durationMs: 1000 }],
  });

  expect(call.mock.calls[0]?.[0].scenes[0]?.node).toBe(html);
});

test("parses an HTML scene into a node tree to split its emoji from a CDN", async () => {
  const renderer = new Renderer();
  const call = spyOn(renderer, "prepareAnimation");

  await renderAnimation({
    renderer,
    width: 10,
    height: 10,
    fps: 1,
    format: "gif",
    scenes: [{ node: "<p>a</p>", durationMs: 1000 }],
  });

  expect(typeof call.mock.calls[0]?.[0].scenes[0]?.node).toBe("object");
});
