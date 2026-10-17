import { describe, expect, mock, test } from "bun:test";
import { fontBackend } from "./font-backend";

const font = (name: string) => ({ name, data: mock(() => new Uint8Array([1, 2, 3])) });

describe("FontRegistry", () => {
  test("loads a font the backend holds only once", async () => {
    const { backend, fonts } = fontBackend();
    const inter = font("Inter");

    await fonts.prepareFonts([inter]);
    await fonts.prepareFonts([inter]);

    expect(inter.data).toHaveBeenCalledTimes(1);
    expect(backend.cacheFont).toHaveBeenCalledTimes(1);
  });

  test("loads a font once for renders that ask at the same time", async () => {
    const { backend, fonts } = fontBackend();
    const inter = font("Inter");

    await Promise.all([fonts.prepareFonts([inter]), fonts.prepareFonts([inter])]);

    expect(backend.cacheFont).toHaveBeenCalledTimes(1);
  });

  test("loads a font again once the backend released it", async () => {
    const { held, backend, fonts } = fontBackend();
    const inter = font("Inter");

    await fonts.prepareFonts([inter]);
    held.clear();
    await fonts.prepareFonts([inter]);

    expect(backend.cacheFont).toHaveBeenCalledTimes(2);
  });

  test("caches a render's fonts and pins those registerFont registers", async () => {
    const { backend, fonts } = fontBackend();

    await fonts.prepareFonts([font("Inter")]);
    await fonts.register(font("Geist"));

    expect(backend.cacheFont).toHaveBeenCalledTimes(1);
    expect(backend.registerFont).toHaveBeenCalledTimes(1);
  });

  test("pins a font registerFont asks for while a render loads it", async () => {
    const { backend, fonts } = fontBackend();
    const inter = font("Inter");
    const render = fonts.prepareFonts([inter]);

    await fonts.register(inter);
    await render;

    expect(backend.registerFont).not.toHaveBeenCalled();
    expect(backend.cachedFonts.mock.calls.filter(([, pin]) => pin)).toEqual([
      ["Inter::", true],
      ["Inter::", true],
    ]);
  });

  test("loads a render's fonts again when another render pushed them out", async () => {
    const { held, backend, fonts } = fontBackend();
    const inter = font("Inter");
    const geist = {
      name: "Geist",
      data: async () => {
        await new Promise((resolve) => setTimeout(resolve));
        held.clear();

        return new Uint8Array([1]);
      },
    };

    expect(await fonts.prepareFonts([inter, geist])).toEqual(["Inter", "Geist"]);
    expect(backend.cacheFont).toHaveBeenCalledTimes(3);
  });

  test("keys a buffer by its identity", async () => {
    const { backend, fonts } = fontBackend();
    const buffer = new Uint8Array([1]);

    await fonts.register(buffer);
    await fonts.register(buffer);
    await fonts.register(new Uint8Array([1]));

    expect(backend.registerFont).toHaveBeenCalledTimes(2);
  });
});
