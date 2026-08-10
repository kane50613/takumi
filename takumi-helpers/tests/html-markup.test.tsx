import { describe, expect, test } from "bun:test";
import { fromHtml } from "../src/html";
import { fromJsx } from "../src/jsx";
import type { TextNode } from "../src/types";

const STYLE = `<style>.box{background:#2c82c9}</style>`;
const BODY = `<div class="box">x</div>`;

describe("fromHtml", () => {
  test("collects <style> from a fragment", () => {
    expect(fromHtml(`${STYLE}${BODY}`).stylesheets).toEqual([".box{background:#2c82c9}"]);
  });

  test("collects <style> from <head> in a full document", () => {
    const { stylesheets } = fromHtml(`<html><head>${STYLE}</head><body>${BODY}</body></html>`);

    expect(stylesheets).toEqual([".box{background:#2c82c9}"]);
  });

  test("does not render <head> content as nodes", () => {
    const { node } = fromHtml(
      `<html><head><title>t</title>${STYLE}</head><body>${BODY}</body></html>`,
    );

    expect(JSON.stringify(node)).not.toContain('"t"');
  });

  test("decodes character references in text nodes", () => {
    const { node } = fromHtml("<div>a&nbsp;b &deg; &#176; &quot;q&quot;</div>");

    expect((node as TextNode).text).toBe('a\u00a0b ° ° "q"');
  });

  test("decodes character references in nested text nodes", () => {
    const { node } = fromHtml("<div><span>1&lt;2</span><div>&euro;5</div></div>");

    expect(JSON.stringify(node)).toContain("1<2");
    expect(JSON.stringify(node)).toContain("€5");
  });

  test("leaves unknown references untouched", () => {
    const { node } = fromHtml("<div>&notarealentity; &amp;</div>");

    expect((node as TextNode).text).toBe("&notarealentity; &");
  });

  test("does not resolve Object.prototype members as entities", () => {
    const { node } = fromHtml("<div>&constructor; &hasOwnProperty;</div>");

    expect((node as TextNode).text).toBe("&constructor; &hasOwnProperty;");
  });

  test("leaves surrogate code point references untouched", () => {
    const { node } = fromHtml("<div>&#xd800;&#57343;</div>");

    expect((node as TextNode).text).toBe("&#xd800;&#57343;");
  });
});

describe("fromJsx", () => {
  test("collects <style> from <head>", async () => {
    const { stylesheets } = await fromJsx(
      <html>
        <head>
          <style>{".box{background:#2c82c9}"}</style>
        </head>
        <body>
          <div className="box">x</div>
        </body>
      </html>,
    );

    expect(stylesheets).toEqual([".box{background:#2c82c9}"]);
  });
});
