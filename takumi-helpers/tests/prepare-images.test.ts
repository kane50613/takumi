import { describe, expect, mock, test } from "bun:test";
import type { Node } from "../src/types";
import { prepareImages } from "../src/images";

const ok = (url: string) => new Response(new TextEncoder().encode(url).buffer);

const tree = (...srcs: string[]): Node => ({
  type: "container",
  children: srcs.map((src) => ({ type: "image", src })),
});

describe("prepareImages", () => {
  test("fetches remote images and skips provided sources", async () => {
    const fetchMock = mock((url: string) => Promise.resolve(ok(url)));
    const provided = { src: "https://example.com/a.png", data: new ArrayBuffer(0) };

    const images = await prepareImages({
      node: tree("https://example.com/a.png", "https://example.com/b.png"),
      sources: [provided],
      fetch: fetchMock,
    });

    expect(fetchMock).toHaveBeenCalledTimes(1);
    expect(fetchMock.mock.calls[0]?.[0]).toBe("https://example.com/b.png");
    expect(images.map((i) => i.src).sort()).toEqual([
      "https://example.com/a.png",
      "https://example.com/b.png",
    ]);
  });

  test("single-flight cache coalesces concurrent fetches of the same url", async () => {
    const fetchMock = mock((url: string) => Promise.resolve(ok(url)));
    const fetchCache = new Map<string, Promise<ArrayBuffer>>();
    const node = tree("https://example.com/x.png");

    await Promise.all([
      prepareImages({ node, fetchCache, fetch: fetchMock }),
      prepareImages({ node, fetchCache, fetch: fetchMock }),
    ]);

    expect(fetchMock).toHaveBeenCalledTimes(1);
  });

  test("a rejected fetch is evicted so a later call retries", async () => {
    let attempt = 0;
    const fetchMock = mock((url: string) =>
      ++attempt === 1 ? Promise.reject(new Error("boom")) : Promise.resolve(ok(url)),
    );
    const fetchCache = new Map<string, Promise<ArrayBuffer>>();
    const node = tree("https://example.com/y.png");

    expect(prepareImages({ node, fetchCache, fetch: fetchMock })).rejects.toThrow("boom");
    expect(fetchCache.size).toBe(0);

    const images = await prepareImages({ node, fetchCache, fetch: fetchMock });
    expect(images).toHaveLength(1);
    expect(fetchMock).toHaveBeenCalledTimes(2);
  });

  test("extracts urls from backgroundImage, maskImage, and tw", async () => {
    const fetchMock = mock((url: string) => Promise.resolve(ok(url)));
    const node: Node = {
      type: "container",
      style: {
        backgroundImage: "url(https://example.com/bg.png)",
        maskImage: "url('https://example.com/mask.png')",
      },
      children: [{ type: "container", tw: "bg-[url(https://example.com/tw.png)]" }],
    };

    const images = await prepareImages({ node, fetch: fetchMock });

    expect(images.map((i) => i.src).sort()).toEqual([
      "https://example.com/bg.png",
      "https://example.com/mask.png",
      "https://example.com/tw.png",
    ]);
  });

  test("throwOnError false drops failed urls", async () => {
    const fetchMock = mock((url: string) =>
      url.includes("bad") ? Promise.reject(new Error("nope")) : Promise.resolve(ok(url)),
    );

    const images = await prepareImages({
      node: tree("https://example.com/good.png", "https://example.com/bad.png"),
      fetch: fetchMock,
      throwOnError: false,
    });

    expect(images.map((i) => i.src)).toEqual(["https://example.com/good.png"]);
  });
});
