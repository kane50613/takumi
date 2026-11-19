import { afterEach, describe, expect, mock, spyOn, test } from "bun:test";
import { Renderer } from "../bundlers/node";

const cjk = await Bun.file(
  "../assets/fonts/noto-sans/NotoSansTC-VariableFont_wght.woff2",
).arrayBuffer();
const options = { width: 128, height: 64 } as const;
const html = `<p style="font-family: Missing, sans-serif">a中</p>`;

afterEach(() => mock.restore());

/** Silences `console.warn`, returning a function that lists the messages it received. */
const printed = () => {
  const warn = spyOn(console, "warn").mockImplementation(() => {});

  return () => warn.mock.calls.map(([message]) => String(message));
};

describe("the warnings of a prepare", () => {
  test("name the characters and families no font provides", async () => {
    const warnings = printed();

    await new Renderer().prepare(html, options);

    expect(warnings()).toEqual([
      expect.stringContaining('takumi: No font draws "中".'),
      expect.stringContaining('takumi: No font provides the family "Missing".'),
    ]);
  });

  test("are none once onMissingGlyphs returns a font for the characters", async () => {
    const renderer = new Renderer();
    const without = await renderer.render(html, options);
    const onMissingGlyphs = mock((_characters: string[]) => [{ name: "Missing", data: cjk }]);
    const warnings = printed();
    const prepared = await renderer.prepare(html, { ...options, onMissingGlyphs });

    expect(onMissingGlyphs.mock.calls).toEqual([[["中"]]]);
    expect(warnings()).toEqual([]);
    expect(await renderer.render(prepared)).not.toEqual(without);
  });
});
