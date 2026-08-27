import { afterAll, describe, expect, test } from "bun:test";
import { readFile } from "node:fs/promises";
import { join } from "node:path";
import { container, text } from "@takumi-rs/helpers";
import { Renderer } from "../bundlers/node";

const fontsRoot = join(import.meta.dir, "../../assets/fonts");
const manropeFont = await readFile(join(fontsRoot, "manrope/manrope-latin-wght-normal.woff2"));

const renderer = new Renderer();

afterAll(() => {
  renderer.free();
});

await renderer.registerFont(manropeFont);

const node = container({
  children: [text("Vector")],
  style: {
    justifyContent: "center",
    alignItems: "center",
    backgroundColor: "white",
    width: "100%",
    height: "100%",
  },
});

describe("renderSvg", () => {
  test("returns an SVG document string", async () => {
    const svg = await renderer.renderSvg(node, { width: 200, height: 100 });

    expect(typeof svg).toBe("string");
    expect(svg).toContain("<svg");
    expect(svg).toContain("</svg>");
    expect(svg).toContain('width="200"');
    expect(svg).toContain('height="100"');
  });

  test("auto-calculates dimensions without options", async () => {
    const svg = await renderer.renderSvg(node);

    expect(svg).toMatch(/^<svg/);
    expect(svg).toContain("</svg>");
  });

  test("interpolates a stylesheet animation at timeMs", async () => {
    const sheets = [
      `
        div {
          width: 100px;
          height: 100px;
          background: red;
          animation-name: grow;
          animation-duration: 1000ms;
          animation-timing-function: linear;
          animation-fill-mode: both;
        }

        @keyframes grow {
          from { width: 100px; }
          to { width: 200px; }
        }
      `,
    ];

    const at = (timeMs: number) =>
      renderer.renderSvg(
        { type: "container", tagName: "div" },
        { width: 220, height: 120, timeMs, css: sheets },
      );

    const [start, mid, end] = await Promise.all([at(0), at(500), at(1000)]);

    expect(start).toContain('width="100"');
    expect(mid).toContain('width="150"');
    expect(end).toContain('width="200"');
    expect(mid).not.toBe(start);
  });
});
