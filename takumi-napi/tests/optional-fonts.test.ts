import { afterAll, describe, expect, test } from "bun:test";
import { container, fontFromUrl, text } from "@takumi-rs/helpers";
import { type FontLoader, Renderer } from "../src/export";

const monoData = await Bun.file("../assets/fonts/geist/GeistMono[wght].woff2").arrayBuffer();

const server = Bun.serve({
  port: 0,
  fetch: () => new Response(null, { status: 404 }),
});

afterAll(() => server.stop());

const missing = `${server.url}missing.woff2`;
const node = container({
  style: { width: 256, height: 64, fontSize: 32, fontFamily: "Mono", backgroundColor: "#fff" },
  children: [text("0O1lI")],
});
const options = { width: 256, height: 64 } as const;
const mono = { name: "Mono", data: monoData } as const;

describe("an optional font", () => {
  const failing = {
    "that fails to download": fontFromUrl(missing, { optional: true }),
    "whose bytes do not decode": { name: "Garbled", optional: true, data: new Uint8Array([1, 2]) },
  } satisfies Record<string, FontLoader>;

  for (const [name, font] of Object.entries(failing)) {
    test(`${name} leaves the render as it is without the font`, async () => {
      const fonts: FontLoader[] = [mono, font];
      const renderer = new Renderer();
      const without = await renderer.render(node, { ...options, fonts: [mono] });

      expect(await renderer.render(node, { ...options, fonts })).toEqual(without);
    });
  }
});

describe("a font without optional", () => {
  test("rejects the render when it fails to download", async () => {
    await expect(
      new Renderer().render(node, { ...options, fonts: [mono, missing] }),
    ).rejects.toThrow("HTTP 404");
  });

  test("rejects the render when its bytes do not decode", async () => {
    await expect(
      new Renderer().render(node, {
        ...options,
        fonts: [mono, { name: "Garbled", data: new Uint8Array([1, 2]) }],
      }),
    ).rejects.toThrow();
  });
});
