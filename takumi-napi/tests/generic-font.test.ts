import { describe, expect, test } from "bun:test";
import { container, text } from "@takumi-rs/helpers";
import { Renderer } from "../src/export";

const monoData = await Bun.file("../assets/fonts/geist/GeistMono[wght].woff2").arrayBuffer();

const node = (fontFamily: string) =>
  container({
    style: { width: 256, height: 64, fontFamily, fontSize: 32, backgroundColor: "#fff" },
    children: [text("mono 0O1lI")],
  });

const options = { width: 256, height: 64 } as const;

describe("generic font family", () => {
  test("a font registered with generic: monospace resolves monospace stacks", async () => {
    const renderer = new Renderer();
    await renderer.registerFont({ data: monoData, generic: "monospace" });

    const viaGeneric = await renderer.render(node("monospace"), options);
    const viaName = await renderer.render(node("Geist Mono"), options);
    expect(Buffer.compare(viaGeneric, viaName)).toBe(0);
  });

  test("without a generic claim, monospace stacks fall back in registration order", async () => {
    const renderer = new Renderer();
    const sansData = await Bun.file("../assets/fonts/geist/Geist[wght].woff2").arrayBuffer();
    await renderer.registerFont({ data: sansData });
    await renderer.registerFont({ data: monoData });

    const viaGeneric = await renderer.render(node("monospace"), options);
    const viaSans = await renderer.render(node("Geist"), options);
    const viaMono = await renderer.render(node("Geist Mono"), options);
    expect(Buffer.compare(viaGeneric, viaSans)).toBe(0);
    expect(Buffer.compare(viaGeneric, viaMono)).not.toBe(0);
  });

  test("an unknown generic keyword rejects", async () => {
    const renderer = new Renderer();
    // @ts-expect-error deliberately invalid keyword
    expect(renderer.registerFont({ data: monoData, generic: "gothic" })).rejects.toThrow();
  });
});
