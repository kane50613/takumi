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

    expect(first.budgets.images.current).toBeGreaterThan(0);
    expect(first.budgets.images.misses).toBeGreaterThan(0);
    expect(first.budgets.glyphs.current).toBeGreaterThan(0);
    expect(first.budgets.glyphs.misses).toBeGreaterThan(0);
    expect(first.budgets.fonts.current).toBeGreaterThan(0);
    expect(first.budgets.images.max).toBeNull();
    expect(first.memory.max).toBe(128 * 1024 * 1024);
    expect(first.memory.current).toBeGreaterThanOrEqual(
      first.budgets.images.current + first.budgets.glyphs.current,
    );
    expect(second.budgets.images.hits).toBeGreaterThan(first.budgets.images.hits);
    expect(second.budgets.glyphs.hits).toBeGreaterThan(first.budgets.glyphs.hits);
    expect(second.budgets.fonts.hits).toBeGreaterThan(first.budgets.fonts.hits);
  });
});

describe("setCacheOptions", () => {
  test("empties the pool right away when maxBytes is 0", async () => {
    const renderer = new Renderer();

    await renderer.render(node, options);
    const before = renderer.cacheStats();
    renderer.setCacheOptions({ maxBytes: 0 });
    const after = renderer.cacheStats();

    expect(before.memory.current).toBeGreaterThan(0);
    expect(after.memory).toEqual({ current: 0, high: before.memory.high, max: 0 });
    expect(after.budgets.glyphs.evictions).toBeGreaterThan(0);
  });

  test("gives a kind a budget, and the fonts a smaller one", async () => {
    const renderer = new Renderer();

    await renderer.render(node, options);
    renderer.setCacheOptions({ budgets: { images: 1024 * 1024, fonts: 0 } });
    const stats = renderer.cacheStats();

    expect(stats.budgets.images.max).toBe(1024 * 1024);
    expect(stats.budgets.fonts).toMatchObject({ current: 0, max: 0, evictions: 1 });

    await renderer.render(node, options);

    expect(renderer.cacheStats().budgets.images.current).toBeGreaterThan(0);
  });
});
