import { describe, expect, it } from "bun:test";
import { container, image, text } from "@takumi-rs/helpers";
import { type PaintDocument, type TextRun, PaintTreeRenderer } from "takumi-paint";

const renderer = new PaintTreeRenderer();

function runs(document: PaintDocument): TextRun[] {
  return [...document].flatMap((node) => (node.type === "text" ? node.runs : []));
}

describe("PaintTreeRenderer.render", () => {
  it("resolves a box's decorations into drawables and its text into runs", async () => {
    const document = await renderer.render(
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

    expect([document.width, document.height]).toEqual([600, 300]);
    const card = document.find("card");
    expect(card?.drawables.map((drawable) => drawable.role)).toEqual(["background", "border"]);
    expect(card?.drawables[0]).toMatchObject({
      type: "fill",
      paint: { type: "color", color: [247, 243, 236, 255] },
      shape: { type: "rounded-rect", radii: { topLeft: { x: 12, y: 12 } } },
    });

    const [run] = runs(document);
    expect(run?.text).toBe("Hello paint");
    expect(run?.fontSize).toBe(32);
    expect(run?.font.family).toBe("Geist");
    expect(run?.font.data().byteLength).toBeGreaterThan(0);
    expect(run?.outline()).toStartWith("M");

    const glyphs = [...document.paintSteps()]
      .flatMap((step) => (step.type === "draw" ? step.drawables : []))
      .find((drawable) => drawable.type === "glyphs");
    expect(glyphs).toMatchObject({ role: "text", paint: { color: [0, 0, 255, 255] } });
    expect(glyphs?.type === "glyphs" ? glyphs.run : undefined).toBe(run);
  });

  it("scales CSS lengths by the device pixel ratio", async () => {
    const document = await renderer.render(
      `<style>.big { font-size: 20px }</style><div class="big" style="width: 100px">hi</div>`,
      { width: 200, height: 100, devicePixelRatio: 2 },
    );

    expect([document.width, document.height]).toEqual([200, 100]);
    const [run] = runs(document);
    expect(run?.fontSize).toBe(40);
  });

  it("sizes an SVG image from its root element", async () => {
    const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="2in" viewBox="0 0 4 1"><rect width="4" height="1"/></svg>`;
    const document = await renderer.render(
      container({ id: "wrap", children: [image({ src: svg })] }),
      { width: 400, height: 200 },
    );

    const picture = [...document].find((node) => node.type === "image");
    expect([picture?.width, picture?.height]).toEqual([192, 48]);
    expect(picture?.type === "image" && picture.image.src).toBe(svg);
    expect(picture?.parent?.parent).toBe(document.find("wrap"));
  });

  it("names the element a nested span's run comes from", async () => {
    const document = await renderer.render(
      `<style>b { color: rgb(255, 0, 0); font-weight: 700 }</style><p>hello <b>world</b></p>`,
      { width: 400 },
    );

    expect(runs(document).map((run) => [run.text, run.element?.tagName, run.font.weight])).toEqual([
      ["hello ", undefined, 400],
      ["world", "b", 700],
    ]);
  });

  it("wraps groups and clips around what they enclose", async () => {
    const document = await renderer.render(
      `<div style="opacity: 0.5; overflow: hidden; width: 100px; height: 50px"><div style="width: 200px; height: 20px; background: red"></div></div>`,
      { width: 200, height: 100 },
    );

    expect([...document.paintSteps()].map((step) => step.type)).toEqual([
      "begin-group",
      "begin-clip",
      "draw",
      "end-clip",
      "end-group",
    ]);
  });
});
