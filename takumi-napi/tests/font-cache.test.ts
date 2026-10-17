import { describe, expect, test } from "bun:test";
import { container, text } from "@takumi-rs/helpers";
import { Renderer } from "../src/export";

const monoData = await Bun.file("../assets/fonts/geist/GeistMono[wght].woff2").arrayBuffer();
const sansData = await Bun.file("../assets/fonts/geist/Geist[wght].woff2").arrayBuffer();

const node = (fontFamily: string) =>
  container({
    style: { width: 256, height: 64, fontFamily, fontSize: 32, backgroundColor: "#fff" },
    children: [text("mono 0O1lI")],
  });

const options = { width: 256, height: 64 } as const;

/** A font named `name` that counts its loads. */
const counted = (name: string, data: ArrayBuffer) => {
  const font = {
    name,
    loads: 0,
    data: () => {
      font.loads++;
      return data;
    },
  };

  return font;
};

describe("fontCacheMaxBytes", () => {
  test("releases the font renders used least recently, and loads it again when a render lists it", async () => {
    const renderer = new Renderer({ fontCacheMaxBytes: 1 });
    const mono = counted("Mono", monoData);
    const sans = counted("Sans", sansData);

    const first = await renderer.render(node("Mono"), { ...options, fonts: [mono] });
    await renderer.render(node("Sans"), { ...options, fonts: [sans] });
    const again = await renderer.render(node("Mono"), { ...options, fonts: [mono] });

    expect(mono.loads).toBe(2);
    expect(Buffer.compare(again, first)).toBe(0);
  });

  test("keeps fonts from registerFont", async () => {
    const renderer = new Renderer({ fontCacheMaxBytes: 1 });

    await renderer.registerFont(monoData);
    const registered = await renderer.render(node("Geist Mono"), options);

    await renderer.render(node("Sans"), { ...options, fonts: [counted("Sans", sansData)] });

    expect(Buffer.compare(await renderer.render(node("Geist Mono"), options), registered)).toBe(0);
  });

  test("keeps every font without a budget", async () => {
    const renderer = new Renderer();
    const mono = counted("Mono", monoData);

    await renderer.render(node("Mono"), { ...options, fonts: [mono] });
    await renderer.render(node("Sans"), { ...options, fonts: [counted("Sans", sansData)] });
    await renderer.render(node("Mono"), { ...options, fonts: [mono] });

    expect(mono.loads).toBe(1);
  });
});
