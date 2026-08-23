// Shared between the WASM render worker (which registers these via `googleFonts`)
// and the browser preview pane (which loads the same subsets so text routes to
// the same faces). One Noto Sans superfamily: latin default plus a face per
// script, so glyphs across languages share a single design.
export const FONT_FAMILIES = [
  "Noto Sans",
  "Noto Sans TC",
  "Noto Sans JP",
  "Noto Sans KR",
  "Noto Sans SC",
  "Noto Sans Arabic",
  "Noto Sans Hebrew",
  "Noto Sans Devanagari",
  "Noto Sans Thai",
] as const;

/** Google Fonts `css2` URL covering {@link FONT_FAMILIES} across the full weight axis. */
export function googleFontsCssUrl(): string {
  const families = FONT_FAMILIES.map(
    (name) => `family=${name.replace(/ /g, "+")}:wght@100..900`,
  ).join("&");
  return `https://fonts.googleapis.com/css2?${families}&display=swap`;
}

/** Latin-only face served from this site, used when the Google Fonts request fails. */
export const FALLBACK_FONT_URL = "/fonts/Geist.woff2";
