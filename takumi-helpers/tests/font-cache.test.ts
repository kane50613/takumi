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
    expect(unregister).toHaveBeenCalledWith("B");

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

describe("FontRegistry font cache while fonts load", () => {
  test("registers again a family evicted while the render's other fonts loaded", async () => {
    const { registered, registerInner, unregister } = backend();
    const registry = new FontRegistry(registerInner, { maxBytes: 150, unregister });
    let finishLoading: (bytes: Uint8Array) => void = () => {};
    const slow = {
      name: "W",
      data: () =>
        new Promise<Uint8Array>((resolve) => {
          finishLoading = resolve;
        }),
    };

    await render(registry, font("X", 100));

    const other = await registry.resolveResources([font("A", 100)], undefined, undefined);
    const loading = registry.resolveResources([font("X", 100), slow], undefined, undefined);

    await Promise.resolve();
    other.release();
    expect(registered.has("x")).toBe(false);

    finishLoading(new Uint8Array(10));

    const { fontFamilies, release } = await loading;

    expect(fontFamilies).toEqual(["X", "W"]);
    expect(registered.has("x")).toBe(true);

    release();
  });
});

describe("FontRegistry font cache contract", () => {
  test("releases a family under the name the backend gave it", async () => {
    const { registerInner, unregister } = backend();
    const registry = new FontRegistry(registerInner, { maxBytes: 100, unregister });

    await render(registry, font("Alpha", 100));
    await render(registry, font("Beta", 100));

    expect(unregister).toHaveBeenCalledWith("Alpha");
  });

  test("keeps a family a render in progress names in fontFamilies", async () => {
    const { registered, registerInner, unregister } = backend();
    const registry = new FontRegistry(registerInner, { maxBytes: 100, unregister });

    await render(registry, font("A", 100));

    const naming = await registry.resolveResources(undefined, undefined, ["A"]);

    await render(registry, font("B", 100));
    expect(registered.has("a")).toBe(true);

    naming.release();
  });

  test("rejects a negative or NaN budget", () => {
    const { registerInner } = backend();

    expect(() => new FontRegistry(registerInner, { maxBytes: -1 })).toThrow(RangeError);
    expect(() => new FontRegistry(registerInner, { maxBytes: Number.NaN })).toThrow(RangeError);
  });
});

describe("FontRegistry font cache when a reload fails", () => {
  test("lets go of a render's fonts when registering an evicted one again fails", async () => {
    const { registered, registerInner, unregister } = backend();
    const registry = new FontRegistry(registerInner, { maxBytes: 150, unregister });
    let loads = 0;
    let finishLoading: (bytes: Uint8Array) => void = () => {};
    const flaky = {
      name: "X",
      data: () => {
        loads++;

        if (loads > 1) {
          throw new Error("offline");
        }

        return new Uint8Array(100);
      },
    };
    const slow = {
      name: "W",
      data: () =>
        new Promise<Uint8Array>((resolve) => {
          finishLoading = resolve;
        }),
    };

    await registry.resolveResources([flaky], undefined, undefined).then(({ release }) => release());

    const other = await registry.resolveResources([font("A", 100)], undefined, undefined);
    const loading = registry.resolveResources([flaky, slow], undefined, undefined);

    await Promise.resolve();
    other.release();
    finishLoading(new Uint8Array(10));

    await expect(loading).rejects.toThrow("offline");

    await render(registry, font("B", 100));
    await render(registry, font("C", 100));

    expect(registered.has("w")).toBe(false);
  });
});
