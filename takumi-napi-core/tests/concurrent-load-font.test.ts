import { expect, test } from "bun:test";
import { Renderer } from "../export";

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
