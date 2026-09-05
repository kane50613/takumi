import { expect, mock, test } from "bun:test";
import { prepareImages } from "../src/images";

const node = { type: "image", src: "https://example.com/image.png" } as const;

test("rejects an already-cancelled cache reader", async () => {
  const signal = AbortSignal.abort(new Error("cancelled"));
  const fetchCache = new Map([[node.src, Promise.resolve(new ArrayBuffer(1))]]);
  await expect(prepareImages({ node, fetchCache, signal })).rejects.toThrow("cancelled");
});

test.each(["abort", "timeout"])(
  "%s stops one cache reader without cancelling the shared download",
  async (mode) => {
    const download = Promise.withResolvers<Response>();
    const fetch = mock(() => download.promise);
    const fetchCache = new Map<string, Promise<ArrayBuffer>>();
    const first = prepareImages({ node, fetchCache, fetch, timeout: 0 });
    const controller = new AbortController();
    const second = prepareImages({
      node,
      fetchCache,
      signal: controller.signal,
      timeout: mode === "timeout" ? 10 : 0,
    });
    if (mode === "abort") controller.abort(new Error("cancelled"));
    try {
      await expect(
        Promise.race([second, Bun.sleep(200).then(() => "still waiting")]),
      ).rejects.toThrow(mode === "timeout" ? "timed out" : "cancelled");
    } finally {
      download.resolve(new Response(new Uint8Array([1, 2, 3])));
      expect(await first).toHaveLength(1);
    }
    expect(fetch).toHaveBeenCalledTimes(1);
    expect(fetchCache.has(node.src)).toBe(true);
    expect(await prepareImages({ node, fetchCache })).toHaveLength(1);
  },
);
