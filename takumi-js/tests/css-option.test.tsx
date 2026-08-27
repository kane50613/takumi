import { expect, spyOn, test } from "bun:test";
import { render } from "../src";

const node = <div style={{ width: 10, height: 10 }} className="box" />;
const sheet = ".box { background: rgb(255, 0, 0); }";

// Runs first: the deprecation warning fires once per process, and every later
// test in this file trips it.
test("stylesheets warns once", async () => {
  const warn = spyOn(console, "warn").mockImplementation(() => {});

  await render(node, { width: 10, height: 10, stylesheets: [sheet], format: "raw" });
  await render(node, { width: 10, height: 10, stylesheets: [sheet], format: "raw" });

  expect(warn.mock.calls).toHaveLength(1);
  expect(warn.mock.calls[0]?.[0]).toContain("`stylesheets` option is deprecated");
  warn.mockRestore();
});

test("css string renders like the stylesheets list", async () => {
  const fromCss = await render(node, { width: 10, height: 10, css: sheet, format: "raw" });
  const fromList = await render(node, {
    width: 10,
    height: 10,
    stylesheets: [sheet],
    format: "raw",
  });

  expect(fromCss).toEqual(fromList);
  expect(Array.from(fromCss.slice(0, 4))).toEqual([255, 0, 0, 255]);
});

test("css accepts a list", async () => {
  const fromCss = await render(node, { width: 10, height: 10, css: [sheet], format: "raw" });
  const fromList = await render(node, {
    width: 10,
    height: 10,
    stylesheets: [sheet],
    format: "raw",
  });

  expect(fromCss).toEqual(fromList);
});

test("css next to stylesheets throws", () => {
  expect(render(node, { width: 10, height: 10, css: sheet, stylesheets: [sheet] })).rejects.toThrow(
    "pass either `css` or `stylesheets`, not both",
  );
});
