import { expect, test } from "bun:test";
import { Renderer } from "../src/export";

const fontData = await Bun.file(
  new URL("../../assets/fonts/geist/Geist[wght].woff2", import.meta.url),
).arrayBuffer();

test("concurrent loadFont calls on one renderer", async () => {
  const renderer = new Renderer({
    loadDefaultFonts: false,
  });

  const results = await Promise.all(
    Array.from({ length: 32 }, (_, i) =>
      renderer.loadFont({
        name: `Geist Concurrent ${i}`,
        data: fontData,
        weight: 400,
        style: "normal",
      }),
    ),
  );

  expect(results.every((count) => count === 1)).toBe(true);

  const output = await renderer.render({
    type: "text",
    text: "concurrent loadFont",
    style: {
      color: "#111827",
      fontSize: 48,
    },
  });

  expect(output).toBeInstanceOf(Buffer);
});

test("loadFonts retries loaders that failed before loading", async () => {
  const renderer = new Renderer({
    loadDefaultFonts: false,
  });

  let attempts = 0;

  await expect(
    renderer.loadFonts([
      {
        name: "Geist Retry",
        weight: 400,
        style: "normal",
        async data() {
          attempts += 1;

          if (attempts === 1) {
            throw new Error("transient font loader failure");
          }

          return fontData;
        },
      },
    ]),
  ).rejects.toThrow("transient font loader failure");

  const loadedCount = await renderer.loadFonts([
    {
      name: "Geist Retry",
      weight: 400,
      style: "normal",
      async data() {
        attempts += 1;
        return fontData;
      },
    },
  ]);

  expect(loadedCount).toBe(1);
  expect(attempts).toBe(2);
});
