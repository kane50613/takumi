import { describe, expect, test } from "bun:test";
import { createHash } from "node:crypto";
import { readFile } from "node:fs/promises";
import { container, image, text } from "@takumi-rs/helpers";
import { Renderer, type FontLoader } from "../bundlers/node";

const assets = new URL("../../assets/", import.meta.url);
const faces = await Promise.all(
  [
    { name: "Geist Mono", path: "fonts/geist/GeistMono[wght].woff2" },
    { name: "Geist", path: "fonts/geist/Geist[wght].woff2" },
    { name: "Inter", path: "fonts/inter/inter-latin-opsz-wght-normal.woff2" },
    { name: "Manrope", path: "fonts/manrope/manrope-latin-wght-normal.woff2" },
    {
      name: "Plus Jakarta Sans",
      path: "fonts/plus-jakarta-sans/PlusJakartaSans-VariableFont_wght.woff2",
    },
    {
      name: "Google Sans Code",
      path: "fonts/google-sans-code/GoogleSansCode-VariableFont_wght.woff2",
    },
  ].map(async ({ name, path }) => ({ name, data: await readFile(new URL(path, assets)) })),
);
const logo = await readFile(new URL("images/yeecord.png", assets));
const formats = ["png", "webp", "jpeg"] as const;
const JOBS = 240;

type Face = (typeof faces)[number];

/** The entry of `list` that `index` lands on, wrapping around. */
function pick<T>(list: readonly T[], index: number): T {
  const item = list[index % list.length];

  if (item === undefined) {
    throw new Error("pick from an empty list");
  }

  return item;
}

/** Resolves after `ms`, so loads finish out of order and calls interleave at every await. */
const delay = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));

/** `face` loaded lazily, its bytes arriving after `wait` milliseconds. */
const lazyFont = (face: Face, wait: number): FontLoader => ({
  name: face.name,
  data: async () => {
    await delay(wait);
    return face.data;
  },
});

const scene = (index: number, family: string) =>
  container({
    style: {
      width: "100%",
      height: "100%",
      display: "flex",
      alignItems: "center",
      gap: 8,
      padding: 8,
      fontFamily: family,
      fontSize: 14 + (index % 7) * 2,
      fontWeight: 300 + (index % 6) * 100,
      backgroundColor: `hsl(${(index * 37) % 360} 70% 92%)`,
    },
    children: [
      image({ src: "logo.png", width: 24 + (index % 3) * 8, height: 24 }),
      text(`#${index} ${family} renders the quick brown fox`),
    ],
  });

type Job = (renderer: Renderer) => Promise<Uint8Array | string>;

/** Job `index`: one of the renderer's entry points, with its own fonts, size, and format. */
function job(index: number): Job {
  const face = pick(faces, index);
  const node = scene(index, face.name);
  const wait = (index * 7) % 13;
  const layout = { width: 240 + (index % 5) * 16, height: 48 + (index % 4) * 8 };
  const images = [{ src: "logo.png", data: logo }];
  const format = pick(formats, index);

  switch (index % 7) {
    case 0:
      return (renderer) =>
        renderer.render(node, { ...layout, format, images, fonts: [lazyFont(face, wait)] });
    case 1:
      return async (renderer) => {
        const prepared = await renderer.prepare(node, {
          ...layout,
          images,
          fonts: [lazyFont(face, wait)],
        });

        await delay(wait);

        return renderer.render(prepared, { format });
      };
    case 2:
      // A family of its own, since pinning one keeps all of its faces out of the font cache budget.
      return async (renderer) => {
        await delay(wait);
        await renderer.registerFont({ name: `${face.name} Pinned`, data: face.data });

        return renderer.render(scene(index, `${face.name} Pinned`), { ...layout, format, images });
      };
    case 3:
      return (renderer) =>
        renderer.renderSvg(node, { ...layout, images, fonts: [lazyFont(face, wait)] });
    case 4:
      return async (renderer) =>
        JSON.stringify(
          await renderer.measure(node, { ...layout, images, fonts: [lazyFont(face, wait)] }),
        );
    case 5:
      return (renderer) =>
        renderer.renderAsDataUrl(node, {
          ...layout,
          format,
          images,
          fonts: [lazyFont(face, wait)],
        });
    default:
      return (renderer) =>
        renderer.renderAnimation({
          ...layout,
          scenes: [
            { node, durationMs: 100 },
            { node: scene(index + 1, face.name), durationMs: 100 },
          ],
          fps: 10,
          format: "apng",
          images,
          fonts: [lazyFont(face, wait)],
        });
  }
}

const jobs = Array.from({ length: JOBS }, (_, index) => job(index));

/** A short stand-in for an output, so a failure names the jobs that differ. */
const digest = (output: Uint8Array | string) => createHash("sha256").update(output).digest("hex");

/** Every job run on `renderer` at once. */
const concurrently = (renderer: Renderer) =>
  Promise.all(jobs.map(async (run) => digest(await run(renderer))));

/** Every job run one after another, each on a renderer of its own. */
async function sequentially(): Promise<string[]> {
  const digests = [];

  for (const run of jobs) {
    using renderer = new Renderer();

    digests.push(digest(await run(renderer)));
  }

  return digests;
}

const expected = await sequentially();

describe("one renderer under concurrent calls", () => {
  test("renders what each call renders alone", async () => {
    using renderer = new Renderer();

    expect(await concurrently(renderer)).toEqual(expected);
  }, 120_000);

  test("renders the same with a font cache that releases fonts mid-flight", async () => {
    using renderer = new Renderer({ fontCacheMaxBytes: 1 });

    expect(await concurrently(renderer)).toEqual(expected);
  }, 120_000);

  test("an aborted call leaves the calls beside it intact", async () => {
    using renderer = new Renderer();
    const aborted = jobs.map(async (_, index) => {
      const controller = new AbortController();
      const face = pick(faces, index);
      const call = renderer.render(scene(index, face.name), {
        width: 240,
        height: 48,
        images: [{ src: "logo.png", data: logo }],
        fonts: [lazyFont(face, 5)],
        signal: controller.signal,
      });

      controller.abort();

      return call.then(
        () => "rendered",
        (error: unknown) => (error instanceof Error ? error.name : "unknown"),
      );
    });

    expect(await concurrently(renderer)).toEqual(expected);
    expect(new Set(await Promise.all(aborted))).toEqual(new Set(["AbortError"]));
  }, 120_000);
});
