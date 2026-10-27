import { unicodeRangeOf } from "../unicode-range";

/**
 * The fonts whose `unicodeRange` covers one of `codepoints`. Range-less fallbacks (a full font or a
 * `text=` subset) and entries without a range always stay.
 */
export function fontsCovering<T>(fonts: T[], codepoints: Iterable<number>): T[] {
  const sorted = Uint32Array.from(codepoints).sort();

  return fonts.filter((font) => rangesCover(unicodeRangeOf(font), sorted));
}

/** Whether any of the ascending `codepoints` falls in `ranges`; no ranges covers everything. */
function rangesCover(ranges: [number, number][], codepoints: Uint32Array): boolean {
  return (
    ranges.length === 0 ||
    ranges.some(([lo, hi]) => {
      const first = codepoints[lowerBound(codepoints, lo)];

      return first !== undefined && first <= hi;
    })
  );
}

/** The index of the first of the ascending `values` at or above `target`. */
function lowerBound(values: Uint32Array, target: number): number {
  let low = 0;
  let high = values.length;

  while (low < high) {
    const middle = (low + high) >>> 1;

    if ((values[middle] ?? target) < target) {
      low = middle + 1;
    } else {
      high = middle;
    }
  }

  return low;
}
