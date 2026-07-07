// Shared between the WASM render worker (which registers these via `googleFonts`)
// and the browser preview pane (which loads the same subsets so text routes to
// the same faces). Inter first as the sans default, Noto per script for fallback.
export const FONT_FAMILIES = [
  "Inter",
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
