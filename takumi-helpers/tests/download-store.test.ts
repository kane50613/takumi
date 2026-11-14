import { describe, expect, mock, test } from "bun:test";
import { runInNewContext } from "node:vm";
import {
  type DownloadStore,
  MemoryStore,
  type StoredDownload,
  storeStats,
} from "../src/fetch/store";
import { fontFromUrl } from "../src/fonts";
import { prepareImages } from "../src/images";
import { imageTree } from "./image-tree";

const ok = (body: string) => new Response(new TextEncoder().encode(body));
const text = (data: ArrayBuffer) => new TextDecoder().decode(data);

/** A `fetch` whose responses a test settles one at a time, recording each request's signal. */
const pendingFetch = () => {
  const requests: {
    url: string;
    signal?: AbortSignal | null;
    response: PromiseWithResolvers<Response>;
  }[] = [];
  const fetch = mock((url: string, init?: RequestInit) => {
    const response = Promise.withResolvers<Response>();

    init?.signal?.addEventListener("abort", () => response.reject(init.signal?.reason));
    requests.push({ url, signal: init?.signal, response });

    return response.promise;
  });

  return { fetch, requests };
};

describe("MemoryStore", () => {
  test("drops the downloads used least recently past maxBytes", () => {
    const store = new MemoryStore({ maxBytes: 300 });
    const download = (url: string): StoredDownload => ({ bytes: new ArrayBuffer(100), url });

    store.set("a", download("a"));
    store.set("b", download("b"));
    store.get("a");
    store.set("c", download("c"));

    expect(store.get("a")).toBeDefined();
    expect(store.get("b")).toBeUndefined();
    expect(store.get("c")).toBeDefined();
    expect(store.usage()).toEqual({ current: 208, max: 300, evictions: 1 });
  });

  test("keeps nothing past a maxBytes of 0, and 32 MiB by default", () => {
    const store = new MemoryStore({ maxBytes: 0 });

    store.set("a", { bytes: new ArrayBuffer(1), url: "a" });

    expect(store.get("a")).toBeUndefined();
    expect(new MemoryStore().maxBytes).toBe(32 * 1024 * 1024);
  });
});

