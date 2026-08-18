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

  // A template literal leaves whitespace around the markup, which arrives as
  // text siblings. An inline root keeps their line box and pushes the content
  // down the page: kane50613/takumi#1283.
  test("keeps a newline-wrapped element a single root", () => {
    const markup = `<div style="width:100%;height:100%"></div>`;

    expect(fromHtml(`\n  ${markup}\n`).node).toEqual(fromHtml(markup).node);
  });

  test("leaves surrogate code point references untouched", () => {
    const { node } = fromHtml("<div>&#xd800;&#57343;</div>");

    expect((node as TextNode).text).toBe("&#xd800;&#57343;");
  });

  // A data URL carries its own `;`, and it is the usual way markup embeds an
  // image without a fetch. Splitting the attribute on every `;` truncated the
  // value and dropped `base64,...` as an unparsable declaration.
  test("keeps a data URL intact in an inline style", () => {
    const { node } = fromHtml(
      `<div style="background-image:url(data:image/png;base64,iVBORw0KGgo=);color:red">x</div>`,
    );

    expect(node.style).toEqual({
      backgroundImage: "url(data:image/png;base64,iVBORw0KGgo=)",
      color: "red",
    });
  });

  test("keeps a quoted semicolon inside a declaration value", () => {
    const { node } = fromHtml(`<div style="font-family:'Foo; Bar', sans-serif;color:red">x</div>`);

    expect(node.style).toEqual({ fontFamily: "'Foo; Bar', sans-serif", color: "red" });
  });

  test("still separates ordinary declarations", () => {
    const { node } = fromHtml(`<div style="color:red;font-size:12px;">x</div>`);

    expect(node.style).toEqual({ color: "red", fontSize: "12px" });
  });

  // A backslash escapes the next character in CSS outside a string as well as
  // inside one, so an escaped `;` is part of the value rather than a boundary.
  test("keeps an escaped semicolon outside quotes", () => {
    const backslash = String.fromCharCode(92);
    const { node } = fromHtml(`<div style="--value:foo${backslash};bar;color:red">x</div>`);

    expect(node.style).toEqual({ "--value": `foo${backslash};bar`, color: "red" });
  });

  test("tolerates an unclosed url() in an inline style", () => {
    const { node } = fromHtml(`<div style="color:red;background-image:url(">x</div>`);

    expect(node.style).toEqual({ color: "red", backgroundImage: "url(" });
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
