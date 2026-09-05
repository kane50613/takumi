import { expect, test } from "bun:test";
import { createElement, type CSSProperties } from "react";
import { container, image, style, text } from "../src/helpers";
import { fromHtml } from "../src/html";
import { fromJsx } from "../src/jsx";
import type { Declarations, NodeMetadata, StyleRule } from "../src/types";

test("custom properties pass through every declaration entry point", async () => {
  const declarations: Declarations = style({
    "--accent": "red",
    "--step": 2,
    color: "var(--accent)",
  });
  const metadata: NodeMetadata = { style: declarations, preset: declarations };
  const rule: StyleRule = { selector: ".card", style: declarations };
  const nodes = [
    container({ children: [], ...metadata }),
    text({ text: "hello", ...metadata }),
    image({ src: "image.png", ...metadata }),
  ];

  for (const node of nodes) {
    expect(node.style).toEqual(rule.style);
    expect(node.preset).toEqual(declarations);
  }
  expect(text("hello", { "--accent": "red" }).style).toEqual({ "--accent": "red" });
  expect(fromHtml('<div style="--accent:red;color:var(--accent)"></div>').node.style).toEqual({
    "--accent": "red",
    color: "var(--accent)",
  });
  const reactStyle: CSSProperties = declarations;
  const jsx = await fromJsx(createElement("div", { style: reactStyle }));
  expect(jsx.node.style).toEqual(declarations);
});
