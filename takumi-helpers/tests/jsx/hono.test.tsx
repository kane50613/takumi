/** @jsxImportSource hono/jsx */
import { describe, expect, test } from "bun:test";
import { html } from "hono/html";
import { createContext, useContext } from "hono/jsx";
import { Suspense } from "hono/jsx/streaming";
import { isNode } from "../../src";
import { fromJsx } from "../../src/jsx";

const later = <T,>(value: T) => new Promise<T>((resolve) => setTimeout(() => resolve(value), 1));

const Loaded = async () => <p>{await later("loaded")}</p>;

describe("hono trees", () => {
  test("render components with their styles", async () => {
    const Label = ({ text }: { text: string }) => <p style={{ color: "red" }}>{text}</p>;

    const { node } = await fromJsx(
      <div class="card">
        <Label text="hi" />
      </div>,
    );

    expect(node).toMatchObject({
      type: "container",
      tagName: "div",
      className: "card",
      children: [{ type: "text", text: "hi", style: { color: "red" } }],
    });
  });

  test("read the value a context provider gives", async () => {
    const Theme = createContext("light");
    const Label = () => <p>{useContext(Theme)}</p>;

    const { node } = await fromJsx(
      <div>
        <Theme.Provider value="dark">
          <Label />
        </Theme.Provider>
      </div>,
    );

    expect(node).toMatchObject({ children: [{ type: "text", text: "dark" }] });
  });

  test("draw a Suspense boundary's children, not its fallback", async () => {
    const { node } = await fromJsx(
      <div>
        <Suspense fallback={<p>loading</p>}>
          <Loaded />
        </Suspense>
      </div>,
    );

    expect(node).toMatchObject({
      type: "container",
      children: [{ type: "text", text: "loaded", tagName: "p" }],
    });
  });

  test("parse an html template with its escaped values", async () => {
    const { node } = await fromJsx(
      html`<div>
        <p>${"x<y"}</p>
        <p>${later("awaited")}</p>
      </div>`,
    );

    const texts = "children" in node ? node.children?.filter((child) => child.tagName === "p") : [];

    expect(texts).toMatchObject([
      { type: "text", text: "x<y" },
      { type: "text", text: "awaited" },
    ]);
  });

  test("keep the CSS of a style element in an html template", async () => {
    const { css } = await fromJsx(
      html`<style>
          p {
            color: red;
          }
        </style>
        <p>a</p>`,
    );

    expect(css).toHaveLength(1);
    expect(css[0]).toContain("color: red");
  });
});

test("a hono element is not a node tree, even one whose tag a node tree uses", () => {
  expect(isNode(<image href="a.png" />)).toBeFalse();
  expect(isNode({ type: "image", src: "a.png" })).toBeTrue();
});
