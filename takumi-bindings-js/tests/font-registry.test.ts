import { describe, expect, mock, test } from "bun:test";
import type { FontLoader } from "../src/renderer";
import { fontBackend } from "./font-backend";

/** The `fontFamilies` a render with `loaders` passes on. */
const families = (fonts: ReturnType<typeof fontBackend>["fonts"], loaders: FontLoader[]) =>
  fonts.resolveResources(loaders, undefined, undefined, (resources) => resources.fontFamilies);

const font = (name: string) => ({ name, data: mock(() => new Uint8Array([1, 2, 3])) });

describe("FontRegistry", () => {
  test("loads a font the backend holds only once", async () => {
    const { backend, fonts } = fontBackend();
    const inter = font("Inter");

    await families(fonts, [inter]);
    await families(fonts, [inter]);

    expect(inter.data).toHaveBeenCalledTimes(1);
    expect(backend.cacheFont).toHaveBeenCalledTimes(1);
  });

  test("loads a font once for renders that ask at the same time", async () => {
    const { backend, fonts } = fontBackend();
    const inter = font("Inter");

    await Promise.all([families(fonts, [inter]), families(fonts, [inter])]);

    expect(backend.cacheFont).toHaveBeenCalledTimes(1);
  });

  test("loads a font again once the backend released it", async () => {
    const { held, backend, fonts } = fontBackend();
    const inter = font("Inter");

    await families(fonts, [inter]);
    held.clear();
    await families(fonts, [inter]);

    expect(backend.cacheFont).toHaveBeenCalledTimes(2);
  });

  test("caches a render's fonts and pins those registerFont registers", async () => {
    const { backend, fonts } = fontBackend();

    await families(fonts, [font("Inter")]);
    await fonts.register(font("Geist"));

    expect(backend.cacheFont).toHaveBeenCalledTimes(1);
    expect(backend.registerFont).toHaveBeenCalledTimes(1);
  });

  test("pins a font registerFont asks for while a render loads it", async () => {
    const { backend, fonts } = fontBackend();
    const inter = font("Inter");
    const render = families(fonts, [inter]);

    await fonts.register(inter);
    await render;

    expect(backend.registerFont).not.toHaveBeenCalled();
    expect(
      backend.cachedFonts.mock.calls.filter(([, pin]) => pin).map(([key, pin]) => [key, pin]),
    ).toEqual([
      ["Inter::::", true],
      ["Inter::::", true],
    ]);
  });

  test("registers a font again when registerFont joins a load whose font was released", async () => {
    const { backend, fonts } = fontBackend();
    const inter = font("Inter");

    backend.cacheFont.mockImplementation(() => [{ name: "Inter" }]);

    const render = families(fonts, [inter]);

    await fonts.register(inter);
    await render;

    expect(backend.registerFont).toHaveBeenCalledTimes(1);
  });

  test("leases a render's fonts until the render settles", async () => {
    const { leases, backend, fonts } = fontBackend();
    let during: number[] = [];

    await fonts.resolveResources([font("Inter")], undefined, undefined, async () => {
      during = [...leases];
      await new Promise((resolve) => setTimeout(resolve));
    });

    expect(during).toHaveLength(1);
    expect(backend.cacheFont.mock.calls[0]?.[2]).toBe(during[0]);
    expect(leases.size).toBe(0);
  });

  test("releases a render's leases when it fails", async () => {
    const { leases, fonts } = fontBackend();
    const failing = fonts.resolveResources([font("Inter")], undefined, undefined, () => {
      throw new Error("render failed");
    });

    await expect(failing).rejects.toThrow("render failed");
    expect(leases.size).toBe(0);
  });

  test("leases a font another render is loading under its own lease", async () => {
    const { backend, fonts } = fontBackend();
    const inter = font("Inter");

    await Promise.all([families(fonts, [inter]), families(fonts, [inter])]);

    const leased = backend.cachedFonts.mock.calls.flatMap(([, , lease]) => lease ?? []);

    expect(backend.cacheFont).toHaveBeenCalledTimes(1);
    expect(new Set(leased).size).toBe(2);
  });

  test("orders a render's fonts as it lists them", async () => {
    const { backend, fonts } = fontBackend();

    await families(fonts, [font("Inter"), font("Geist")]);

    expect(backend.orderFonts).toHaveBeenCalledWith(["Inter::::", "Geist::::"]);
  });

  test("keys a buffer by its bytes", async () => {
    const { backend, fonts } = fontBackend();
    const buffer = new Uint8Array([1]);

    await fonts.register(buffer);
    await fonts.register(buffer);
    await fonts.register(new Uint8Array([1]));
    await fonts.register(new Uint8Array([2]).buffer);

    expect(backend.registerFont).toHaveBeenCalledTimes(2);
  });

  test("registers one buffer again under another family, weight, style or unicodeRange", async () => {
    const { backend, fonts } = fontBackend();
    const data = new Uint8Array([1]);

    await fonts.register({ name: "Alpha", data });
    await fonts.register({ name: "Beta", data });
    await fonts.register({ name: "Beta", data, weight: 700 });
    await fonts.register({ name: "Beta", data, style: "italic" });
    await fonts.register({ name: "Beta", data, unicodeRange: [[0x41, 0x5a]] });
    await fonts.register({ name: "Beta", data, unicodeRange: [[0x41, 0x5a]] });

    expect(
      backend.registerFont.mock.calls.map(([font]) => ("name" in font ? font.name : "")),
    ).toEqual(["Alpha", "Beta", "Beta", "Beta", "Beta"]);
  });

  test("keys a lazy font by its unicodeRange", async () => {
    const { backend, fonts } = fontBackend();

    await fonts.register({ ...font("Inter"), unicodeRange: [[0x00, 0x7f]] });
    await fonts.register({ ...font("Inter"), unicodeRange: [[0x400, 0x4ff]] });

    expect(backend.registerFont).toHaveBeenCalledTimes(2);
  });
});