describe("download store", () => {
  const url = "https://example.com/a.png";
  const node = imageTree(url);

  test("serves later requests from the store and counts the lookups", async () => {
    const fetch = mock((requested: string) => Promise.resolve(ok(requested)));
    const store = new MemoryStore();

    for (let call = 0; call < 3; call++) {
      await prepareImages({ node, fetch, store });
    }

    expect(fetch).toHaveBeenCalledTimes(1);
    expect(storeStats(store)).toMatchObject({
      hits: 2,
      misses: 1,
      evictions: 0,
      max: store.maxBytes,
    });
  });

  test("reads and writes a custom store, which may answer asynchronously", async () => {
    const kept = new Map<string, StoredDownload>();
    const store: DownloadStore = {
      get: async (key) => kept.get(key),
      set: async (key, download) => {
        kept.set(key, download);
      },
    };
    const fetch = mock((requested: string) => Promise.resolve(ok(requested)));

    await prepareImages({ node, fetch, store });
    const [image] = await prepareImages({ node, fetch, store });

    expect(fetch).toHaveBeenCalledTimes(1);
    expect(kept.get(url)?.url).toBe(url);
    expect(image && text(image.data)).toBe(url);
    expect(storeStats(store)).toEqual({
      current: null,
      max: null,
      hits: 1,
      misses: 1,
      evictions: null,
    });
  });

  test("a store that fails counts as holding nothing", async () => {
    const store: DownloadStore = {
      get: () => Promise.reject(new Error("store down")),
      set: () => {
        throw new Error("store down");
      },
    };
    const fetch = mock((requested: string) => Promise.resolve(ok(requested)));

    expect(await prepareImages({ node, fetch, store })).toHaveLength(1);
    expect(await prepareImages({ node, fetch, store })).toHaveLength(1);
    expect(fetch).toHaveBeenCalledTimes(2);
  });

  test("a null store keeps nothing, emoji included", async () => {
    const emoji = "https://cdn.jsdelivr.net/gh/jdecked/twemoji@17.0.2/assets/svg/1f9ea.svg";
    const fetch = mock((requested: string) => Promise.resolve(ok(requested)));

    for (let call = 0; call < 2; call++) {
      await prepareImages({ node: imageTree(url, emoji), fetch, store: null });
    }

    expect(fetch).toHaveBeenCalledTimes(4);
  });

  test("a failed download is not kept, and a later request fetches again", async () => {
    let attempt = 0;
    const fetch = mock((requested: string) =>
      Promise.resolve(++attempt === 1 ? new Response(null, { status: 404 }) : ok(requested)),
    );
    const store = new MemoryStore();

    await expect(prepareImages({ node, fetch, store })).rejects.toThrow("HTTP 404");
    expect(store.get(url)).toBeUndefined();
    expect(await prepareImages({ node, fetch, store })).toHaveLength(1);
    expect(fetch).toHaveBeenCalledTimes(2);
  });

  test("an abandoned download that fails later leaves the newer one in flight", async () => {
    const { fetch, requests } = pendingFetch();
    const store = new MemoryStore();
    const controller = new AbortController();
    const abandoned = prepareImages({ node, fetch, store, signal: controller.signal });

    controller.abort(new Error("cancelled"));
    await expect(abandoned).rejects.toThrow("cancelled");
    expect(requests[0]?.signal?.aborted).toBe(true);

    const newer = prepareImages({ node, fetch, store });
    const joined = prepareImages({ node, fetch, store });

    expect(requests).toHaveLength(2);
    requests[1]?.response.resolve(ok("newer"));

    for (const images of await Promise.all([newer, joined])) {
      expect(images.map((image) => text(image.data))).toEqual(["newer"]);
    }
  });

  test("one caller's abort or timeout leaves the shared download to the others", async () => {
    const { fetch, requests } = pendingFetch();
    const store = new MemoryStore();
    const controller = new AbortController();
    const aborted = prepareImages({ node, fetch, store, signal: controller.signal });
    const timedOut = prepareImages({ node, fetch, store, timeout: 10 });
    const waiting = prepareImages({ node, fetch, store, timeout: 0 });

    controller.abort(new Error("cancelled"));
    await expect(aborted).rejects.toThrow("cancelled");
    await expect(timedOut).rejects.toThrow("timed out");
    expect(requests[0]?.signal?.aborted).toBe(false);

    requests[0]?.response.resolve(ok("shared"));

    expect((await waiting).map((image) => text(image.data))).toEqual(["shared"]);
    expect(requests).toHaveLength(1);
  });

  test("shares a download only between requests with the same fetch, allowUrl, and maxBytes", async () => {
    const { fetch, requests } = pendingFetch();
    const other = pendingFetch();
    const store = new MemoryStore();
    const allowUrl = () => true;
    const loads = [
      prepareImages({ node, fetch, store, allowUrl }),
      prepareImages({ node, fetch, store, allowUrl }),
      prepareImages({ node, fetch, store, allowUrl: () => true }),
      prepareImages({ node, fetch, store, allowUrl, maxBytes: 1024 }),
      prepareImages({ node, fetch: other.fetch, store, allowUrl }),
    ];

    expect(requests).toHaveLength(3);
    expect(other.requests).toHaveLength(1);

    for (const request of [...requests, ...other.requests]) {
      request.response.resolve(ok("bytes"));
    }

    expect(await Promise.all(loads)).toHaveLength(5);
  });

  test("an abandoned download that finishes later leaves the newer one in the store", async () => {
    const bodies: ReadableStreamDefaultController<Uint8Array>[] = [];
    const fetch = mock(() => {
      const body = new ReadableStream<Uint8Array>({
        start: (controller) => {
          bodies.push(controller);
        },
      });

      return Promise.resolve(new Response(body));
    });
    const finish = (index: number, body: string) => {
      bodies[index]?.enqueue(new TextEncoder().encode(body));
      bodies[index]?.close();
    };
    const store = new MemoryStore();
    const controller = new AbortController();
    const abandoned = prepareImages({ node, fetch, store, signal: controller.signal });

    await Bun.sleep(0);
    controller.abort(new Error("cancelled"));
    await expect(abandoned).rejects.toThrow("cancelled");

    const newer = prepareImages({ node, fetch, store });

    await Bun.sleep(0);
    finish(1, "newer");
    await newer;
    finish(0, "older");
    await Bun.sleep(0);

    const stored = store.get(url);

    expect(fetch).toHaveBeenCalledTimes(2);
    expect(stored && text(stored.bytes)).toBe("newer");
  });

  test("reads a store that answers with a promise from another realm", async () => {
    const kept = new Map<string, StoredDownload>();
    const store: DownloadStore = {
      get: (key) => runInNewContext("Promise.resolve(download)", { download: kept.get(key) }),
      set: (key, download) => {
        kept.set(key, download);

        return runInNewContext("Promise.reject(new Error('store down'))");
      },
    };
    const fetch = mock((requested: string) => Promise.resolve(ok(requested)));

    await prepareImages({ node, fetch, store });
    const [image] = await prepareImages({ node, fetch, store });

    expect(fetch).toHaveBeenCalledTimes(1);
    expect(image && text(image.data)).toBe(url);
  });

  test("a stored redirect serves only callers whose allowUrl accepts where it ended", async () => {
    const entry = "https://allowed.example.com/image.png";
    const destination = "https://other.example.com/image.png";
    const fetch = mock((requested: string) =>
      Promise.resolve(
        requested === entry
          ? new Response(null, { status: 302, headers: { location: destination } })
          : ok("redirected"),
      ),
    );
    const store = new MemoryStore();
    const entryOnly = (requested: string) => requested === entry;
    const load = (allowUrl: (url: string) => boolean) =>
      prepareImages({ node: imageTree(entry), fetch, store, allowUrl });

    await load(() => true);
    expect(store.get(entry)?.url).toBe(destination);

    await expect(load(entryOnly)).rejects.toThrow("URL blocked by allowUrl policy");
    expect(fetch).toHaveBeenCalledTimes(3);

    expect((await load(() => true)).map((image) => text(image.data))).toEqual(["redirected"]);
    expect(fetch).toHaveBeenCalledTimes(3);
    expect(storeStats(store)).toMatchObject({ hits: 1, misses: 2 });
  });

  test("fontFromUrl keeps the font in the store it is given", async () => {
    const fetch = mock((requested: string) => Promise.resolve(ok(requested)));
    const store = new MemoryStore();
    const font = "https://example.com/font.woff2";

    await fontFromUrl(font, { fetch, store }).data();
    await fontFromUrl(font, { fetch, store }).data();

    expect(fetch).toHaveBeenCalledTimes(1);
    expect(store.get(font)?.url).toBe(font);
  });
});
