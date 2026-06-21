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

  test("applies timeMs to stylesheet animation", async () => {
    const svg = await renderer.renderSvg(
      { type: "container", tagName: "div" },
      {
        width: 200,
        height: 100,
        timeMs: 500,
        stylesheets: [
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
        ],
      },
    );

    expect(svg).toContain("<svg");
  });
});
