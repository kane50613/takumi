import { describe, expect, test } from "bun:test";
import { createElement } from "react";
import { isNode, resolveDocument, sheetsOf } from "../src/document";

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

  test("renders an element without $$typeof whose type no node tree uses", async () => {
    const element = { type: "p", props: { children: "a" } };

    expect(isNode(element)).toBeFalse();
    expect((await resolveDocument(element)).node).toMatchObject({ tagName: "p", text: "a" });
  });
});

test("sheetsOf puts the option's sheets before the document's", () => {
  expect(sheetsOf("a {}", ["b {}"])).toEqual(["a {}", "b {}"]);
  expect(sheetsOf(["a {}"], [])).toEqual(["a {}"]);
  expect(sheetsOf(undefined, [])).toBeUndefined();
});
