// Records where Chromium draws each character of the HTML fixtures, for `takumi/tests/chromium_geometry.rs`.
//
// bun chromium-geometry [fragment...]
//
// Writes `takumi/tests/fixtures-chromium/<fixture>.json`. Fragments select fixtures by name, as
// `FIXTURE` does for the Rust fixtures. Run `bunx playwright-core install chromium` once first.
import { mkdir, readdir, readFile, writeFile } from "node:fs/promises";
import { join, resolve } from "node:path";
import { chromium } from "playwright-core";

const root = resolve(import.meta.dir, "..");
const fixtures = join(root, "takumi/tests/fixtures-html");
const output = join(root, "takumi/tests/fixtures-chromium");
const fragments = process.argv.slice(2);

/** The test fonts registered for each generic family in `takumi/tests/test_utils.rs`. */
const GENERIC_FAMILIES = {
  standard: "Geist",
  sansSerif: "Geist",
  serif: "Geist",
  fixed: "Geist Mono",
  cursive: "Geist",
  fantasy: "Geist",
};

/** The DOM's `Node.TEXT_NODE`, which a snapshot's node types hold. */
const TEXT_NODE = 3;

type Snapshot = {
  strings: string[];
  documents: {
    nodes: { nodeType?: number[]; nodeName?: number[]; parentIndex?: number[] };
    layout: { nodeIndex: number[]; text: number[] };
  }[];
};

/**
 * The text each of the body's text nodes lays out, in document order: its `LayoutText`'s, which
 * `text-transform` has already changed. `null` for a node with no layout object.
 */
function laidOutText({ strings, documents: [document] }: Snapshot) {
  const { nodeType = [], nodeName = [], parentIndex = [] } = document.nodes;
  const { nodeIndex, text } = document.layout;
  const body = nodeName.findIndex((name) => strings[name] === "BODY");
  const inBody = (node: number): boolean =>
    node === body || (node >= 0 && inBody(parentIndex[node] ?? -1));
  const laidOut = new Map<number, string>();

  nodeIndex.forEach((node, layout) => {
    if (text[layout] >= 0) {
      laidOut.set(node, (laidOut.get(node) ?? "") + strings[text[layout]]);
    }
  });

  return nodeType.flatMap((type, node) =>
    type === TEXT_NODE && inBody(node) ? [laidOut.get(node) ?? null] : [],
  );
}

/** The viewport a fixture's `<body style>` sets, as `html_fixtures.rs` reads it. */
function viewport(html: string) {
  const style = /<body style="([^"]*)"/.exec(html)?.[1] ?? "";
  const pixels = (property: string) =>
    Number(new RegExp(`(?:^|;)\\s*${property}\\s*:\\s*(\\d+)px`).exec(style)?.[1]);

  return { width: pixels("width"), height: pixels("height") };
}

const names = (await readdir(fixtures))
  .filter((file) => file.endsWith(".html"))
  .map((file) => file.slice(0, -".html".length))
  .filter((name) => fragments.length === 0 || fragments.some((fragment) => name.includes(fragment)))
  .sort();
const browser = await chromium.launch();

await mkdir(output, { recursive: true });

for (const name of names) {
  const path = join(fixtures, `${name}.html`);
  const size = viewport(await readFile(path, "utf8"));

  if (!size.width || !size.height) {
    console.warn(`${name}: no body viewport, skipped`);
    continue;
  }

  const page = await browser.newPage({ viewport: size, deviceScaleFactor: 1 });
  const cdp = await page.context().newCDPSession(page);

  // takumi reads a fixture's `<style>` but not `shared.css`, so its body text falls back to the
  // test fonts' generic families. Chromium resolves generic families to these names, then to
  // `shared.css`'s faces of those names.
  await cdp.send("Page.setFontFamilies", { fontFamilies: GENERIC_FAMILIES });
  await page.goto(`file://${path}`);
  await page.addStyleTag({ content: "body { font-family: initial; }" });
  await page.evaluate(() => document.fonts.ready);

  const laidOut = laidOutText(
    await cdp.send("DOMSnapshot.captureSnapshot", { computedStyles: [] }),
  );

  // Each character's rect left and top, one per rect: a character a line wraps through has one per
  // line. White space is left out, as its rects say little a neighbour's do not.
  const characters = await page.evaluate((laidOut) => {
    const found: [string, number, number][] = [];
    const walker = document.createTreeWalker(document.body, NodeFilter.SHOW_TEXT);
    const range = document.createRange();

    for (let node = walker.nextNode(), index = 0; node; node = walker.nextNode(), index++) {
      const text = node.textContent ?? "";
      // The characters Chromium draws, after `text-transform`, where they line up with the DOM's.
      const drawn = laidOut[index]?.length === text.length ? laidOut[index] : text;

      for (let offset = 0; offset < text.length;) {
        const character = String.fromCodePoint(drawn.codePointAt(offset) ?? 0);
        const end = offset + ((text.codePointAt(offset) ?? 0) > 0xffff ? 2 : 1);

        range.setStart(node, offset);
        range.setEnd(node, end);
        offset = end;

        if (/\s/.test(character)) {
          continue;
        }

        for (const rect of range.getClientRects()) {
          found.push([character, rect.left, rect.top]);
        }
      }
    }

    return found;
  }, laidOut);

  await page.close();
  await writeFile(
    join(output, `${name}.json`),
    `${JSON.stringify({ chromium: browser.version(), characters })}\n`,
  );
  console.log(`${name}: ${characters.length} characters`);
}

await browser.close();
