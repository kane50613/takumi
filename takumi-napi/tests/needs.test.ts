import { describe, expect, mock, test } from "bun:test";
import { readFile } from "node:fs/promises";
import { join } from "node:path";
import { Renderer } from "../src/export";

const renderer = new Renderer();

describe("needs", () => {
  test("reads images and characters after the cascade", async () => {
    const html = `<style>
      .bg { background-image: url(https://example.com/bg.png) }
      .hidden { display: none }
      p { text-transform: uppercase }
      p::before { content: "→" }
    </style>
    <div class="bg"></div>
    <div class="hidden"><img src="https://example.com/hidden.png">z</div>
    <img src="https://example.com/img.png">
    <p>ab</p>`;

    const needs = await renderer.needs(html, { width: 100, height: 100 });

    expect(needs.images).toEqual(["https://example.com/bg.png", "https://example.com/img.png"]);
    expect(needs.characters).toContain("→");
    expect(needs.characters).toContain("AB");
    expect(needs.characters).not.toContain("z");
  });

  test("reads a node tree the same way", async () => {
    const needs = await renderer.needs(
      {
        type: "container",
        style: { backgroundImage: "url(https://example.com/a.png)" },
        children: [{ type: "text", text: "hi" }],
      },
      { width: 10, height: 10 },
    );

    expect(needs).toEqual({ images: ["https://example.com/a.png"], characters: "hi" });
  });
});

describe("font subsets", () => {
  test("load for characters only a stylesheet adds", async () => {
    const bytes = await readFile(
      join(import.meta.dir, "../../assets/fonts/geist/Geist[wght].woff2"),
    );
    const latin = {
      name: "Subset",
      key: "latin",
      unicodeRange: [[0x20, 0x7f]] as [number, number][],
      data: mock(() => bytes),
    };
    const cyrillic = {
      name: "Subset",
      key: "cyrillic",
      unicodeRange: [[0x400, 0x4ff]] as [number, number][],
      data: mock(() => bytes),
    };
    const greek = {
      name: "Subset",
      key: "greek",
      unicodeRange: [[0x370, 0x3ff]] as [number, number][],
      data: mock(() => bytes),
    };

    await new Renderer().render("<div>a</div>", {
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
