import { readFile } from "node:fs/promises";
import { join } from "node:path";
import type { FontDetails } from "takumi-js";
import { googleFonts, type GoogleFontFamily } from "takumi-js/helpers";

export type AssetModule = {
  fonts?: ReadonlyArray<string | ({ path: string } & Omit<FontDetails, "data">)>;
  googleFonts?: GoogleFontFamily[];
  images?: ReadonlyArray<{ src: string; path: string }>;
};

export async function loadFonts(module: AssetModule) {
  const loaders = (module.fonts ?? []).map((font) => {
    const { path, ...details } = typeof font === "string" ? { path: font } : font;

    return {
      key: path,
      data: () => readFile(join("../../assets/fonts", path)),
      ...details,
    };
  });
  const subsets = module.googleFonts ? await googleFonts(module.googleFonts) : [];

  return [...loaders, ...subsets];
}

export function loadImages(module: AssetModule) {
  return Promise.all(
    (module.images ?? []).map(async ({ src, path }) => ({
      src,
      data: await readFile(join("../../assets/images", path)),
    })),
  );
}
