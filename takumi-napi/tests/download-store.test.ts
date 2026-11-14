import { afterAll, describe, expect, test } from "bun:test";
import { container, image, MemoryStore, text } from "@takumi-rs/helpers";
import { Renderer } from "../src/export";

const imageData = await Bun.file("../assets/images/yeecord.png").arrayBuffer();
const monoData = await Bun.file("../assets/fonts/geist/GeistMono[wght].woff2").arrayBuffer();

const requests = new Map<string, number>();
const server = Bun.serve({
  port: 0,
  fetch(request) {
    const { pathname } = new URL(request.url);

    requests.set(pathname, (requests.get(pathname) ?? 0) + 1);

    return new Response(imageData);
  },
});

afterAll(() => server.stop());

/** A tree drawing the image the local server serves at `path`. */
const drawing = (path: string) =>
  container({
    style: { width: 64, height: 64 },
    children: [image({ src: `${server.url}${path.slice(1)}`, width: 32, height: 32 })],
  });

/** Prepares and renders `path`'s tree twice on `renderer`, returning how often the server served it. */
const renderTwice = async (renderer: Renderer, path: string) => {
  for (let render = 0; render < 2; render++) {
    await renderer.render(await renderer.prepare(drawing(path), { width: 64, height: 64 }));
  }

  return requests.get(path);
};

describe("a renderer's download store", () => {
  test("downloads an image URL once across renders", async () => {
    const renderer = new Renderer();

    expect(await renderTwice(renderer, "/default.png")).toBe(1);
    expect(renderer.cacheStats().store).toMatchObject({
      hits: 1,
      misses: 1,
      max: 32 * 1024 * 1024,
      evictions: 0,
    });
    expect(renderer.cacheStats().store?.current).toBeGreaterThan(imageData.byteLength);
  });

  test("downloads every time when cache.store is null", async () => {
    const renderer = new Renderer({ cache: { store: null } });

    expect(await renderTwice(renderer, "/disabled.png")).toBe(2);
    expect(renderer.cacheStats().store).toBeNull();
  });

  test("takes a store from setCacheOptions", async () => {
    const renderer = new Renderer({ cache: { store: null } });
    const store = new MemoryStore({ maxBytes: 1024 * 1024 });

    renderer.setCacheOptions({ store, maxBytes: 64 * 1024 * 1024 });

    expect(await renderTwice(renderer, "/replaced.png")).toBe(1);
    expect(renderer.cacheStats().memory.max).toBe(64 * 1024 * 1024);
    expect(renderer.cacheStats().store).toMatchObject({ hits: 1, max: 1024 * 1024 });
  });
});

describe("fonts with cache none", () => {
  const node = container({
    style: { width: 256, height: 64, fontSize: 32, fontFamily: "Mono" },
    children: [text("0O1lI")],
  });
  const render = (renderer: Renderer, cache?: "none") =>
    renderer.render(node, {
      width: 256,
      height: 64,
      fonts: [{ name: "Mono", key: "mono", data: () => monoData, cache }],
    });

  test("leave the renderer once their render settles", async () => {
    const renderer = new Renderer();

    await render(renderer, "none");

    expect(renderer.cacheStats().budgets.fonts.current).toBe(0);
  });

  test("use a font the renderer holds and leave it there", async () => {
    const renderer = new Renderer();

    await render(renderer);
    const held = renderer.cacheStats().budgets.fonts;
    await render(renderer, "none");
    const after = renderer.cacheStats().budgets.fonts;

    expect(held.current).toBeGreaterThan(0);
    expect(after.current).toBe(held.current);
    expect(after.misses).toBe(held.misses);
  });
});
