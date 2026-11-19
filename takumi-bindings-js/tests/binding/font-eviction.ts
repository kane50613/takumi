import { describe, expect, mock, test } from "bun:test";
import { container, text } from "@takumi-rs/helpers";
import type { BindingRenderer } from "./cache";

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

/** Runs the font eviction suite against a binding's `Renderer`. */
export function testFontEviction(Renderer: BindingRenderer): void {
  describe("font eviction", () => {
    test("one family fed new files stays within the fonts budget", async () => {
      const renderer = new Renderer({ cache: { maxBytes: MiB } });
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

      const stats = renderer.cacheStats().fonts;

      loads = 0;

      for (let tail = 0; tail < files; tail++) {
        await renderer.render(node("Inter"), { ...options, fonts: listed(tail) });
      }

      expect(stats.current).toBeLessThanOrEqual(MiB);
      expect(stats.evictions).toBeGreaterThanOrEqual(files - Math.ceil(MiB / geistTtf.length));
      expect(loads).toBeGreaterThanOrEqual(files - Math.ceil(MiB / geistTtf.length));
    }, 30_000);

    test("concurrent renders keep the fonts they use under a budget of 0", async () => {
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
      const draw = (
        renderer: InstanceType<BindingRenderer>,
        [first, second]: (typeof pairs)[number],
      ) =>
        renderer.render(
          container({
            style: {
              width: 320,
              height: 96,
              fontSize: 32,
              display: "flex",
              flexDirection: "column",
            },
            children: [
              text("Hamburgefonstiv", { fontFamily: first.name }),
              text("Hamburgefonstiv", { fontFamily: second.name }),
            ],
          }),
          { width: 320, height: 96, fonts: [late(first), late(second)] },
        );
      const unbounded = new Renderer();
      const expected = await Promise.all(pairs.map((pair) => draw(unbounded, pair)));
      const tight = new Renderer({ cache: { fonts: "none" } });
      const drawn = await Promise.all(pairs.map((pair) => draw(tight, pair)));

      drawn.forEach((image, index) => {
        expect(Buffer.compare(image, expected[index] ?? Buffer.alloc(0))).toBe(0);
      });
      expect(tight.cacheStats().fonts.current).toBe(0);
    });

    test("the same bytes under two keys count once", async () => {
      const mono = await font("geist/GeistMono[wght].woff2");
      const once = new Renderer();
      const twice = new Renderer();

      await once.render(node("A"), { ...options, fonts: [{ name: "A", data: mono }] });
      await twice.render(node("A"), { ...options, fonts: [{ name: "A", data: mono }] });
      await twice.render(node("B"), { ...options, fonts: [{ name: "B", data: mono }] });

      expect(twice.cacheStats().fonts.current).toBe(once.cacheStats().fonts.current);
    });

    test("fonts from registerFont stay and count toward the cache", async () => {
      const renderer = new Renderer({ cache: { fonts: "none" } });

      await renderer.registerFont({ name: "Kept", data: geist(1) });
      await renderer.render(node("Gone"), {
        ...options,
        fonts: [{ name: "Gone", data: geist(2) }],
      });

      const { memory, fonts } = renderer.cacheStats();

      expect(fonts.current).toBe(geistTtf.length + 4);
      expect(memory.current).toBeGreaterThanOrEqual(fonts.current);
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
    const title = async (renderer: InstanceType<BindingRenderer>, faces: ArrayBuffer[]) =>
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
      const drawn = await Promise.all(
        Array.from({ length: 12 }, () => title(new Renderer(), faces)),
      );

      for (const image of drawn) {
        expect(Buffer.compare(image, alone)).toBe(0);
      }
    });

    test("a render's order wins over the order earlier renders registered in", async () => {
      const renderer = new Renderer();
      const sansFirst = [
        { key: "sans", name: "Title", data: await sans },
        { key: "mono", name: "Title", data: await mono },
      ];
      const draw = (fonts: typeof sansFirst) =>
        renderer.render(node("Title"), { ...options, fonts });

      const monoWins = await draw(sansFirst);
      const sansWins = await draw(sansFirst.toReversed());

      expect(Buffer.compare(monoWins, sansWins)).not.toBe(0);
      expect(Buffer.compare(await draw(sansFirst), monoWins)).toBe(0);
    });
  });

  describe("@font-face files", () => {
    const url = "https://fonts.test/face.ttf";
    const page = `<style>
      @font-face { font-family: Face; src: url(${url}) }
      p { font: 32px Face; margin: 0 }
    </style><p>Hamburgefonstiv</p>`;
    const pageOptions = { width: 320, height: 48, format: "png" as const };

    /** A renderer whose pool fits what one render of `page` with `data` keeps, and little more. */
    const fitting = async (data: ArrayBuffer) => {
      const probe = new Renderer();
      const fetch = async (_url: string) => new Response(data);

      await probe.render(
        await probe.prepare(page, { ...pageOptions, images: { fetch } }),
        pageOptions,
      );

      return new Renderer({ cache: { maxBytes: probe.cacheStats().memory.current + 4096 } });
    };

    /** Fills `renderer`'s pool with another font, which evicts the files no render holds. */
    const crowd = (renderer: InstanceType<BindingRenderer>) =>
      renderer.render(node("Crowd"), { ...options, fonts: [{ name: "Crowd", data: geist(9) }] });

    test("a prepared render loads a file the renderer evicted since prepare", async () => {
      const fetch = mock(async (_url: string) => new Response(geist(7)));
      const unbounded = new Renderer();
      const expected = await unbounded.render(
        await unbounded.prepare(page, { ...pageOptions, images: { fetch } }),
        pageOptions,
      );
      const tight = new Renderer({ cache: { fonts: "none" } });
      const prepared = await tight.prepare(page, { ...pageOptions, images: { fetch } });

      expect(tight.cacheStats().fonts.current).toBe(0);
      expect(Buffer.compare(await tight.render(prepared, pageOptions), expected)).toBe(0);
      expect(fetch).toHaveBeenCalledTimes(3);
    });

    test("a prepare made while the renderer held a file loads it again once evicted", async () => {
      const manrope = await font("manrope/manrope-latin-wght-normal.woff2");
      const fetch = mock(async (_url: string) => new Response(manrope));
      const renderer = await fitting(manrope);
      const expected = await renderer.render(
        await renderer.prepare(page, { ...pageOptions, images: { fetch } }),
        pageOptions,
      );
      const warm = await renderer.prepare(page, { ...pageOptions, images: { fetch } });

      await crowd(renderer);

      expect(fetch).toHaveBeenCalledTimes(1);
      expect(Buffer.compare(await renderer.render(warm, pageOptions), expected)).toBe(0);
      expect(fetch).toHaveBeenCalledTimes(2);
    });

    test("a file that fails to load again moves its rule on to its next src", async () => {
      const next = "https://fonts.test/next.ttf";
      const twoSources = page.replace(`url(${url})`, `url(${url}), url(${next})`);
      const [manrope, mono] = await Promise.all([
        font("manrope/manrope-latin-wght-normal.woff2"),
        font("geist/GeistMono[wght].woff2"),
      ]);
      let firstFails = true;
      const fetch = mock(async (requested: string) =>
        requested === url && firstFails
          ? new Response(null, { status: 404 })
          : new Response(requested === url ? manrope : mono),
      );
      const fallback = new Renderer();
      const expected = await fallback.render(
        await fallback.prepare(twoSources, { ...pageOptions, images: { fetch } }),
        pageOptions,
      );

      firstFails = false;

      const renderer = await fitting(manrope);
      const prepared = await renderer.prepare(twoSources, { ...pageOptions, images: { fetch } });

      firstFails = true;
      await crowd(renderer);

      expect(Buffer.compare(await renderer.render(prepared, pageOptions), expected)).toBe(0);
    });

    test("a file a render uses stays over fonts no render uses again", async () => {
      const fetch = mock(async (_url: string) => new Response(geist(7)));
      const renderer = new Renderer({
        cache: { maxBytes: Math.floor(geistTtf.length * 3.5) },
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
      expect(Buffer.compare(await renderer.render(prepared, pageOptions), expected)).toBe(0);
    });
  });
}
