import { describe, expect, mock, test } from "bun:test";
import { FontRegistry } from "../src/renderer";
import type { Font } from "../src/renderer";

/** A backend whose registrations resolve only when `finish` is called. */
const slowBackend = () => {
  const pending: (() => void)[] = [];
  const registerInner = mock(
    (font: Font) =>
      new Promise<{ name: string }[]>((resolve) => {
        pending.push(() => resolve([{ name: "name" in font && font.name ? font.name : "Fam" }]));
      }),
  );

  return { registerInner, finish: () => pending.shift()?.() };
};

const font = (name: string) => ({ name, data: () => new Uint8Array(1) });

describe("FontRegistry release", () => {
  test("a font whose family was released while it registered registers again", async () => {
    const { registerInner, finish } = slowBackend();
    const registry = new FontRegistry(registerInner);
    const first = registry.register(font("A"));

    await Promise.resolve();
    registry.forget("a");
    finish();
    await first;

    const second = registry.register(font("A"));

    await Promise.resolve();
    finish();
    await second;

    expect(registerInner).toHaveBeenCalledTimes(2);
  });

  test("releasing another family leaves a pending registration cached", async () => {
    const { registerInner, finish } = slowBackend();
    const registry = new FontRegistry(registerInner);
    const first = registry.register(font("A"));

    await Promise.resolve();
    registry.forget("b");
    finish();
    await first;
    await registry.register(font("A"));

    expect(registerInner).toHaveBeenCalledTimes(1);
  });

  test("matches family names as the backend folds them, a character at a time", async () => {
    const registerInner = mock((_font: Font) => [{ name: "ΟΣ" }]);
    const registry = new FontRegistry(registerInner);

    await registry.register(font("ΟΣ"));
    registry.forget("οσ");
    await registry.register(font("ΟΣ"));

    expect(registerInner).toHaveBeenCalledTimes(2);
  });
});
