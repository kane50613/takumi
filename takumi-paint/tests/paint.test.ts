import { describe, expect, it } from "bun:test";
import { container, image, text } from "@takumi-rs/helpers";
import { type PaintTree, type TextRun, Painter } from "takumi-paint";

const painter = new Painter();

function box(tree: PaintTree, id: string) {
  return [...tree].find((node) => node.type === "box" && node.element?.id === id);
}

function runs(tree: PaintTree): TextRun[] {
  return [...tree].flatMap((node) => (node.type === "text" ? node.runs : []));
}

describe("Painter.paint", () => {
  it("resolves a box's decorations into drawables and its text into runs", async () => {
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
    const card = box(tree, "card");
    expect(card?.drawables.map((drawable) => drawable.role)).toEqual(["background", "border"]);
    expect(card?.drawables[0]).toMatchObject({
      type: "fill",
      paint: { type: "color", color: [247, 243, 236, 255] },
      shape: { type: "rounded-rect", radii: { topLeft: { x: 12, y: 12 } } },
    });

    const [run] = runs(tree);
    expect(run?.text).toBe("Hello paint");
    expect(run?.fontSize).toBe(32);
    expect(run?.font.family).toBe("Geist");
    expect(run?.font.data().byteLength).toBeGreaterThan(0);
    expect(run?.outline()).toStartWith("M");

    const glyphs = [...tree.paintSteps()]
      .flatMap((step) => (step.type === "draw" ? step.drawables : []))
      .find((drawable) => drawable.type === "glyphs");
    expect(glyphs).toMatchObject({ role: "text", paint: { color: [0, 0, 255, 255] } });
    expect(glyphs?.type === "glyphs" ? glyphs.run : undefined).toBe(run);
  });

  it("scales CSS lengths by the device pixel ratio", async () => {
    const tree = await painter.paint(
      `<style>.big { font-size: 20px }</style><div class="big" style="width: 100px">hi</div>`,
      { width: 200, height: 100, devicePixelRatio: 2 },
    );

    expect([tree.width, tree.height]).toEqual([200, 100]);
    const [run] = runs(tree);
    expect(run?.fontSize).toBe(40);
  });

  it("sizes an SVG image from its root element", async () => {
    const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="2in" viewBox="0 0 4 1"><rect width="4" height="1"/></svg>`;
    const tree = await painter.paint(container({ id: "wrap", children: [image({ src: svg })] }), {
      width: 400,
      height: 200,
    });

    const picture = [...tree].find((node) => node.type === "image");
    expect([picture?.width, picture?.height]).toEqual([192, 48]);
    expect(picture?.type === "image" && picture.image.src).toBe(svg);
    expect(picture?.parent?.parent).toBe(box(tree, "wrap"));
  });

  it("names the element a nested span's run comes from", async () => {
    const tree = await painter.paint(
      `<style>b { color: rgb(255, 0, 0); font-weight: 700 }</style><p>hello <b>world</b></p>`,
      { width: 400 },
    );

    expect(runs(tree).map((run) => [run.text, run.element?.tagName, run.font.weight])).toEqual([
      ["hello ", undefined, 400],
      ["world", "b", 700],
    ]);
  });

  it("wraps groups and clips around what they enclose", async () => {
    const tree = await painter.paint(
      `<div style="opacity: 0.5; overflow: hidden; width: 100px; height: 50px"><div style="width: 200px; height: 20px; background: red"></div></div>`,
      { width: 200, height: 100 },
    );

    expect([...tree.paintSteps()].map((step) => step.type)).toEqual([
      "begin-group",
      "begin-clip",
      "draw",
      "end-clip",
      "end-group",
    ]);
  });
});
