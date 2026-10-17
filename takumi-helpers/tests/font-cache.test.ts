import { describe, expect, mock, test } from "bun:test";
import { FontRegistry } from "../src/renderer";
import type { Font } from "../src/renderer";

/** A backend that keeps the families it was given, by lowercased name. */
const backend = () => {
  const registered = new Set<string>();
  const registerInner = mock((font: Font) => {
    const name = "name" in font && font.name ? font.name : "Fam";

    registered.add(name.toLowerCase());

    return [{ name }];
  });
  const unregister = mock((family: string) => registered.delete(family.toLowerCase()));

  return { registered, registerInner, unregister };
};

const font = (name: string, bytes: number) => ({ name, data: () => new Uint8Array(bytes) });

/** Renders once with `fonts`, releasing them when done. */
const render = async (
  registry: FontRegistry<{ name: string }>,
  ...fonts: ReturnType<typeof font>[]
) => {
  const { release } = await registry.resolveResources(fonts, undefined, undefined);

  release();
};

describe("FontRegistry font cache", () => {
  test("releases the fonts renders used least recently once past the budget", async () => {
    const { registered, registerInner, unregister } = backend();
    const registry = new FontRegistry(registerInner, { maxBytes: 250, unregister });

    await render(registry, font("A", 100));
    await render(registry, font("B", 100));
    await render(registry, font("A", 100));
    await render(registry, font("C", 100));

    expect([...registered].sort()).toEqual(["a", "c"]);
    expect(unregister).toHaveBeenCalledWith("b");

    await render(registry, font("B", 100));

    expect(registerInner).toHaveBeenCalledTimes(4);
    expect(registered.has("b")).toBe(true);
  });

  test("never releases a font a render in progress uses", async () => {
    const { registered, registerInner, unregister } = backend();
    const registry = new FontRegistry(registerInner, { maxBytes: 150, unregister });
    const running = await registry.resolveResources([font("A", 100)], undefined, undefined);

    await render(registry, font("B", 100));

    expect(registered.has("a")).toBe(true);
    expect(registered.has("b")).toBe(false);

    running.release();
  });

  test("keeps fonts from registerFont", async () => {
    const { registered, registerInner, unregister } = backend();
    const registry = new FontRegistry(registerInner, { maxBytes: 50, unregister });

    await registry.register(font("Pinned", 100));
    await render(registry, font("A", 100));

    expect([...registered].sort()).toEqual(["pinned"]);
  });

  test("keeps every font without a budget", async () => {
    const { registered, registerInner, unregister } = backend();
    const registry = new FontRegistry(registerInner, { unregister });

    await render(registry, font("A", 100));
    await render(registry, font("B", 100));

    expect(unregister).not.toHaveBeenCalled();
    expect(registered.size).toBe(2);
  });
});
