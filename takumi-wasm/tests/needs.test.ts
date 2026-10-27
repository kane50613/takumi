import { describe, expect, test } from "bun:test";
import { Renderer } from "../bundlers/node";

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
