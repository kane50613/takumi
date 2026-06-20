import { expect, test } from "bun:test";
import { container, image } from "@takumi-rs/helpers";
import { Renderer, type RenderOptions } from "../src/export";

const src = "asset://fuma.jpg";
const data = await Bun.file("../assets/images/fuma.jpg").arrayBuffer();

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

const options: RenderOptions = { width: 512, height: 512, format: "png" };

test("immutable cache produces byte-identical output to auto", async () => {
  const renderer = new Renderer();

  const auto = await renderer.render(node, {
    ...options,
    images: [{ src, data, cache: "auto" }],
  });

  // First immutable render sends the bytes; second relies on the pin store.
  const immutableFirst = await renderer.render(node, {
    ...options,
    images: [{ src, data, cache: "immutable" }],
  });
  const immutableSecond = await renderer.render(node, {
    ...options,
    images: [{ src, data, cache: "immutable" }],
  });

  expect(Buffer.compare(auto, immutableFirst)).toBe(0);
  expect(Buffer.compare(auto, immutableSecond)).toBe(0);
});

test("immutable src renders without re-sending bytes after first call", async () => {
  const renderer = new Renderer();

  await renderer.render(node, {
    ...options,
    images: [{ src, data, cache: "immutable" }],
  });

  // No `images` provided: the pinned decode must satisfy `src`.
  const result = await renderer.render(node, options);

  expect(result).toBeInstanceOf(Buffer);
  expect(result.length).toBeGreaterThan(0);
});
