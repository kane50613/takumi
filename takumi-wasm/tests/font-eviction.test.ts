import { describe, expect, mock, test } from "bun:test";
import { container, text } from "@takumi-rs/helpers";
import { Renderer } from "../bundlers/node";

const geistTtf = new Uint8Array(
  await Bun.file("../assets/fonts/geist/geist-latin-wght-300-800.ttf").arrayBuffer(),
);

/** Geist made distinct by a `tail` the font parser ignores, so each is its own font file. */
const geist = (tail: number) => {
  const bytes = new Uint8Array(geistTtf.length + 4);

  bytes.set(geistTtf);
  new DataView(bytes.buffer).setUint32(geistTtf.length, tail);

  return bytes;
};

const font = (path: string) => Bun.file(`../assets/fonts/${path}`).arrayBuffer();

const node = (fontFamily: string) =>
  container({
    style: { width: 320, height: 48, fontFamily, fontSize: 32, backgroundColor: "#fff" },
    children: [text("Hamburgefonstiv 0O1lI")],
  });

const options = { width: 320, height: 48 } as const;
const MiB = 1024 * 1024;

const same = (a: Uint8Array, b: Uint8Array | undefined) =>
  b !== undefined && Buffer.compare(a, b) === 0;

describe("font eviction", () => {
  test("one family fed new files stays within the fonts budget", async () => {
    const renderer = new Renderer({ cache: { budgets: { fonts: MiB } } });
    const files = 300;
    let loads = 0;
    const listed = (tail: number) => [
      {
        key: `inter ${tail}`,
        name: "Inter",
        data: () => {
          loads++;

          return geist(tail);
        },
      },
    ];

    for (let tail = 0; tail < files; tail++) {
      await renderer.render(node("Inter"), { ...options, fonts: listed(tail) });
    }

    const stats = renderer.cacheStats().budgets.fonts;

    loads = 0;

    for (let tail = 0; tail < files; tail++) {
      await renderer.render(node("Inter"), { ...options, fonts: listed(tail) });
    }

    expect(stats.current).toBeLessThanOrEqual(MiB);
    expect(stats.evictions).toBeGreaterThanOrEqual(files - Math.ceil(MiB / geistTtf.length));
    expect(loads).toBeGreaterThanOrEqual(files - Math.ceil(MiB / geistTtf.length));
  });

  test("renders keep the fonts they use under a budget of 0", async () => {
    const faces = await Promise.all(
      [
        "geist/Geist[wght].woff2",
        "geist/GeistMono[wght].woff2",
        "inter/inter-latin-opsz-wght-normal.woff2",
        "manrope/manrope-latin-wght-normal.woff2",
      ].map(async (path, index) => ({ name: `Face ${"ABCD"[index]}`, data: await font(path) })),
    );
    const pairs = faces.flatMap((first, index) =>
      faces.filter((_, other) => other !== index).map((second) => [first, second] as const),
    );
    const late = ({ name, data }: (typeof faces)[number]) => ({
      name,
      data: async () => {
        await Bun.sleep(Math.random() * 20);

        return data;
      },
    });
    const draw = (renderer: Renderer, [first, second]: (typeof pairs)[number]) =>
      renderer.render(
        container({
          style: { width: 320, height: 96, fontSize: 32, display: "flex", flexDirection: "column" },
          children: [
            text("Hamburgefonstiv", { fontFamily: first.name }),
            text("Hamburgefonstiv", { fontFamily: second.name }),
          ],
        }),
        { width: 320, height: 96, fonts: [late(first), late(second)] },
      );
    const unbounded = new Renderer();
    const expected = await Promise.all(pairs.map((pair) => draw(unbounded, pair)));
    const tight = new Renderer({ cache: { budgets: { fonts: 0 } } });
    const drawn = await Promise.all(pairs.map((pair) => draw(tight, pair)));

    drawn.forEach((image, index) => {
      expect(same(image, expected[index])).toBe(true);
    });
    expect(tight.cacheStats().budgets.fonts.current).toBe(0);
  });

  test("the same bytes under two keys count once", async () => {
    const mono = await font("geist/GeistMono[wght].woff2");
    const once = new Renderer();
    const twice = new Renderer();

    await once.render(node("A"), { ...options, fonts: [{ name: "A", data: mono }] });
    await twice.render(node("A"), { ...options, fonts: [{ name: "A", data: mono }] });
    await twice.render(node("B"), { ...options, fonts: [{ name: "B", data: mono }] });

    expect(twice.cacheStats().budgets.fonts.current).toBe(once.cacheStats().budgets.fonts.current);
  });

  test("fonts from registerFont stay and count toward the cache", async () => {
    const renderer = new Renderer({ cache: { budgets: { fonts: 0 } } });

    await renderer.registerFont({ name: "Kept", data: geist(1) });
    await renderer.render(node("Gone"), { ...options, fonts: [{ name: "Gone", data: geist(2) }] });

    const { memory, budgets } = renderer.cacheStats();

    expect(budgets.fonts.current).toBe(geistTtf.length + 4);
    expect(memory.current).toBeGreaterThanOrEqual(budgets.fonts.current);
  });
});

