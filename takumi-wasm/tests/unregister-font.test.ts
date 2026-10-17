import { describe, expect, test } from "bun:test";
import { container, text } from "@takumi-rs/helpers";
import { Renderer } from "../bundlers/node";

const monoData = await Bun.file("../assets/fonts/geist/GeistMono[wght].woff2").arrayBuffer();

const node = (fontFamily: string) =>
  container({
    style: { width: 256, height: 64, fontFamily, fontSize: 32, backgroundColor: "#fff" },
    children: [text("mono 0O1lI")],
  });

const options = { width: 256, height: 64 } as const;

describe("unregisterFont", () => {
  test("a family let go of renders as an unknown one, and registers again from the same bytes", async () => {
    const renderer = new Renderer();

    await renderer.registerFont(monoData);
    const registered = await renderer.render(node("Geist Mono"), options);

    expect(renderer.unregisterFont("geist mono")).toBe(true);
    expect(renderer.unregisterFont("Geist Mono")).toBe(false);

    const removed = await renderer.render(node("Geist Mono"), options);

    expect(Buffer.compare(removed, await renderer.render(node("Unknown"), options))).toBe(0);
    expect(Buffer.compare(removed, registered)).not.toBe(0);

    await renderer.registerFont(monoData);
    expect(Buffer.compare(await renderer.render(node("Geist Mono"), options), registered)).toBe(0);
  });

  test("a render's fonts load again after their family is let go of", async () => {
    const renderer = new Renderer();
    let loads = 0;
    const fonts = [
      {
        name: "Geist Mono",
        data: () => {
          loads++;
          return monoData;
        },
      },
    ];

    const first = await renderer.render(node("Geist Mono"), { ...options, fonts });
    await renderer.render(node("Geist Mono"), { ...options, fonts });
    expect(loads).toBe(1);

    renderer.unregisterFont("Geist Mono");

    const again = await renderer.render(node("Geist Mono"), { ...options, fonts });
    expect(loads).toBe(2);
    expect(Buffer.compare(again, first)).toBe(0);
  });

  test("clearFonts leaves the bundled fallback font", async () => {
    const renderer = new Renderer();
    const fallback = await renderer.render(node("Geist Mono"), options);

    await renderer.registerFont(monoData);
    renderer.clearFonts();

    expect(Buffer.compare(await renderer.render(node("Geist Mono"), options), fallback)).toBe(0);
  });
});
