import { describe, expect, mock, test } from "bun:test";
import { Renderer } from "../bundlers/node";

const cjk = await Bun.file(
  "../assets/fonts/noto-sans/NotoSansTC-VariableFont_wght.woff2",
).arrayBuffer();
const options = { width: 128, height: 64 } as const;
const html = `<p style="font-family: Missing, sans-serif">a中</p>`;

describe("a prepared render's report", () => {
  test("names the characters and families no font provides", async () => {
    const prepared = await new Renderer().prepare(html, options);

    expect(prepared.report).toEqual({
      missingCharacters: ["中"],
      unresolvedFamilies: ["Missing"],
      failedFonts: [],
      failedImages: [],
    });
  });

  test("is empty once onMissingGlyphs returns a font for the characters", async () => {
    const renderer = new Renderer();
    const without = await renderer.render(html, options);
    const onMissingGlyphs = mock((_characters: string[]) => [{ name: "Missing", data: cjk }]);
    const prepared = await renderer.prepare(html, { ...options, onMissingGlyphs });

    expect(onMissingGlyphs.mock.calls).toEqual([[["中"]]]);
    expect(prepared.report.missingCharacters).toEqual([]);
    expect(await renderer.render(prepared)).not.toEqual(without);
  });
});
