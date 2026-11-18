import { afterAll, describe, expect, mock, spyOn, test } from "bun:test";
import { container, fontFromUrl, image, type Node, text } from "@takumi-rs/helpers";
import { type FontLoader, type PrepareOptions, Renderer } from "../src/export";

const monoData = await Bun.file("../assets/fonts/geist/GeistMono[wght].woff2").arrayBuffer();
const cjkData = await Bun.file(
  "../assets/fonts/noto-sans/NotoSansTC-VariableFont_wght.woff2",
).arrayBuffer();

let hookRequests = 0;
const server = Bun.serve({
  port: 0,
  fetch: (request) => {
    if (new URL(request.url).pathname !== "/hook.woff2") {
      return new Response(null, { status: 404 });
    }

    hookRequests++;

    return new Response(cjkData);
  },
});

afterAll(() => server.stop());

const options = { width: 128, height: 64 } as const;
const cjk: FontLoader = { name: "Noto Sans TC", data: cjkData };
const line = (content: string, fontFamily?: string): Node =>
  container({
    style: { width: 128, height: 64, fontSize: 24, fontFamily, backgroundColor: "#fff" },
    children: [text(content)],
  });

/** The warning for the font at `url` that failed to load. */
const fontFailure = (url: string) =>
  expect.stringContaining(`takumi: Failed to load the font ${url}: `);

const missingGlyphs = "takumi: No font draws";
const unresolvedFamily = "takumi: No font provides the family";

/** The text quoted in each of `warnings` that starts with `prefix`. */
const quotedOf = (warnings: string[], prefix: string) =>
  warnings.flatMap((warning) =>
    warning.startsWith(prefix) ? (/"([^"]*)"/.exec(warning)?.slice(1) ?? []) : [],
  );

/** The characters the missing glyphs warning among `warnings` names, none without one. */
const missingOf = (warnings: string[]) => [...quotedOf(warnings, missingGlyphs).join("")];

/** Calls `run`, returning what it returns with the messages it printed through `console.warn`. */
const printing = async <T>(run: () => Promise<T>) => {
  const warn = spyOn(console, "warn").mockImplementation(() => {});

  try {
    const result = await run();

    return { result, warnings: warn.mock.calls.map(([message]) => String(message)) };
  } finally {
    warn.mockRestore();
  }
};

/** Prepares `node` with `renderer`, collecting the warnings it prints. */
const prepareWarnings = async (
  node: Node,
  prepareOptions: PrepareOptions,
  renderer = new Renderer(),
) => {
  const { result: prepared, warnings } = await printing(() =>
    renderer.prepare(node, prepareOptions),
  );

  return { prepared, warnings };
};

describe("the warnings of a prepare", () => {
  test("is empty when the built-in font draws every character", async () => {
    const { warnings } = await prepareWarnings(line("Hello"), options);

    expect(warnings).toEqual([]);
  });

  test("names each character no font draws once", async () => {
    const { warnings } = await prepareWarnings(line("a中 b中文"), options);

    expect(warnings).toEqual([
      'takumi: No font draws "中文". Add a font that covers them to `fonts`, or return one from `onMissingGlyphs`.',
    ]);
  });

  test("leaves out the characters a listed font draws", async () => {
    const { warnings } = await prepareWarnings(line("a中"), { ...options, fonts: [cjk] });

    expect(missingOf(warnings)).toEqual([]);
  });

  test("leaves out the characters a listed subset draws, and keeps the ones past its range", async () => {
    const subset: FontLoader = { name: "Mono", data: monoData, unicodeRange: [[0x20, 0x7e]] };
    const { warnings } = await prepareWarnings(line("a中", "Mono"), {
      ...options,
      fonts: [subset],
    });

    expect(missingOf(warnings)).toEqual(["中"]);
  });

  test("names the families no font provides, but no generic keyword", async () => {
    const { warnings } = await prepareWarnings(line("a", "Missing, sans-serif"), {
      ...options,
      fontFamilies: ["Mono", "Absent"],
      fonts: [{ name: "Mono", data: monoData }],
    });

    expect(quotedOf(warnings, unresolvedFamily)).toEqual(["Missing", "Absent"]);
  });

  test("names an optional font that failed to load", async () => {
    const url = `${server.url}missing.woff2`;
    const { warnings } = await prepareWarnings(line("a"), {
      ...options,
      fonts: [fontFromUrl(url, { optional: true })],
    });

    expect(warnings).toEqual([fontFailure(url)]);
  });

  test("names an @font-face file that failed to load", async () => {
    const url = `${server.url}face.woff2`;
    const { warnings } = await prepareWarnings(line("a", "Face"), {
      ...options,
      css: [`@font-face { font-family: Face; src: url(${url}) }`],
    });

    expect(warnings).toContainEqual(fontFailure(url));
  });

  test("names an image left out under throwOnError: false", async () => {
    const src = `${server.url}missing.png`;
    const { warnings } = await prepareWarnings(
      container({ children: [image({ src, width: 16, height: 16 })] }),
      { ...options, images: { throwOnError: false } },
    );

    expect(warnings).toEqual([
      expect.stringContaining(`takumi: Failed to load the image ${src}: `),
    ]);
  });
});

