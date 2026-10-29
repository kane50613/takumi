import { describe, expect, mock, test } from "bun:test";
import { Renderer } from "../bundlers/node";

const renderer = new Renderer();
const html = `<style>.a { width: 4px; height: 4px; background-image: url(https://example.com/a.png) }</style><div class="a"></div>`;
const png = await renderer.render(
  { type: "container", style: { width: 2, height: 2, backgroundColor: "red" } },
  { width: 2, height: 2 },
);

describe("prepare", () => {
  test("fetches the images a stylesheet names and renders as a render given them", async () => {
    const fetch = mock(async (_url: string) => new Response(png));
    const prepared = await renderer.prepare(html, { width: 4, height: 4, images: { fetch } });

    expect(fetch.mock.calls.map(([url]) => url)).toEqual(["https://example.com/a.png"]);

    const direct = await renderer.render(html, {
      width: 4,
      height: 4,
      images: [{ src: "https://example.com/a.png", data: png }],
    });

    expect(await renderer.render(prepared)).toEqual(direct);
  });

  test("measures what it laid out", async () => {
    const prepared = await renderer.prepare("<p>hi</p>", { width: 40, height: 20 });

    expect(await renderer.measure(prepared)).toEqual(
      await renderer.measure("<p>hi</p>", { width: 40, height: 20 }),
    );
  });
});
