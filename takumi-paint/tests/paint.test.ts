import { describe, expect, it } from "bun:test";
import { container, image, text } from "@takumi-rs/helpers";
import { Painter, type PaintTree } from "takumi-paint";

const painter = new Painter();

function texts(tree: PaintTree) {
  return [...tree.textRuns()].map(({ run }) => run);
}

describe("Painter.paint", () => {
  it("records a box's used decorations and its text runs", async () => {
    const tree = await painter.paint(
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
    const card = tree.find("card");
    expect([card?.x, card?.y, card?.transform]).toEqual([0, 0, undefined]);
    expect(card?.background?.color).toEqual([247, 243, 236, 255]);
    expect(card?.border?.widths).toEqual([4, 4, 4, 4]);
    expect(card?.border?.radii[0]).toEqual([12, 12]);
    expect(card?.shadows).toBeUndefined();

    const [run] = texts(tree);
    expect(run?.text).toBe("Hello paint");
    expect(run?.color).toEqual([0, 0, 255, 255]);
    expect(run?.fontSize).toBe(32);
    expect(run?.font.family).toBe("Geist");
  });

  it("places a transformed box by its origin and keeps the matrix", async () => {
    const tree = await painter.paint(
      `<div id="box" style="width: 100px; height: 50px; transform: rotate(90deg)"></div>`,
      { width: 200, height: 200 },
    );

    const box = tree.find("box");
    expect(box?.transform).toBeDefined();
    expect([box?.x, box?.y]).toEqual([box?.transform?.[4], box?.transform?.[5]]);
  });

  it("scales CSS lengths by the device pixel ratio", async () => {
    const tree = await painter.paint(
      `<style>.big { font-size: 20px }</style><div class="big" style="width: 100px">hi</div>`,
      { width: 200, height: 100, devicePixelRatio: 2 },
    );

    expect([tree.width, tree.height]).toEqual([200, 100]);
    const [run] = texts(tree);
    expect(run?.fontSize).toBe(40);
  });

  it("sizes an SVG image from its root element", async () => {
    const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="2in" viewBox="0 0 4 1"><rect width="4" height="1"/></svg>`;
    const tree = await painter.paint(container({ id: "wrap", children: [image({ src: svg })] }), {
      width: 400,
      height: 200,
    });

    const [picture] = tree.find("wrap")?.children ?? [];
    expect([picture?.width, picture?.height]).toEqual([192, 48]);
    expect(picture?.image?.src).toBe(svg);
  });

  it("keeps a nested span's own color as its own run", async () => {
    const tree = await painter.paint(
      `<style>b { color: rgb(255, 0, 0); font-weight: 700 }</style><p>hello <b>world</b></p>`,
      { width: 400 },
    );

    const runs = texts(tree).map((run) => [run.text, run.color[0], run.font.weight]);
    expect(runs).toEqual([
      ["hello ", 0, 400],
      ["world", 255, 700],
    ]);
  });
});
