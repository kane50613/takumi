import { type Mock, mock } from "bun:test";
import { FontRegistry } from "../src/renderer";
import type { Font } from "../src/renderer";

/** A backend that holds each font under its key until a test drops it, as a renderer past its budget would. */
type HeldFamily = { name: string };

export const fontBackend = (): {
  held: Map<string, HeldFamily[]>;
  backend: {
    registerFont: Mock<(font: Font, key: string) => HeldFamily[]>;
    cacheFont: Mock<(font: Font, key: string) => HeldFamily[]>;
    cachedFonts: Mock<(key: string, _pin: boolean) => HeldFamily[] | undefined>;
  };
  fonts: FontRegistry<HeldFamily>;
} => {
  const held = new Map<string, HeldFamily[]>();
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
