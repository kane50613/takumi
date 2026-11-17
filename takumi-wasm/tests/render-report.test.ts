import { describe, expect, mock, test } from "bun:test";
import { isTakumiError, Renderer } from "../bundlers/node";

const cjk = await Bun.file(
  "../assets/fonts/noto-sans/NotoSansTC-VariableFont_wght.woff2",
).arrayBuffer();
const options = { width: 128, height: 64 } as const;
const html = `<p style="font-family: Missing, sans-serif">a中</p>`;

describe("a prepared render's report", () => {
  test("names the characters and families no font provides", async () => {
    const prepared = await new Renderer().prepare(html, options);

    expect(prepared.report.warnings.map(({ code }) => code)).toEqual([
      "missing-glyphs",
      "unresolved-family",
    ]);
  });

  test("is empty once onMissingGlyphs returns a font for the characters", async () => {
    const renderer = new Renderer();
    const without = await renderer.render(html, options);
    const onMissingGlyphs = mock((_characters: string[]) => [{ name: "Missing", data: cjk }]);
    const prepared = await renderer.prepare(html, { ...options, onMissingGlyphs });

    expect(onMissingGlyphs.mock.calls).toEqual([[["中"]]]);
    expect(prepared.report.warnings.map(({ code }) => code)).toEqual(["unresolved-family"]);
    expect(await renderer.render(prepared)).not.toEqual(without);
  });
});

describe("strict", () => {
  test("rejects a render with a TakumiError holding its warnings", async () => {
    const error: unknown = await new Renderer()
      .render(html, { ...options, onWarning: () => {}, strict: true })
      .catch((caught: unknown) => caught);

    expect(isTakumiError(error, "ERR_TAKUMI_STRICT") && error.warnings.length).toBe(2);
  });
});
