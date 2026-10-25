import { expect, test } from "bun:test";
import { render } from "../src";

const node = (
  <div style={{ width: 10, height: 10, backgroundColor: "rgb(0, 0, 255)" }}>
    <img src="https://example.com/missing.png" style={{ width: 10, height: 10 }} />
  </div>
);
const raw = { width: 10, height: 10, format: "raw" } as const;
const notFound = async () => new Response(null, { status: 404 });

test("a failed image fetch rejects the render by default", async () => {
  await expect(render(node, { ...raw, images: { fetch: notFound } })).rejects.toThrow("HTTP 404");
});

test("throwOnError: false leaves a failed image out", async () => {
  const image = await render(node, { ...raw, images: { fetch: notFound, throwOnError: false } });

  expect(Array.from(image.slice(0, 4))).toEqual([0, 0, 255, 255]);
});
