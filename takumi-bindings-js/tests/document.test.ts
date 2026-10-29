import { describe, expect, test } from "bun:test";
import { createElement } from "react";
import { isNode, resolveDocument } from "../src/document";
import { lazy, sheetsOf } from "../src/wasm-renderer";

describe("resolveDocument", () => {
  test("keeps HTML and node trees as they are", async () => {
    const tree = { type: "text" as const, text: "a" };

    expect(await resolveDocument("<p>a</p>")).toEqual({ node: "<p>a</p>", css: [] });
    expect(await resolveDocument(tree)).toEqual({ node: tree, css: [] });
  });

  test("renders JSX whose element type a node tree also uses", async () => {
    const element = createElement("image", { href: "a.png" });

    expect(isNode(element)).toBeFalse();
    expect((await resolveDocument(element)).node).not.toBe(element);
  });
});

test("sheetsOf puts the option's sheets before the document's", () => {
  expect(sheetsOf("a {}", ["b {}"])).toEqual(["a {}", "b {}"]);
  expect(sheetsOf(["a {}"], [])).toEqual(["a {}"]);
  expect(sheetsOf(undefined, [])).toBeUndefined();
});

test("lazy creates its value once", () => {
  let made = 0;
  const value = lazy(() => ++made);

  expect([value(), value()]).toEqual([1, 1]);
});
