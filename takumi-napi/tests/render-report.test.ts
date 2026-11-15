import { afterAll, describe, expect, mock, test } from "bun:test";
import { container, fontFromUrl, image, type Node, text } from "@takumi-rs/helpers";
import { type FontLoader, Renderer } from "../src/export";

const monoData = await Bun.file("../assets/fonts/geist/GeistMono[wght].woff2").arrayBuffer();
const cjkData = await Bun.file(
  "../assets/fonts/noto-sans/NotoSansTC-VariableFont_wght.woff2",
).arrayBuffer();

const server = Bun.serve({
  port: 0,
  fetch: () => new Response(null, { status: 404 }),
});

afterAll(() => server.stop());

const options = { width: 128, height: 64 } as const;
const cjk: FontLoader = { name: "Noto Sans TC", data: cjkData };
const line = (content: string, fontFamily?: string): Node =>
  container({
    style: { width: 128, height: 64, fontSize: 24, fontFamily, backgroundColor: "#fff" },
    children: [text(content)],
  });
const empty = { missingCharacters: [], unresolvedFamilies: [], failedFonts: [], failedImages: [] };

describe("a prepared render's report", () => {
  test("is empty when the built-in font draws every character", async () => {
    const prepared = await new Renderer().prepare(line("Hello"), options);

    expect(prepared.report).toEqual(empty);
  });

  test("names each character no font draws once", async () => {
    const prepared = await new Renderer().prepare(line("a中 b中文"), options);

    expect(prepared.report.missingCharacters).toEqual(["中", "文"]);
  });

  test("leaves out the characters a listed font draws", async () => {
    const prepared = await new Renderer().prepare(line("a中"), { ...options, fonts: [cjk] });

    expect(prepared.report.missingCharacters).toEqual([]);
  });

  test("leaves out the characters a listed subset draws, and keeps the ones past its range", async () => {
    const subset: FontLoader = { name: "Mono", data: monoData, unicodeRange: [[0x20, 0x7e]] };
    const prepared = await new Renderer().prepare(line("a中", "Mono"), {
      ...options,
      fonts: [subset],
    });

    expect(prepared.report).toEqual({ ...empty, missingCharacters: ["中"] });
  });

  test("names the families no font provides, but no generic keyword", async () => {
    const prepared = await new Renderer().prepare(line("a", "Missing, sans-serif"), {
      ...options,
      fontFamilies: ["Mono", "Absent"],
      fonts: [{ name: "Mono", data: monoData }],
    });

    expect(prepared.report.unresolvedFamilies).toEqual(["Missing", "Absent"]);
  });

  test("names an optional font that failed to load", async () => {
    const url = `${server.url}missing.woff2`;
    const prepared = await new Renderer().prepare(line("a"), {
      ...options,
      fonts: [fontFromUrl(url, { optional: true })],
    });

    expect(prepared.report.failedFonts).toEqual([{ source: url, error: expect.any(Error) }]);
  });

  test("names an @font-face file that failed to load", async () => {
    const url = `${server.url}face.woff2`;
    const prepared = await new Renderer().prepare(line("a", "Face"), {
      ...options,
      css: [`@font-face { font-family: Face; src: url(${url}) }`],
    });

    expect(prepared.report.failedFonts).toEqual([{ source: url, error: expect.any(Error) }]);
  });

  test("names an image left out under throwOnError: false", async () => {
    const src = `${server.url}missing.png`;
    const prepared = await new Renderer().prepare(
      container({ children: [image({ src, width: 16, height: 16 })] }),
      { ...options, images: { throwOnError: false } },
    );

    expect(prepared.report.failedImages).toEqual([{ src, error: expect.any(Error) }]);
  });
});

describe("onMissingGlyphs", () => {
  test("loads the fonts it returns into the render, leaving the report empty", async () => {
    const renderer = new Renderer();
    const onMissingGlyphs = mock((_characters: string[]) => [cjk]);
    const without = await renderer.render(line("a中"), options);
    const prepared = await renderer.prepare(line("a中"), { ...options, onMissingGlyphs });

    expect(onMissingGlyphs.mock.calls).toEqual([[["中"]]]);
    expect(prepared.report).toEqual(empty);
    expect(await renderer.render(prepared)).not.toEqual(without);
    expect(await renderer.render(prepared)).toEqual(
      await renderer.render(line("a中"), { ...options, fonts: [cjk] }),
    );
  });

  test("runs for a render without prepare", async () => {
    const renderer = new Renderer();
    const onMissingGlyphs = mock(() => [cjk]);

    expect(await renderer.render(line("a中"), { ...options, onMissingGlyphs })).toEqual(
      await renderer.render(line("a中"), { ...options, fonts: [cjk] }),
    );
    expect(onMissingGlyphs).toHaveBeenCalledTimes(1);
  });

  test("joins an explicit fontFamilies with the families of the fonts it returns", async () => {
    const renderer = new Renderer();
    const prepared = await renderer.prepare(line("a中"), {
      ...options,
      fontFamilies: ["Mono"],
      fonts: [{ name: "Mono", data: monoData }],
      onMissingGlyphs: () => [cjk],
    });

    expect(prepared.report).toEqual(empty);
  });

  test("is not called when every character is covered, and keeps what stays uncovered", async () => {
    const covered = mock(() => undefined);

    await new Renderer().prepare(line("abc"), { ...options, onMissingGlyphs: covered });
    expect(covered).not.toHaveBeenCalled();

    const prepared = await new Renderer().prepare(line("a中"), {
      ...options,
      onMissingGlyphs: () => [{ name: "Mono", data: monoData }],
    });

    expect(prepared.report.missingCharacters).toEqual(["中"]);
  });
});
