import { type Mock, mock } from "bun:test";
import { FontRegistry } from "../src/renderer";
import type { Font } from "../src/renderer";
import { RendererStore } from "../src/store";

/** A backend that holds each font under its key until a test drops it, as a renderer past its budget would. */
type HeldFamily = { name: string };

export const fontBackend = (
  store: RendererStore = new RendererStore(),
): {
  held: Map<string, HeldFamily[]>;
  files: Set<string>;
  /** The leases open now. */
  leases: Set<number>;
  backend: {
    registerFont: Mock<(font: Font, key: string) => HeldFamily[]>;
    cacheFont: Mock<(font: Font, key: string, _lease: number) => HeldFamily[]>;
    cachedFonts: Mock<(key: string, _pin: boolean, _lease?: number) => HeldFamily[] | undefined>;
    cacheFontFile: Mock<(url: string, _data: ArrayBuffer | Uint8Array, _lease: number) => void>;
    cachedFontFile: Mock<(url: string, _lease: number) => boolean>;
    leaseFonts: Mock<() => number>;
    releaseFonts: Mock<(lease: number) => void>;
    orderFonts: Mock<(keys: string[]) => void>;
    dropFonts: Mock<(keys: string[]) => string[]>;
  };
  fonts: FontRegistry<HeldFamily>;
} => {
  const held = new Map<string, HeldFamily[]>();
  const load = (font: Font, key: string) => {
    const families = [{ name: "name" in font && font.name ? font.name : "Fam" }];

    held.set(key, families);

    return families;
  };
  const files = new Set<string>();
  const leases = new Set<number>();
  let next = 0;
  const backend = {
    registerFont: mock(load),
    cacheFont: mock((font: Font, key: string, _lease: number) => load(font, key)),
    cachedFonts: mock((key: string, _pin: boolean, _lease?: number) => held.get(key)),
    cacheFontFile: mock((url: string, _data: ArrayBuffer | Uint8Array, _lease: number) => {
      files.add(url);
    }),
    cachedFontFile: mock((url: string, _lease: number) => files.has(url)),
    leaseFonts: mock(() => {
      leases.add(++next);

      return next;
    }),
    releaseFonts: mock((lease: number) => {
      leases.delete(lease);
    }),
    orderFonts: mock((_keys: string[]) => {}),
    // A held key leased by an open render stays, as the backend keeps a file a render leases.
    dropFonts: mock((keys: string[]) => {
      const dropped = leases.size > 0 ? [] : keys;

      for (const key of dropped) {
        held.delete(key);
      }

      return dropped;
    }),
  };

  return { held, files, leases, backend, fonts: new FontRegistry(backend, store) };
};
