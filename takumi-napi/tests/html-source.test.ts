import { describe, expect, it } from "bun:test";
import { Renderer } from "../src/export";

describe("HTML sources", () => {
  const renderer = new Renderer();

  const firstPixel = async (html: string, css: string[]) => {
    const raw = await renderer.render(html, { width: 4, height: 4, format: "raw", css });

    return Array.from(raw.subarray(0, 4));
  };

  it("applies <style> after the css option", async () => {
    const pixel = await firstPixel(
      "<style>div { background-color: rgb(0, 0, 255) }</style><div></div>",
      ["div { width: 4px; height: 4px; background-color: rgb(255, 0, 0) }"],
    );

    expect(pixel).toEqual([0, 0, 255, 255]);
  });

  it("keeps the deprecated stylesheets option", async () => {
    const measured = await renderer.measure("<div></div>", {
      stylesheets: ["div { width: 20px; height: 10px }"],
    });

    expect([measured.width, measured.height]).toEqual([20, 10]);
  });

  it("parses inline styles and attributes", async () => {
    const measured = await renderer.measure('<div style="width: 30px; height: 12px">&amp;</div>');

    expect([measured.width, measured.height]).toEqual([30, 12]);
  });

  it("renders an HTML animation scene as its tree with the same CSS", async () => {
    const options = { width: 8, height: 8, fps: 1, format: "gif" } as const;
    const css = "div { width: 8px; height: 8px; background: red }";
    const html = await renderer.renderAnimation({
      ...options,
      scenes: [{ node: `<style>${css}</style><div></div>`, durationMs: 1000 }],
    });
    const tree = await renderer.renderAnimation({
      ...options,
      css: [css],
      scenes: [{ node: { type: "container", tagName: "div", children: [] }, durationMs: 1000 }],
    });

    expect(Buffer.compare(html, tree)).toBe(0);
  });
});
