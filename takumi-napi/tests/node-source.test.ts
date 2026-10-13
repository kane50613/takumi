import { describe, expect, test } from "bun:test";
import { readFile } from "node:fs/promises";
import { join } from "node:path";
import { container, image, text } from "@takumi-rs/helpers";
import { type Node, Renderer } from "../index";
import { toNativeSource } from "../src/node-source";

const renderer = new Renderer();

await renderer.registerFont(
  await readFile(join(import.meta.dir, "../../assets/fonts/geist/Geist[wght].woff2")),
);

const options = { width: 240, height: 120, format: "raw" as const };

const card = container({
  style: {
    display: "flex",
    flexDirection: "column",
    width: "100%",
    height: "100%",
    padding: "10px 14px",
    borderRadius: 10.25,
    backgroundColor: "#141b33",
    opacity: 0.7,
    gap: 2,
    fontFamily: undefined,
  },
  children: [
    text("CPU · node-07 😀", { fontSize: 13.5, color: "#8b93a7" }),
    text("42.7 %", { fontSize: 22, fontWeight: 600, letterSpacing: -0.4 }),
    image({ src: "https://example.com/a.png", width: undefined, height: 10 }),
  ],
});

describe("toNativeSource", () => {
  test("passes a plain tree as JSON, with undefined kept as null", () => {
    const source = toNativeSource(card);

    expect(typeof source).toBe("string");
    expect(source).toContain('"fontFamily":null');
  });

  test.each([
    ["bytes", image({ src: new Uint8Array([1, 2, 3]) as unknown as string })],
    ["a lone surrogate", text("cut \uD83D")],
    ["a non-finite number", text("x", { fontSize: Number.NaN })],
    ["a class instance", { ...text("x"), tagName: new Date(0) } as unknown as Node],
    ["a bigint", { ...text("x"), tagName: 1n } as unknown as Node],
    ["a function", { ...text("x"), tagName: () => "p" } as unknown as Node],
  ])("keeps a tree holding %s as the tree", (_, node) => {
    expect(toNativeSource(node)).toBe(node);
  });

  test("keeps a cyclic tree as the tree", () => {
    const node = container({ children: [] });

    node.children?.push(node);
    expect(toNativeSource(node)).toBe(node);
  });
});

describe("a JSON source", () => {
  test("renders the same pixels as the tree", async () => {
    const fromTree = await renderer.render(card, options);
    const fromJson = await renderer.render(toNativeSource(card), options);

    expect(Buffer.compare(fromJson, fromTree)).toBe(0);
  });

  test.each([
    ["an unreadable value", text("x", { width: "wider" })],
    ["a value of the wrong type", text("x", { justifyContent: 123 as unknown as "center" })],
    ["no type", { text: "x" } as unknown as Node],
  ])("fails on %s with the tree's message", async (_, node) => {
    const message = (source: Node | string) =>
      Promise.resolve()
        .then(() => renderer.render(source, options))
        .then(
          () => "rendered",
          (error: Error) => error.message,
        );

    const fromTree = await message(node);

    expect(fromTree).not.toBe("rendered");
    expect(await message(toNativeSource(node))).toBe(fromTree);
  });
});
