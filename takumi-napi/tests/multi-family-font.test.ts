import { expect, test } from "bun:test";
import { Renderer } from "../src/export";

const ttcData = await Bun.file(
  new URL("../../assets/fonts/geist/GeistDuo.ttc", import.meta.url),
).arrayBuffer();

const node = {
  type: "text" as const,
  text: "multi family 0123",
  style: { color: "#111827", fontSize: 32 },
};
const options = { width: 400, height: 100, format: "png" as const };

async function coldRender() {
  const renderer = new Renderer();
  const families = await renderer.registerFont({ data: ttcData });

  return { families, image: await renderer.render(node, options) };
}

test("a multi-family ttc registers in face order on every renderer", async () => {
  const reference = await coldRender();

  expect(reference.families.map((family) => family.name)).toEqual(["Geist", "Geist Mono"]);

  for (let round = 0; round < 8; round++) {
    const { families, image } = await coldRender();

    expect(families.map((family) => family.name)).toEqual(["Geist", "Geist Mono"]);
    expect(Buffer.compare(image, reference.image)).toBe(0);
  }
});