describe("face precedence", () => {
  const sans = font("geist/Geist[wght].woff2");
  const mono = font("geist/GeistMono[wght].woff2");
  /** `data` loaded after a random delay, so loads finish in any order. */
  const late = (data: ArrayBuffer) => async () => {
    await Bun.sleep(Math.random() * 10);

    return data;
  };
  const title = async (renderer: Renderer, faces: ArrayBuffer[]) =>
    renderer.render(node("Title"), {
      ...options,
      fonts: faces.map((data, index) => ({
        key: `title ${index}`,
        name: "Title",
        data: late(data),
      })),
    });

  test("the font a render lists last wins, however its loads finish", async () => {
    const faces = [await sans, await mono];
    const alone = await title(new Renderer(), [await mono]);
    const drawn = await Promise.all(Array.from({ length: 12 }, () => title(new Renderer(), faces)));

    for (const image of drawn) {
      expect(same(image, alone)).toBe(true);
    }
  });

  test("a render's order wins over the order earlier renders registered in", async () => {
    const renderer = new Renderer();
    const sansFirst = [
      { key: "sans", name: "Title", data: await sans },
      { key: "mono", name: "Title", data: await mono },
    ];
    const draw = (fonts: typeof sansFirst) => renderer.render(node("Title"), { ...options, fonts });

    const monoWins = await draw(sansFirst);
    const sansWins = await draw(sansFirst.toReversed());

    expect(same(monoWins, sansWins)).toBe(false);
    expect(same(await draw(sansFirst), monoWins)).toBe(true);
  });
});

describe("@font-face files", () => {
  const url = "https://fonts.test/face.ttf";
  const page = `<style>
    @font-face { font-family: Face; src: url(${url}) }
    p { font: 32px Face; margin: 0 }
  </style><p>Hamburgefonstiv</p>`;
  const pageOptions = { width: 320, height: 48, format: "png" as const };

  test("a prepared render loads a file the renderer evicted since prepare", async () => {
    const fetch = mock(async (_url: string) => new Response(geist(7)));
    const unbounded = new Renderer();
    const expected = await unbounded.render(
      await unbounded.prepare(page, { ...pageOptions, images: { fetch } }),
      pageOptions,
    );
    const tight = new Renderer({ cache: { budgets: { fonts: 0 } } });
    const prepared = await tight.prepare(page, { ...pageOptions, images: { fetch } });

    expect(tight.cacheStats().budgets.fonts.current).toBe(0);
    expect(same(await tight.render(prepared, pageOptions), expected)).toBe(true);
    expect(fetch).toHaveBeenCalledTimes(3);
  });

  test("a file a render uses stays over fonts no render uses again", async () => {
    const fetch = mock(async (_url: string) => new Response(geist(7)));
    const renderer = new Renderer({
      cache: { budgets: { fonts: Math.floor(geistTtf.length * 2.5) } },
    });
    const prepared = await renderer.prepare(page, { ...pageOptions, images: { fetch } });
    const expected = await renderer.render(prepared, pageOptions);

    for (const tail of [1, 2, 3]) {
      await renderer.render(node("Other"), {
        ...options,
        fonts: [{ name: "Other", data: geist(tail) }],
      });
      await renderer.render(prepared, pageOptions);
    }

    expect(fetch).toHaveBeenCalledTimes(1);
    expect(same(await renderer.render(prepared, pageOptions), expected)).toBe(true);
  });
});
