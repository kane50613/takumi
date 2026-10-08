import { expect, test } from "bun:test";
import type { BoxNode, Matrix, PaintNode, PaintTree, TextNode } from "takumi-paint";
import { inspectPaint, nodeAt } from "./inspect-paint";

const IDENTITY: Matrix = [1, 0, 0, 1, 0, 0];

function box(fields: Partial<BoxNode> & Pick<BoxNode, "width" | "height">): BoxNode {
  const transform = fields.transform ?? IDENTITY;

  return {
    type: "box",
    element: undefined,
    transform,
    bounds: { x: transform[4], y: transform[5], width: fields.width, height: fields.height },
    parent: undefined,
    drawables: [],
    contentBox: { x: 0, y: 0, width: fields.width, height: fields.height },
    outline: [],
    effects: undefined,
    overflowClip: undefined,
    children: [],
    ...fields,
  };
}

/**
 * A 200 × 100 red root holding a padded box at (20, 20) without a background, whose
 * paragraph draws "Hi" from x 10 to 30 on a baseline at y 25.
 */
function card(): PaintTree {
  const root = box({
    width: 200,
    height: 100,
    element: { tagName: "div", id: "card", path: [] },
    drawables: [
      {
        type: "fill",
        role: "background",
        shape: { type: "rect", rect: { x: 0, y: 0, width: 200, height: 100 } },
        paint: { type: "color", color: [255, 0, 0, 255] },
      },
    ],
  });
  const padded = box({
    width: 100,
    height: 40,
    transform: [1, 0, 0, 1, 20, 20],
    element: { tagName: "p", className: "lead", path: [0] },
    contentBox: { x: 10, y: 5, width: 80, height: 30 },
    parent: root,
  });
  const text: TextNode = {
    type: "text",
    element: padded.element,
    transform: padded.transform,
    width: 100,
    height: 40,
    bounds: padded.bounds,
    parent: padded,
    drawables: [],
    textAlign: "left",
    runs: [
      {
        text: "Hi",
        x: 10,
        y: 25,
        width: 20,
        line: 0,
        ascent: 10,
        descent: 4,
        font: {
          family: "Geist",
          weight: 400,
          style: "normal",
          stretch: 1,
          variationSettings: [],
          faceIndex: 0,
          data: () => new Uint8Array(),
        },
        fontSize: 16,
        lineHeight: 20,
        letterSpacing: 0,
        glyphs: [],
        outline: () => "",
      },
    ],
  };
  const nodes: PaintNode[] = [root, padded, text];

  Object.assign(root, { children: [padded] });
  Object.assign(padded, { children: [text] });

  return {
    width: 200,
    height: 100,
    fonts: [],
    root,
    nodes,
    steps: [
      { type: "draw", node: root, drawables: root.drawables },
      { type: "draw", node: text, drawables: [{ type: "group", opacity: 1, drawables: [] }] },
    ],
  };
}

test("frames each node on the canvas and words what it draws", () => {
  const { nodes } = inspectPaint(card());

  expect(nodes.map(({ name, depth, parent }) => ({ name, depth, parent }))).toEqual([
    { name: "div#card", depth: 0, parent: undefined },
    { name: "p", depth: 1, parent: 0 },
    { name: '"Hi"', depth: 2, parent: 1 },
  ]);
  expect(nodes[1]?.contentQuad?.[0]).toEqual({ x: 30, y: 25 });
  expect(nodes[2]?.runQuads?.[0]?.[0]).toEqual({ x: 30, y: 35 });
  expect(nodes[0]?.sections.find((section) => section.title === "Draws")?.rows).toEqual([
    { label: "background", value: "#FF0000 · rect", swatch: "rgba(255, 0, 0, 1)" },
  ]);
});

test("keeps a node's key across renders of the same markup", () => {
  expect(inspectPaint(card()).nodes.map((node) => node.key)).toEqual(
    inspectPaint(card()).nodes.map((node) => node.key),
  );
  expect(new Set(inspectPaint(card()).nodes.map((node) => node.key)).size).toBe(3);
});

test("picks the text under the pointer, then the box around it, then the root", () => {
  const inspection = inspectPaint(card());

  expect(nodeAt(inspection, { x: 40, y: 40 })).toBe(2);
  expect(nodeAt(inspection, { x: 110, y: 30 })).toBe(1);
  expect(nodeAt(inspection, { x: 5, y: 5 })).toBe(0);
  expect(nodeAt(inspection, { x: 250, y: 5 })).toBeUndefined();
});
