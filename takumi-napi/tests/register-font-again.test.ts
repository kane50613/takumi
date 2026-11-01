import { expect, test } from "bun:test";
import { Renderer } from "../index";

const fontData = await Bun.file("../assets/fonts/geist/Geist[wght].woff2").arrayBuffer();
const options = { width: 240, height: 64, format: "png" as const };
const text = (fontFamily: string) => ({
  type: "text" as const,
  text: "Again",
  style: { fontFamily, fontSize: 32 },
});

test("one font registered under two families draws both in it", async () => {
  const renderer = new Renderer();

  const alpha = await renderer.registerFont({ name: "Alpha", data: fontData });
  const beta = await renderer.registerFont({ name: "Beta", data: new Uint8Array(fontData) });

  expect(alpha.map((family) => family.name)).toEqual(["Alpha"]);
  expect(beta.map((family) => family.name)).toEqual(["Beta"]);

  const [drawnInAlpha, drawnInBeta] = await Promise.all([
    renderer.render(text("Alpha"), options),
    renderer.render(text("Beta"), options),
  ]);

  expect(Buffer.compare(drawnInAlpha, drawnInBeta)).toBe(0);
});

test("a font registered again after its family is removed loads its bytes again", async () => {
  const renderer = new Renderer();

  await renderer.registerFont({ name: "Alpha", data: fontData });
  const drawn = await renderer.render(text("Alpha"), options);

  renderer.unregisterFont("Alpha");
  await renderer.registerFont({ name: "Beta", data: fontData });

  expect(Buffer.compare(await renderer.render(text("Beta"), options), drawn)).toBe(0);
});