describe("onMissingGlyphs", () => {
  test("loads the fonts it returns into the render, which then raises no warning", async () => {
    const renderer = new Renderer();
    const onMissingGlyphs = mock((_characters: string[]) => [cjk]);
    const without = await renderer.render(line("a中"), options);
    const { prepared, warnings } = await prepareWarnings(
      line("a中"),
      { ...options, onMissingGlyphs },
      renderer,
    );

    expect(onMissingGlyphs.mock.calls).toEqual([[["中"]]]);
    expect(warnings).toEqual([]);
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
    const { warnings } = await prepareWarnings(line("a中"), {
      ...options,
      fontFamilies: ["Mono"],
      fonts: [{ name: "Mono", data: monoData }],
      onMissingGlyphs: () => [cjk],
    });

    expect(warnings).toEqual([]);
  });

  test("is not called when every character is covered, and keeps what stays uncovered", async () => {
    const covered = mock(() => undefined);

    await new Renderer().prepare(line("abc"), { ...options, onMissingGlyphs: covered });
    expect(covered).not.toHaveBeenCalled();

    const { warnings } = await prepareWarnings(line("a中"), {
      ...options,
      onMissingGlyphs: () => [{ name: "Mono", data: monoData }],
    });

    expect(missingOf(warnings)).toEqual(["中"]);
  });

  test("leaves the warning's characters alone when it changes the list it receives", async () => {
    const { warnings } = await prepareWarnings(line("a中"), {
      ...options,
      onMissingGlyphs: (characters) => {
        characters.splice(0);

        return undefined;
      },
    });

    expect(missingOf(warnings)).toEqual(["中"]);
  });

  test("loads the @font-face files a family it adds reaches", async () => {
    const node = container({
      style: { width: 128, height: 64, fontSize: 24, fontWeight: 700, backgroundColor: "#fff" },
      children: [text("a中")],
    });
    const { warnings } = await prepareWarnings(node, {
      ...options,
      css: [
        `@font-face { font-family: Hook; font-weight: 700; src: url(${server.url}hook.woff2) }`,
      ],
      fontFamilies: ["Mono"],
      fonts: [{ name: "Mono", data: monoData }],
      onMissingGlyphs: () => [{ name: "Hook", data: cjkData, weight: 400 }],
    });

    expect(hookRequests).toBe(1);
    expect(warnings).toEqual([]);
  });
});

describe("a render without prepare", () => {
  test("prints what it could not draw when it passes onMissingGlyphs", async () => {
    const { warnings } = await printing(() =>
      new Renderer().render(line("a中", "Missing"), {
        ...options,
        onMissingGlyphs: () => undefined,
      }),
    );

    expect(missingOf(warnings)).toEqual(["中"]);
    expect(quotedOf(warnings, unresolvedFamily)).toEqual(["Missing"]);
  });
});
