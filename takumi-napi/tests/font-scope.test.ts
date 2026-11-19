import { describe, expect, test } from "bun:test";
import { readFile } from "node:fs/promises";
import { join } from "node:path";
import { Renderer } from "../src/export";

const poppins = (file: string) =>
  readFile(join(import.meta.dir, "../../assets/fonts/poppins", file));
const regular = {
  name: "Brand",
  weight: 400,
  data: await poppins("poppins-v24-devanagari_latin-regular.woff2"),
};
const bold = {
  name: "Brand",
  weight: 700,
  data: await poppins("poppins-v24-devanagari_latin-700.woff2"),
};
const html = `<div style="font-family: Brand; font-weight: 700; font-size: 48px">Brand title</div>`;
const options = { width: 400, height: 100 };

describe("a render's fonts list", () => {
  test("does not reach a later render that lists other fonts", async () => {
    const renderer = new Renderer();
    const first = await renderer.render(html, { ...options, fonts: [regular] });

    await renderer.render(html, { ...options, fonts: [bold] });

    expect(await renderer.render(html, { ...options, fonts: [regular] })).toEqual(first);
  });

  test("does not reach a later render that lists none", async () => {
    const renderer = new Renderer();
    const unlisted = await renderer.render(html, options);

    await renderer.render(html, { ...options, fonts: [bold] });

    expect(await renderer.render(html, options)).toEqual(unlisted);
  });

  test("leaves the fonts registerFont registered to every render", async () => {
    const renderer = new Renderer();

    await renderer.registerFont(bold);

    const registered = await renderer.render(html, options);

    await renderer.render(html, { ...options, fonts: [regular] });

    expect(await renderer.render(html, options)).toEqual(registered);
  });
});
