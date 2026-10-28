import { describe, expect, it, mock } from "bun:test";
import { readFile } from "node:fs/promises";
import { join } from "node:path";
import { container, image, text } from "@takumi-rs/helpers";
import {
  type BoxNode,
  type PaintTree,
  type SerializedPaintTree,
  type TextRun,
  Painter,
} from "takumi-paint";

const painter = new Painter();

function box(tree: PaintTree, id: string): BoxNode | undefined {
  return tree.nodes.find((node): node is BoxNode => node.type === "box" && node.element?.id === id);
}

function runs(tree: PaintTree): TextRun[] {
  return tree.nodes.flatMap((node) => (node.type === "text" ? node.runs : []));
}

describe("Painter.paint", () => {
  it("points each glyph at where its cluster starts in the run's text", async () => {
    const tree = await painter.paint(text("café ok", { fontSize: 32 }), {
      width: 400,
      height: 100,
    });
    const [run] = runs(tree);

    expect(run?.glyphs.map((glyph) => glyph.cluster)).toEqual([0, 1, 2, 3, 4, 5, 6]);
  });

  it("writes every node, effect, and glyph outline to JSON", async () => {
    const tree = await painter.paint(
      container({
        id: "card",
        style: { display: "flex", width: 300, padding: 20 },
        children: [
          container({
            id: "clipped",
            style: { width: 100, height: 100, clipPath: "circle(40%)" },
            children: [text("clipped")],
          }),
          text("Hello paint", { fontSize: 32 }),
        ],
      }),
      { width: 400, height: 200 },
    );
    const json: SerializedPaintTree = JSON.parse(JSON.stringify(tree));
    const clipped = json.tree.nodes.find((node) => node.element?.id === "clipped");
    const node = json.tree.nodes.findIndex(
      (each) => each.type === "text" && each.runs.some((run) => run.text === "Hello paint"),
    );
    const raw = json.tree.nodes[node];
    const run =
      raw?.type === "text" ? raw.runs.findIndex((each) => each.text === "Hello paint") : -1;

    expect(json.version).toBe(1);
    expect(json.tree.nodes).toHaveLength(tree.nodes.length);
    expect(clipped?.type === "box" ? clipped.effects?.clip : undefined).toMatchObject({
      type: "path",
    });
    expect(json.outlines.map((outlines) => outlines === null)).toEqual(
      json.tree.nodes.map((each) => each.type !== "text"),
    );
    expect(json.outlines[node]?.[run]).toEqual(
      runs(tree)
        .find((each) => each.text === "Hello paint")
        ?.outlines(),
    );
  });

  it("outlines each glyph of a run on its own", async () => {
    const tree = await painter.paint(text("paint", { fontSize: 32 }), { width: 400, height: 100 });
    const [run] = runs(tree);
    const outlines = run?.outlines() ?? [];

    expect(outlines).toHaveLength(5);
    expect(outlines.every((outline) => outline?.startsWith("M"))).toBe(true);

    const spaced = await painter.paint(text("a b", { fontSize: 32 }), { width: 400, height: 100 });

    expect(runs(spaced)[0]?.outlines()[1]).toBe("");
  });

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
    expect(
      card?.drawables.flatMap((drawable) => (drawable.type === "group" ? [] : [drawable.role])),
    ).toEqual(["background", "border"]);
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
    expect(run?.outlines().join("")).toStartWith("M");

    const glyphs = tree.steps
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

    const picture = tree.nodes.find((node) => node.type === "image");
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

    expect(tree.steps.map((step) => step.type)).toEqual([
      "begin-group",
      "begin-clip",
      "draw",
      "end-clip",
      "end-group",
    ]);
  });

  it("lists nodes in document order, whatever order they paint in", async () => {
    const tree = await painter.paint(
      `<div id="a"><p id="p">text</p><div id="behind" style="position: absolute; z-index: -1">behind</div></div>`,
      { width: 200 },
    );

    expect(tree.nodes.map((node) => `${node.type}:${node.element?.id}`)).toEqual([
      "box:a",
      "box:p",
      "text:p",
      "box:behind",
      "text:behind",
    ]);
    expect(tree.nodes[0]).toBe(tree.root);
  });

  it("keeps a translucent span's opacity on its group, not its colors", async () => {
    const tree = await painter.paint(
      `<p>plain <span style="opacity: 0.5; background-color: rgb(255, 0, 0)">faded</span></p>`,
      { width: 200 },
    );
    const text = tree.nodes.find((node) => node.type === "text");
    const group = text?.drawables.find((drawable) => drawable.type === "group");

    expect(group).toMatchObject({ type: "group", opacity: 0.5 });
    expect(group?.type === "group" && group.drawables[0]).toMatchObject({
      role: "inline-background",
      paint: { type: "color", color: [255, 0, 0, 255] },
    });
  });

  it("masks a text-clipped background with the outlines of the text inside it, keeping its blend modes", async () => {
    const tree = await painter.paint(
      `<div id="clip" style="color: transparent; background-color: red; background-image: linear-gradient(blue, blue); background-blend-mode: multiply; background-clip: text"><div>Hi</div></div>`,
      { width: 200 },
    );
    const group = box(tree, "clip")?.drawables.find((drawable) => drawable.type === "group");
    const masked =
      group?.type === "group"
        ? group.drawables.find((drawable) => drawable.type === "masked")
        : undefined;
    const mask = masked?.type === "masked" ? masked.mask : [];

    expect(mask).toHaveLength(2);
    expect(mask.every((drawable) => drawable.type === "fill")).toBe(true);
    expect(masked?.type === "masked" && masked.content).toMatchObject([
      { type: "fill", paint: { type: "color", color: [255, 0, 0, 255] } },
      { type: "fill", blendMode: "multiply" },
    ]);
  });

  it("isolates a background whose layers blend", async () => {
    const tree = await painter.paint(
      `<div id="blend" style="width: 40px; height: 20px; background-color: red; background-image: linear-gradient(blue, blue); background-blend-mode: multiply"></div>`,
      { width: 100 },
    );
    const group = box(tree, "blend")?.drawables.find((drawable) => drawable.type === "group");

    expect(group?.type === "group" && group.opacity).toBe(1);
    expect(group?.type === "group" && group.drawables).toMatchObject([
      { type: "fill", paint: { type: "color", color: [255, 0, 0, 255] } },
      { type: "fill", blendMode: "multiply" },
    ]);
  });
});

describe("font subsets", () => {
  it("load for characters only a stylesheet adds", async () => {
    const bytes = await readFile(
      join(import.meta.dir, "../../assets/fonts/geist/Geist[wght].woff2"),
    );
    const subset = (key: string, range: [number, number]) => ({
      name: "Subset",
      key,
      unicodeRange: [range],
      data: mock(() => bytes),
    });
    const latin = subset("latin", [0x20, 0x7f]);
    const cyrillic = subset("cyrillic", [0x400, 0x4ff]);
    const greek = subset("greek", [0x370, 0x3ff]);

    await new Painter().paint(container({ tagName: "div", children: [text("a")] }), {
      width: 20,
      height: 20,
      css: ["div::before { content: 'Ж' }"],
      fonts: [latin, cyrillic, greek],
    });

    expect(latin.data).toHaveBeenCalledTimes(1);
    expect(cyrillic.data).toHaveBeenCalledTimes(1);
    expect(greek.data).not.toHaveBeenCalled();
  });
});
