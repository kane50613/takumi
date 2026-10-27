import { expect, mock, spyOn, test } from "bun:test";
import { Renderer } from "@takumi-rs/core";
import { render } from "../src";

const raw = { width: 10, height: 10, format: "raw" } as const;

test("hands HTML to the renderer as written", async () => {
  const renderer = new Renderer();
  const call = spyOn(renderer, "render");
  const html = "<div>a</div>";

  await render(html, { ...raw, renderer });

  expect(call.mock.calls[0]?.[0]).toBe(html);
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
