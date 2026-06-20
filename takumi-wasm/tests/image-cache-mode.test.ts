import { afterAll, expect, test } from "bun:test";
import { readFile } from "node:fs/promises";
import { join } from "node:path";
import { container, image } from "@takumi-rs/helpers";
import { Renderer } from "../bundlers/node";

const src = "asset://fuma.jpg";
const data = new Uint8Array(await readFile(join(import.meta.dir, "../../assets/images/fuma.jpg")));

const node = container({
  children: [image({ src, width: 256, height: 256 })],
  style: {
    width: "100%",
    height: "100%",
    backgroundColor: "white",
    justifyContent: "center",
    alignItems: "center",
  },
});

const renderer = new Renderer();

afterAll(() => {
  renderer.free();
});

test("immutable cache produces byte-identical output to auto", async () => {
  const options = { width: 512, height: 512, format: "png" as const };

  const auto = await renderer.render(node, { ...options, images: [{ src, data, cache: "auto" }] });
  const immutableFirst = await renderer.render(node, {
    ...options,
    images: [{ src, data, cache: "immutable" }],
  });
  // Second immutable call drops the bytes; the pinned decode satisfies `src`.
  const immutableSecond = await renderer.render(node, {
    ...options,
    images: [{ src, data, cache: "immutable" }],
  });

  expect(Buffer.compare(Buffer.from(auto), Buffer.from(immutableFirst))).toBe(0);
  expect(Buffer.compare(Buffer.from(auto), Buffer.from(immutableSecond))).toBe(0);
});
