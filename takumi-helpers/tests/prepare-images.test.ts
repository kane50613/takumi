import { describe, expect, mock, test } from "bun:test";
import type { Node } from "../src/types";
import { prepareImages } from "../src/images";
import { imageTree } from "./image-tree";

const ok = (url: string) => new Response(new TextEncoder().encode(url).buffer);

describe("prepareImages", () => {
  test("fetches remote images and skips provided sources", async () => {
    const fetchMock = mock((url: string) => Promise.resolve(ok(url)));
    const provided = { src: "https://example.com/a.png", data: new ArrayBuffer(0) };

    const images = await prepareImages({
      node: imageTree("https://example.com/a.png", "https://example.com/b.png"),
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
    const node = imageTree("https://example.com/x.png");

    await Promise.all([
      prepareImages({ node, fetchCache, fetch: fetchMock }),
      prepareImages({ node, fetchCache, fetch: fetchMock }),
    ]);

    expect(fetchMock).toHaveBeenCalledTimes(1);
  });

  test("keeps emoji images across calls without a fetchCache", async () => {
    const fetchMock = mock((url: string) => Promise.resolve(ok(url)));
    const emoji = "https://cdn.jsdelivr.net/gh/jdecked/twemoji@17.0.2/assets/svg/1f600.svg";
    const other = "https://example.com/x.png";

    for (let call = 0; call < 2; call++) {
      await prepareImages({ node: imageTree(emoji, other), fetch: fetchMock });
    }

    expect(fetchMock.mock.calls.map(([url]) => url).sort()).toEqual([emoji, other, other]);
  });

  test("a caller aborting leaves another waiting on the same emoji its image", async () => {
    let respond: (response: Response) => void = () => {};
    const fetchMock = mock(() => new Promise<Response>((resolve) => (respond = resolve)));
    const node = imageTree(
      "https://cdn.jsdelivr.net/gh/jdecked/twemoji@17.0.2/assets/svg/1f680.svg",
    );
    const controller = new AbortController();
    const aborted = prepareImages({ node, fetch: fetchMock, signal: controller.signal });
    const waiting = prepareImages({ node, fetch: fetchMock });

    controller.abort();
    respond(ok("emoji"));

    await expect(aborted).rejects.toThrow();
    expect(await waiting).toHaveLength(1);
    expect(fetchMock).toHaveBeenCalledTimes(1);
  });

  test("an aborted caller starts no shared emoji fetch", async () => {
    const fetchMock = mock((url: string) => Promise.resolve(ok(url)));
    const node = imageTree(
      "https://cdn.jsdelivr.net/gh/jdecked/twemoji@17.0.2/assets/svg/1f4a9.svg",
    );

    await expect(
      prepareImages({ node, fetch: fetchMock, signal: AbortSignal.abort() }),
    ).rejects.toThrow();
    expect(fetchMock).not.toHaveBeenCalled();
  });

  test("a failed shared emoji fetch leaves the cache", async () => {
    const fetchMock = mock(() => Promise.resolve(new Response(null, { status: 404 })));
    const node = imageTree(
      "https://cdn.jsdelivr.net/gh/jdecked/twemoji@17.0.2/assets/svg/1f4a5.svg",
    );

    for (let call = 0; call < 2; call++) {
      await expect(prepareImages({ node, fetch: fetchMock })).rejects.toThrow();
    }

    expect(fetchMock).toHaveBeenCalledTimes(2);
  });

  test("a provider URL that resolves elsewhere is not shared", async () => {
    const fetchMock = mock((url: string) => Promise.resolve(ok(url)));
    const node = imageTree(
      "https://cdn.jsdelivr.net/gh/jdecked/twemoji@17.0.2/assets/svg/../../../other/repo/a.svg",
    );

    for (let call = 0; call < 2; call++) {
      await prepareImages({ node, fetch: fetchMock });
    }

    expect(fetchMock).toHaveBeenCalledTimes(2);
  });

  test("a rejected fetch is evicted so a later call retries", async () => {
    let attempt = 0;
    const fetchMock = mock((url: string) =>
      ++attempt === 1 ? Promise.reject(new Error("boom")) : Promise.resolve(ok(url)),
    );
    const fetchCache = new Map<string, Promise<ArrayBuffer>>();
    const node = imageTree("https://example.com/y.png");

    await expect(prepareImages({ node, fetchCache, fetch: fetchMock })).rejects.toThrow("boom");
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
      node: imageTree("https://example.com/good.png", "https://example.com/bad.png"),
      fetch: fetchMock,
      throwOnError: false,
    });

    expect(images.map((i) => i.src)).toEqual(["https://example.com/good.png"]);
  });
});
