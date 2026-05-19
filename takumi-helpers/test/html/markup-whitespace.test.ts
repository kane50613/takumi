import { describe, expect, it } from "bun:test";
import { fromStaticMarkup } from "../../src/html/markup";

describe("fromStaticMarkup whitespace handling", () => {
  it("drops whitespace-only text nodes between siblings in display:grid container (issue #695)", () => {
    const withNewline = `<pre><code style="display:grid"><span class="line"><span>row 1</span></span>
      <span class="line"><span>row 2</span></span></code></pre>`;
    const withoutNewline = `<pre><code style="display:grid"><span class="line"><span>row 1</span></span><span class="line"><span>row 2</span></span></code></pre>`;

    expect(fromStaticMarkup(withNewline)).toEqual(fromStaticMarkup(withoutNewline));
  });

  it("drops whitespace-only text nodes between siblings in default block container", () => {
    const withNewlines = `<div><p>a</p>\n  <p>b</p></div>`;
    const withoutNewlines = `<div><p>a</p><p>b</p></div>`;

    expect(fromStaticMarkup(withNewlines)).toEqual(fromStaticMarkup(withoutNewlines));
  });

  it("drops whitespace-only text nodes between siblings in display:flex container", () => {
    const a = `<section style="display:flex"><div>a</div>\n<div>b</div></section>`;
    const b = `<section style="display:flex"><div>a</div><div>b</div></section>`;

    expect(fromStaticMarkup(a)).toEqual(fromStaticMarkup(b));
  });

  it("preserves whitespace text between inline siblings (default inline span)", () => {
    const markup = `<span>hello <em>world</em>!</span>`;
    const result = fromStaticMarkup(markup);
    // The whitespace " " before <em> must be kept; the span has mixed children
    // so it becomes a container with text children including the space.
    const span = result.nodes[0] as { children: { text?: string }[] };
    const joined = span.children.map((c) => c.text ?? "").join("");
    expect(joined).toContain("hello ");
    expect(joined).toContain("!");
  });

  it("preserves whitespace-only text inside elements whose siblings are all text (concatenated)", () => {
    const markup = `<p>  hello   world  </p>`;
    const result = fromStaticMarkup(markup);
    expect((result.nodes[0] as { text: string }).text).toBe("  hello   world  ");
  });
});
