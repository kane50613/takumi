import { mock } from "bun:test";
import { FontRegistry } from "../src/renderer";
import type { Font } from "../src/renderer";

/** A backend that holds each font under its key until a test drops it, as a renderer past its budget would. */
export const fontBackend = () => {
  const held = new Map<string, { name: string }[]>();
  const load = (font: Font, key: string) => {
    const families = [{ name: "name" in font && font.name ? font.name : "Fam" }];

    held.set(key, families);

    return families;
  };
  const backend = {
    registerFont: mock(load),
    cacheFont: mock(load),
    cachedFonts: mock((key: string, _pin: boolean) => held.get(key)),
  };

  return { held, backend, fonts: new FontRegistry(backend) };
};
