import { describe, expect, it } from "bun:test";
import { container, text } from "@takumi-rs/helpers";
import { type PaintNode, type PaintTextRun, PaintTreeRenderer } from "takumi-paint";

const renderer = new PaintTreeRenderer();

function runs(node: PaintNode): PaintTextRun[] {
  return [...(node.runs ?? []), ...(node.children ?? []).flatMap(runs)];
}

function find(node: PaintNode, id: string): PaintNode | undefined {
  if (node.source?.id === id) return node;
  for (const child of node.children ?? []) {
    const found = find(child, id);
    if (found) return found;
  }
  return undefined;
}

describe("PaintTreeRenderer.render", () => {
  it("records a box's used decorations and its text runs", async () => {
    const tree = await renderer.render(
      container({
        id: "card",
        style: {
          display: "flex",
          width: 300,
          padding: 20,
          backgroundColor: "#F7F3EC",
          border: "4px solid #B3261E",
          borderRadius: 12,
        },
        children: [text("Hello paint", { fontSize: 32, color: "rgb(0, 0, 255)" })],
      }),
      { width: 600, height: 300 },
    );

    expect([tree.width, tree.height]).toEqual([600, 300]);
    const card = find(tree.root, "card");
    expect(card?.boxDecoration?.background.color).toEqual([247, 243, 236, 255]);
    expect(card?.boxDecoration?.border.widths).toEqual([4, 4, 4, 4]);
    expect(card?.boxDecoration?.border.radii[0]).toEqual([12, 12]);

    const [run] = runs(tree.root);
    expect(run?.text).toBe("Hello paint");
    expect(run?.color).toEqual([0, 0, 255, 255]);
    expect(run?.fontSize).toBe(32);
    expect(tree.fonts[run!.fontIndex]?.family).toBe("Geist");
  });

  it("scales CSS lengths by the device pixel ratio", async () => {
    const tree = await renderer.render(
      `<style>.big { font-size: 20px }</style><div class="big" style="width: 100px">hi</div>`,
      { width: 200, height: 100, devicePixelRatio: 2 },
    );

    expect([tree.width, tree.height]).toEqual([200, 100]);
    const [run] = runs(tree.root);
    expect(run?.fontSize).toBe(40);
  });

  it("keeps a nested span's own color as its own run", async () => {
    const tree = await renderer.render(
      `<style>b { color: rgb(255, 0, 0); font-weight: 700 }</style><p>hello <b>world</b></p>`,
      { width: 400 },
    );

    const texts = runs(tree.root).map((run) => [run.text, run.color[0]]);
    expect(texts).toEqual([
      ["hello ", 0],
      ["world", 255],
    ]);
  });
});
