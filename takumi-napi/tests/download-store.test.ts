import { afterAll, describe, expect, test } from "bun:test";
import { container, type FetchLike, image, text } from "@takumi-rs/helpers";
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

/**
 * Prepares and renders `path`'s tree twice on `renderer`, fetching with `fetch` when given,
 * returning how often the server served it.
 */
const renderTwice = async (renderer: Renderer, path: string, fetch?: FetchLike) => {
  for (let render = 0; render < 2; render++) {
    const images = fetch ? { fetch } : undefined;

    await renderer.render(await renderer.prepare(drawing(path), { width: 64, height: 64, images }));
  }

  return requests.get(path);
};

describe("the download store", () => {
  test("downloads an image URL once across renders and renderers", async () => {
    expect(await renderTwice(new Renderer(), "/default.png")).toBe(1);
    expect(await renderTwice(new Renderer(), "/default.png")).toBe(1);
  });

  test("skips the store for a caller's own fetch, which may send its credentials", async () => {
    const own: FetchLike = (input, init) => fetch(input, init);

    expect(await renderTwice(new Renderer(), "/own-fetch.png", own)).toBe(2);
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

    expect(renderer.cacheStats().fonts.current).toBe(0);
  });

  test("use a font the renderer holds and leave it there", async () => {
    const renderer = new Renderer();

    await render(renderer);
    const held = renderer.cacheStats().fonts;
    await render(renderer, "none");
    const after = renderer.cacheStats().fonts;

    expect(held.current).toBeGreaterThan(0);
    expect(after.current).toBe(held.current);
    expect(after.misses).toBe(held.misses);
  });
});
