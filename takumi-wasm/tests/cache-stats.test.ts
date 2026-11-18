import { describe, expect, test } from "bun:test";
import { container, image, text } from "@takumi-rs/helpers";
import { Renderer } from "../bundlers/node";

const imageData = await Bun.file("../assets/images/yeecord.png").arrayBuffer();
const monoData = await Bun.file("../assets/fonts/geist/GeistMono[wght].woff2").arrayBuffer();

const node = container({
  style: { width: 256, height: 64, fontSize: 32, fontFamily: "Mono", backgroundColor: "#fff" },
  children: [image({ src: "logo", width: 32, height: 32 }), text("cached 0O1lI")],
});

const options = {
  width: 256,
  height: 64,
  images: [{ src: "logo", data: imageData }],
  fonts: [{ name: "Mono", data: monoData }],
};

describe("cacheStats", () => {
  test("reports what a render keeps, by kind", async () => {
    const renderer = new Renderer();

    await renderer.render(node, options);
    const first = renderer.cacheStats();
    await renderer.render(node, options);
    const second = renderer.cacheStats();

    expect(first.images.current).toBeGreaterThan(0);
    expect(first.images.misses).toBeGreaterThan(0);
    expect(first.glyphs.current).toBeGreaterThan(0);
    expect(first.glyphs.misses).toBeGreaterThan(0);
    expect(first.fonts.current).toBeGreaterThan(0);
    expect(first.memory.current).toBeGreaterThanOrEqual(
      first.images.current + first.glyphs.current,
    );
    expect(second.images.hits).toBeGreaterThan(first.images.hits);
    expect(second.glyphs.hits).toBeGreaterThan(first.glyphs.hits);
    expect(second.fonts.hits).toBeGreaterThan(first.fonts.hits);
  });
});

describe("cache options", () => {
  test("keep nothing when maxBytes is 0", async () => {
    const renderer = new Renderer({ cache: { maxBytes: 0 } });

    await renderer.render(node, options);
    const stats = renderer.cacheStats();

    expect(stats.memory.current).toBe(0);
    expect(stats.images.hits).toBe(0);
  });

  test("keep none of a kind whose mode is none", async () => {
    const renderer = new Renderer({ cache: { images: "none", glyphs: "none" } });

    await renderer.render(node, options);
    await renderer.render(node, options);
    const stats = renderer.cacheStats();

    expect(stats.images).toMatchObject({ current: 0, hits: 0 });
    expect(stats.glyphs).toMatchObject({ current: 0, hits: 0 });
  });
});
