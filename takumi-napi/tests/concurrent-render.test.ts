import { expect, test } from "bun:test";
import { Renderer } from "../src/export";

const fontData = await Bun.file(
  new URL("../../assets/fonts/geist/Geist[wght].woff2", import.meta.url),
).arrayBuffer();

const renderer = new Renderer();

await renderer.registerFont({
  name: "Geist Concurrent",
  data: fontData,
  weight: 400,
  style: "normal",
});

function textNode(index: number) {
  return {
    type: "text" as const,
    text: `concurrent render ${index}`,
    style: {
      color: "#111827",
      backgroundColor: `rgb(${index * 8}, ${255 - index * 8}, 128)`,
      fontSize: 32,
    },
  };
}

test("concurrent static renders resolve to distinct valid PNGs", async () => {
  const results = await Promise.all(
    Array.from({ length: 16 }, (_, i) =>
      renderer.render(textNode(i), { width: 400, height: 100, format: "png" }),
    ),
  );

  for (const result of results) {
    expect(result).toBeInstanceOf(Buffer);
    expect(result.length).toBeGreaterThan(0);
    expect(result.subarray(0, 8)).toEqual(
      Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
    );
  }
});

test("interleaved concurrent render and measure calls all resolve", async () => {
  const renders = Array.from({ length: 8 }, (_, i) =>
    renderer.render(textNode(i * 2), { width: 400, height: 100, format: "png" }),
  );
  const measures = Array.from({ length: 8 }, (_, i) =>
    renderer.measure(textNode(i * 2 + 1), { width: 400, height: 100 }),
  );

  const [renderResults, measureResults] = await Promise.all([
    Promise.all(renders),
    Promise.all(measures),
  ]);

  for (const result of renderResults) {
    expect(result).toBeInstanceOf(Buffer);
    expect(result.length).toBeGreaterThan(0);
  }

  for (const measured of measureResults) {
    expect(Number.isFinite(measured.width)).toBe(true);
    expect(Number.isFinite(measured.height)).toBe(true);
  }
});

test("concurrent animation and static render calls all resolve", async () => {
  const scene = (index: number) => ({
    node: textNode(index),
    durationMs: 200,
  });

  const animationTasks = [
    renderer.renderAnimation({
      scenes: [scene(0), scene(1)],
      width: 200,
      height: 100,
      fps: 2,
      format: "gif" as const,
    }),
    renderer.renderAnimation({
      scenes: [scene(2), scene(3), scene(4)],
      width: 200,
      height: 100,
      fps: 2,
      format: "webp" as const,
    }),
  ];

  const staticTasks = Array.from({ length: 4 }, (_, i) =>
    renderer.render(textNode(i + 10), { width: 200, height: 100, format: "png" }),
  );

  const [gif, webp, ...staticResults] = await Promise.all([...animationTasks, ...staticTasks]);

  expect(gif.length).toBeGreaterThan(0);
  expect(gif.subarray(0, 6).toString("ascii")).toMatch(/^GIF8[79]a$/);

  expect(webp.length).toBeGreaterThan(0);
  expect(webp.subarray(0, 4).toString("ascii")).toBe("RIFF");
  expect(webp.subarray(8, 12).toString("ascii")).toBe("WEBP");

  for (const result of staticResults) {
    expect(result).toBeInstanceOf(Buffer);
    expect(result.length).toBeGreaterThan(0);
  }
});

test("concurrent renders of the same node are byte-identical to a serial render", async () => {
  const node = textNode(42);
  const options = { width: 400, height: 100, format: "png" as const };

  const concurrent = await Promise.all(
    Array.from({ length: 8 }, () => renderer.render(node, options)),
  );

  const serial = await renderer.render(node, options);

  for (const result of concurrent) {
    expect(Buffer.compare(result, serial)).toBe(0);
  }
});
