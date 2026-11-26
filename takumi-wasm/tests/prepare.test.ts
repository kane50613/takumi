import { describe, expect, mock, test } from "bun:test";
import { container, text } from "@takumi-rs/helpers";
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

  test("reads a tree past what JSON carries once per prepare and its render", async () => {
    let reads = 0;
    const counted = text("hi");

    Object.defineProperty(counted, "style", {
      enumerable: true,
      get: () => {
        reads++;

        return { color: "red" };
      },
    });

    const tree = container({
      children: [text("x".repeat(64 * 1024 + 1), { display: "none" }), counted],
    });
    const read = async () => renderer.render(await renderer.prepare(tree, { width: 8, height: 8 }));

    await read();
    expect(reads).toBe(1);

    await read();
    expect(reads).toBe(2);
  });
});
