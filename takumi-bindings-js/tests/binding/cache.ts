import { describe, expect, test } from "bun:test";
import { type CacheStats, container, image, type RendererOptions, text } from "@takumi-rs/helpers";
import { testFontEviction } from "./font-eviction";

/** A binding's `Renderer`, as the suites every binding runs drive it. */
export type BindingRenderer = new (options?: RendererOptions) => {
  /** `options` stays loose, since each binding types its render options apart. */
  render(source: unknown, options?: object): Promise<Uint8Array>;
  prepare(source: string, options: object): Promise<unknown>;
  registerFont(font: ArrayBuffer | { name: string; data: Uint8Array }): Promise<unknown>;
  cacheStats(): CacheStats;
};

const imageData = await Bun.file("../assets/images/yeecord.png").arrayBuffer();
const monoData = await Bun.file("../assets/fonts/geist/GeistMono[wght].woff2").arrayBuffer();
const sansData = await Bun.file("../assets/fonts/geist/Geist[wght].woff2").arrayBuffer();

/** Runs the cache suites against a binding's `Renderer`. */
export function testCache(Renderer: BindingRenderer): void {
  testCacheStats(Renderer);
  testFontCache(Renderer);
  testFontEviction(Renderer);
}

function testCacheStats(Renderer: BindingRenderer) {
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
}

function testFontCache(Renderer: BindingRenderer) {
  const node = (fontFamily: string) =>
    container({
      style: { width: 256, height: 64, fontFamily, fontSize: 32, backgroundColor: "#fff" },
      children: [text("mono 0O1lI")],
    });

  const options = { width: 256, height: 64 } as const;

  /** A font named `name` that counts its loads. */
  const counted = (name: string, data: ArrayBuffer) => {
    const font = {
      name,
      loads: 0,
      data: () => {
        font.loads++;
        return data;
      },
    };

    return font;
  };

  describe("fontCacheMaxBytes", () => {
    test("of 0 releases each font once its render settles, and loads it again when a render lists it", async () => {
      const renderer = new Renderer({ fontCacheMaxBytes: 0 });
      const mono = counted("Mono", monoData);
      const sans = counted("Sans", sansData);

      const first = await renderer.render(node("Mono"), { ...options, fonts: [mono] });
      await renderer.render(node("Sans"), { ...options, fonts: [sans] });
      const again = await renderer.render(node("Mono"), { ...options, fonts: [mono] });

      expect(mono.loads).toBe(2);
      expect(Buffer.compare(again, first)).toBe(0);
    });

    test("keeps none of them when cache.fonts is none", async () => {
      const renderer = new Renderer({ cache: { fonts: "none" } });
      const mono = counted("Mono", monoData);

      await renderer.render(node("Mono"), { ...options, fonts: [mono] });
      await renderer.render(node("Sans"), { ...options, fonts: [counted("Sans", sansData)] });
      await renderer.render(node("Mono"), { ...options, fonts: [mono] });

      expect(mono.loads).toBe(2);
    });

    test("keeps fonts from registerFont", async () => {
      const renderer = new Renderer({ fontCacheMaxBytes: 1 });

      await renderer.registerFont(monoData);
      const registered = await renderer.render(node("Geist Mono"), options);

      await renderer.render(node("Sans"), { ...options, fonts: [counted("Sans", sansData)] });

      expect(Buffer.compare(await renderer.render(node("Geist Mono"), options), registered)).toBe(
        0,
      );
    });

    test("keeps every font without a budget", async () => {
      const renderer = new Renderer();
      const mono = counted("Mono", monoData);

      await renderer.render(node("Mono"), { ...options, fonts: [mono] });
      await renderer.render(node("Sans"), { ...options, fonts: [counted("Sans", sansData)] });
      await renderer.render(node("Mono"), { ...options, fonts: [mono] });

      expect(mono.loads).toBe(1);
    });
  });

  describe("fonts with cache none", () => {
    const listed = (cache?: "none") => ({
      ...options,
      fonts: [{ name: "Mono", key: "mono", data: () => monoData, cache }],
    });

    test("leave the renderer once their render settles", async () => {
      const renderer = new Renderer();

      await renderer.render(node("Mono"), listed("none"));

      expect(renderer.cacheStats().fonts.current).toBe(0);
    });

    test("use a font the renderer holds and leave it there", async () => {
      const renderer = new Renderer();

      await renderer.render(node("Mono"), listed());
      const held = renderer.cacheStats().fonts;
      await renderer.render(node("Mono"), listed("none"));
      const after = renderer.cacheStats().fonts;

      expect(held.current).toBeGreaterThan(0);
      expect(after.current).toBe(held.current);
      expect(after.misses).toBe(held.misses);
    });
  });
}
