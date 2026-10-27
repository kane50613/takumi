import { afterAll, describe, expect, test } from "bun:test";
import { container, text } from "@takumi-rs/helpers";
import { toNativeSource } from "@takumi-rs/helpers/renderer";
import { type Node, Renderer } from "../bundlers/node";

const renderer = new Renderer();
const inner = renderer["inner"];
const options = { width: 64, height: 32, format: "raw" } as const;

afterAll(() => renderer.free());

const card = container({
  style: { width: 64, height: 32, backgroundColor: "rgb(30, 64, 175)", padding: 4 },
  children: [text("Hi", { color: "white", fontSize: 12 })],
});

describe("a JSON source", () => {
  test("renders the same pixels as the tree", () => {
    const source = toNativeSource(card);

    expect(source).toHaveProperty("json");
    expect(Buffer.compare(inner.render(source, options), inner.render(card, options))).toBe(0);
  });

  test.each([
    ["an unreadable value", text("x", { width: "wider" })],
    ["a value of the wrong type", text("x", { justifyContent: 123 as unknown as "center" })],
    ["no type", { text: "x" } as unknown as Node],
  ])("fails on %s with the tree's message", (_, node) => {
    const message = (source: Parameters<typeof inner.render>[0]) => {
      try {
        inner.render(source, options);
        return "rendered";
      } catch (error) {
        return error instanceof Error ? error.message : String(error);
      }
    };

    const fromTree = message(node);

    expect(fromTree).not.toBe("rendered");
    expect(message(toNativeSource(node))).toBe(fromTree);
  });
});
