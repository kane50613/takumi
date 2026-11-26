import { describe, expect, mock, test } from "bun:test";
import { readFile } from "node:fs/promises";
import { join } from "node:path";
import init, { Renderer } from "@takumi-rs/wasm/no-bundler";

// The behaviour nuxt-og-image relies on, initialized the way its WASM binding does:
// https://github.com/nuxt-modules/og-image/blob/5f191b7dbed829a192cd66fffec7426c07fdc9c7/src/runtime/server/og-image/bindings/takumi/wasm.ts
// https://github.com/nuxt-modules/og-image/blob/d56c0cfe1a433c9dab7e2212e6d9b4f2f8e53b82/src/runtime/server/og-image/takumi/renderer.ts

const fontBytes = await readFile(
  join(import.meta.dir, "../../assets/fonts/geist/Geist[wght].woff2"),
);
await init({
  module_or_path: await WebAssembly.compile(
    await readFile(join(import.meta.dir, "../pkg/takumi_wasm_bg.wasm")),
  ),
});

const html = `<p style="font-family: Downstream">Hi</p>`;
const layout = { width: 40, height: 20, fontFamilies: ["Downstream"] };

/** A face of the `Downstream` family covering `range`, loaded the way og-image loads it. */
const loader = (range: [number, number], data: () => Promise<Uint8Array>) => ({
  name: "Downstream",
  data: mock(data),
  weight: 400,
  style: "normal" as const,
  unicodeRange: [range],
});

describe("nuxt-og-image", () => {
  test("calls a font loader only for the text that needs it", async () => {
    const latin = loader([0x20, 0x7f], async () => fontBytes);
    const greek = loader([0x370, 0x3ff], async () => fontBytes);

    await new Renderer().prepare(html, { ...layout, fonts: [latin, greek] });

    expect(latin.data).toHaveBeenCalledTimes(1);
    expect(greek.data).not.toHaveBeenCalled();
  });

  test("rejects prepare when a font loader fails", async () => {
    const failing = loader([0x20, 0x7f], async () => {
      throw new Error("font loader failed");
    });

    await expect(new Renderer().prepare(html, { ...layout, fonts: [failing] })).rejects.toThrow(
      "font loader failed",
    );
  });

  test("renders without an image that answers 404 when throwOnError is false", async () => {
    const renderer = new Renderer();
    const prepared = await renderer.prepare(`<img src="/missing.png" width="4" height="4">`, {
      width: 4,
      height: 4,
      images: {
        sources: [],
        fetch: async () => new Response(null, { status: 404 }),
        allowUrl: (src) => src.startsWith("/"),
        timeout: 1000,
        throwOnError: false,
      },
    });

    expect((await renderer.render(prepared, { format: "png" })).length).toBeGreaterThan(0);
  });

  test("renders raw frames of width * height * 4 bytes", async () => {
    const renderer = new Renderer();
    const prepared = await renderer.prepare("<p>Hi</p>", {
      width: 40,
      height: 20,
      devicePixelRatio: 2,
      timeMs: 0,
    });

    expect((await renderer.render(prepared, { format: "raw" })).length).toBe(40 * 20 * 4);
  });
});
