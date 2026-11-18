import { describe, expect, mock, test } from "bun:test";
import { defaultStore, MemoryStore } from "../src/fetch/store";
import { fetchImages } from "../src/fetch/images";
import { fontFromUrl } from "../src/fonts";

const ok = (body: string) => new Response(new TextEncoder().encode(body));
const text = (data: ArrayBuffer) => new TextDecoder().decode(data);

describe("MemoryStore", () => {
  test("drops the downloads used least recently past its size", () => {
    const store = new MemoryStore(300);
    const download = (url: string) => ({ bytes: new ArrayBuffer(100), url });

    store.set("a", download("a"));
    store.set("b", download("b"));
    store.get("a");
    store.set("c", download("c"));

    expect(store.get("a")).toBeDefined();
    expect(store.get("b")).toBeUndefined();
    expect(store.get("c")).toBeDefined();
  });

  test("keeps nothing at a size of 0", () => {
    const store = new MemoryStore(0);

    store.set("a", { bytes: new ArrayBuffer(1), url: "a" });

    expect(store.get("a")).toBeUndefined();
  });
});

describe("download store", () => {
  const url = "https://example.com/a.png";
  const urls = [url];

  test("serves later requests from the store", async () => {
    const fetch = mock((requested: string) => Promise.resolve(ok(requested)));
    const store = new MemoryStore();

    for (let call = 0; call < 3; call++) {
      await fetchImages({ urls, fetch, store });
    }

    expect(fetch).toHaveBeenCalledTimes(1);
  });

  test("a null store keeps nothing, emoji included", async () => {
    const emoji = "https://cdn.jsdelivr.net/gh/jdecked/twemoji@17.0.2/assets/svg/1f9ea.svg";
    const fetch = mock((requested: string) => Promise.resolve(ok(requested)));

    for (let call = 0; call < 2; call++) {
      await fetchImages({ urls: [url, emoji], fetch, store: null });
    }

    expect(fetch).toHaveBeenCalledTimes(4);
  });

  test("a failed download is not kept, and a later request fetches again", async () => {
    let attempt = 0;
    const fetch = mock((requested: string) =>
      Promise.resolve(++attempt === 1 ? new Response(null, { status: 404 }) : ok(requested)),
    );
    const store = new MemoryStore();

    await expect(fetchImages({ urls, fetch, store })).rejects.toThrow("HTTP 404");
    expect(store.get(url)).toBeUndefined();
    expect(await fetchImages({ urls, fetch, store })).toHaveLength(1);
    expect(fetch).toHaveBeenCalledTimes(2);
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
      fetchImages({ urls: [entry], fetch, store, allowUrl });

    await load(() => true);
    expect(store.get(entry)?.url).toBe(destination);

    await expect(load(entryOnly)).rejects.toThrow("URL blocked by allowUrl policy");
    expect(fetch).toHaveBeenCalledTimes(3);

    expect((await load(() => true)).map((image) => text(image.data))).toEqual(["redirected"]);
    expect(fetch).toHaveBeenCalledTimes(3);
  });

  test("fontFromUrl keeps the font in the store of its fetch", async () => {
    const fetch = mock((requested: string) => Promise.resolve(ok(requested)));
    const font = "https://example.com/font.woff2";

    await fontFromUrl(font, { fetch }).data();
    await fontFromUrl(font, { fetch }).data();

    expect(fetch).toHaveBeenCalledTimes(1);
    expect(defaultStore(fetch).get(font)?.url).toBe(font);
  });
});
