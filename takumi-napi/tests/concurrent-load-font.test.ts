import { expect, test } from "bun:test";
import { Renderer } from "../src/export";

const fontData = await Bun.file(
  new URL("../../assets/fonts/geist/Geist[wght].woff2", import.meta.url),
).arrayBuffer();

test("concurrent registerFont calls on one renderer", async () => {
  const renderer = new Renderer();

  const results = await Promise.all(
    Array.from({ length: 32 }, (_, i) =>
      renderer.registerFont({
        name: `Geist Concurrent ${i}`,
        data: fontData,
        weight: 400,
        style: "normal",
      }),
    ),
  );

  expect(results.every((families) => families[0] && families[0].faces.length > 0)).toBe(true);

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

test("registerFont retries loaders that failed before loading", async () => {
  const renderer = new Renderer();

  let attempts = 0;

  await expect(
    renderer.registerFont({
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
    }),
  ).rejects.toThrow("transient font loader failure");

  const registered = await renderer.registerFont({
    name: "Geist Retry",
    weight: 400,
    style: "normal",
    async data() {
      attempts += 1;
      return fontData;
    },
  });

  expect(registered).toHaveLength(1);
  expect(registered.every((family) => family.faces.length > 0)).toBe(true);
  expect(attempts).toBe(2);
});
