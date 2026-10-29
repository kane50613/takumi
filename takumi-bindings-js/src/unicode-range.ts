/**
 * The `unicodeRange` a font descriptor declares, or the deprecated `ranges`.
 *
 * @internal
 */
export function unicodeRangeOf(font: unknown): [number, number][] {
  if (typeof font !== "object" || font === null) {
    return [];
  }

  if ("unicodeRange" in font && Array.isArray(font.unicodeRange)) {
    return font.unicodeRange;
  }

  if ("ranges" in font && Array.isArray(font.ranges)) {
    return font.ranges;
  }

  return [];
}
