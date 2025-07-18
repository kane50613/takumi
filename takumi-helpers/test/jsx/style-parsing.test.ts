import { describe, expect, test } from "bun:test";
import { em, percentage, rem, vh, vw } from "../../src/helpers";
import {
  parseAspectRatio,
  parseBoxShadow,
  parseDisplay,
  parseFontWeight,
  parseGridAutoFlow,
  parseGridLine,
  parseGridTrackSize,
  parseLengthUnit,
  parsePosition,
  parseSideLengthUnits,
  parseTextOverflow,
  parseTrackSizingFunction,
} from "../../src/jsx/style-parsing";
import type { BoxShadow, GridLine } from "../../src/types";

describe("style-parsing", () => {
  describe("parseLengthUnit", () => {
    test("parses pixel values", () => {
      expect(parseLengthUnit(100)).toBe(100);
      expect(parseLengthUnit("50px")).toBe(50);
      expect(parseLengthUnit("0")).toBe(0);
    });

    test("parses percentage values", () => {
      expect(parseLengthUnit("50%")).toEqual(percentage(50));
      expect(parseLengthUnit("100%")).toEqual(percentage(100));
      expect(parseLengthUnit("0%")).toEqual(percentage(0));
    });

    test("parses rem values", () => {
      expect(parseLengthUnit("2rem")).toEqual(rem(2));
      expect(parseLengthUnit("1.5rem")).toEqual(rem(1.5));
    });

    test("parses em values", () => {
      expect(parseLengthUnit("1em")).toEqual(em(1));
      expect(parseLengthUnit("0.5em")).toEqual(em(0.5));
    });

    test("parses viewport units", () => {
      expect(parseLengthUnit("10vh")).toEqual(vh(10));
      expect(parseLengthUnit("5vw")).toEqual(vw(5));
    });

    test("handles special values", () => {
      expect(parseLengthUnit("auto")).toBe("auto");
      expect(parseLengthUnit("min-content")).toBe("min-content");
      expect(parseLengthUnit("max-content")).toBe("max-content");
    });

    test("returns 0 for invalid values", () => {
      expect(parseLengthUnit("invalid")).toBe(0);
      expect(parseLengthUnit("")).toBe(0);
    });
  });

  describe("parseSideLengthUnits", () => {
    test("parses single value", () => {
      expect(parseSideLengthUnits(10)).toBe(10);
      expect(parseSideLengthUnits("10px")).toBe(10);
    });

    test("parses two values", () => {
      expect(parseSideLengthUnits("10px 20px")).toEqual([10, 20]);
      expect(parseSideLengthUnits("1em 2rem")).toEqual([em(1), rem(2)]);
    });

    test("parses three values", () => {
      expect(parseSideLengthUnits("10px 20px 30px")).toEqual([10, 20, 30, 20]);
    });

    test("parses four values", () => {
      expect(parseSideLengthUnits("10px 20px 30px 40px")).toEqual([
        10, 20, 30, 40,
      ]);
    });

    test("handles mixed units", () => {
      expect(parseSideLengthUnits("10px 20% 30em 40vh")).toEqual([
        10,
        percentage(20),
        em(30),
        vh(40),
      ]);
    });
  });

  describe("parseAspectRatio", () => {
    test("parses string ratios", () => {
      expect(parseAspectRatio("16/9")).toBe(16 / 9);
      expect(parseAspectRatio("4/3")).toBe(4 / 3);
      expect(parseAspectRatio("1/1")).toBe(1);
    });

    test("parses numeric values", () => {
      expect(parseAspectRatio(1.5)).toBe(1.5);
      expect(parseAspectRatio(2)).toBe(2);
    });

    test("parses string numbers", () => {
      expect(parseAspectRatio("1.5")).toBe(1.5);
      expect(parseAspectRatio("2")).toBe(2);
    });

    test("throws error for invalid ratios", () => {
      expect(() => parseAspectRatio("16/0")).toThrow(
        "Denominator cannot be zero",
      );
      expect(parseAspectRatio("invalid")).toBeNaN();
      expect(parseAspectRatio("16/")).toBeNaN();
    });
  });

  describe("parseDisplay", () => {
    test("parses valid display values", () => {
      expect(parseDisplay("block")).toBe("block");
      expect(parseDisplay("flex")).toBe("flex");
      expect(parseDisplay("grid")).toBe("grid");
      expect(parseDisplay("none")).toBe("none");
    });

    test("falls back to block for invalid values", () => {
      expect(parseDisplay("inline")).toBe("block");
      expect(parseDisplay("table")).toBe("block");
    });
  });

  describe("parsePosition", () => {
    test("parses valid position values", () => {
      expect(parsePosition("relative")).toBe("relative");
      expect(parsePosition("absolute")).toBe("absolute");
    });

    test("maps static to relative", () => {
      expect(parsePosition("static")).toBe("relative");
    });

    test("falls back to relative for invalid values", () => {
      expect(parsePosition("fixed")).toBe("relative");
      expect(parsePosition("sticky")).toBe("relative");
    });
  });

  describe("parseBoxShadow", () => {
    test("parses simple box shadow", () => {
      const result = parseBoxShadow("2px 3px 4px #000000");
      expect(result).toEqual({
        color: 0x000000,
        offset_x: 2,
        offset_y: 3,
        blur_radius: 4,
        spread_radius: 0,
        inset: false,
      } satisfies BoxShadow);
    });

    test("parses box shadow with spread", () => {
      const result = parseBoxShadow("2px 3px 4px 5px #ff0000");
      expect(result).toEqual({
        color: 0xff0000,
        offset_x: 2,
        offset_y: 3,
        blur_radius: 4,
        spread_radius: 5,
        inset: false,
      } satisfies BoxShadow);
    });

    test("parses inset box shadow", () => {
      const result = parseBoxShadow("inset 2px 3px 4px #000000");
      expect(result).toEqual({
        color: 0x000000,
        offset_x: 2,
        offset_y: 3,
        blur_radius: 4,
        spread_radius: 0,
        inset: true,
      } satisfies BoxShadow);
    });

    test("throws error for invalid format", () => {
      expect(() => parseBoxShadow("2px")).toThrow("Invalid box-shadow");
      expect(() => parseBoxShadow("2px 3px 4px 5px 6px 7px black")).toThrow(
        "Invalid box-shadow format",
      );
    });
  });

  describe("parseGridTrackSize", () => {
    test("parses fr units", () => {
      expect(parseGridTrackSize("1fr")).toEqual([{ fr: 1 }]);
      expect(parseGridTrackSize("2.5fr")).toEqual([{ fr: 2.5 }]);
    });

    test("parses length units", () => {
      expect(parseGridTrackSize("100px")).toEqual([100]);
      expect(parseGridTrackSize("50%")).toEqual([percentage(50)]);
      expect(parseGridTrackSize("2rem")).toEqual([rem(2)]);
    });

    test("parses multiple values", () => {
      expect(parseGridTrackSize("1fr 2fr 100px")).toEqual([
        { fr: 1 },
        { fr: 2 },
        100,
      ]);
    });
  });

  describe("parseGridAutoFlow", () => {
    test("parses valid values", () => {
      expect(parseGridAutoFlow("row")).toBe("row");
      expect(parseGridAutoFlow("column")).toBe("column");
      expect(parseGridAutoFlow("row dense")).toBe("row-dense");
      expect(parseGridAutoFlow("column dense")).toBe("column-dense");
    });

    test("falls back to row for invalid values", () => {
      expect(parseGridAutoFlow("invalid")).toBe("row");
    });
  });

  describe("parseGridLine", () => {
    test("parses span values", () => {
      expect(parseGridLine("span 2")).toEqual({
        start: 2,
        end: null,
      } satisfies GridLine);
      expect(parseGridLine("span 1")).toEqual({
        start: 1,
        end: null,
      } satisfies GridLine);
    });

    test("parses line ranges", () => {
      expect(parseGridLine("1/3")).toEqual({
        start: 1,
        end: 3,
      } satisfies GridLine);
      expect(parseGridLine("2/5")).toEqual({
        start: 2,
        end: 5,
      } satisfies GridLine);
    });

    test("parses auto values", () => {
      expect(parseGridLine("auto")).toEqual({
        start: null,
        end: null,
      } satisfies GridLine);
    });

    test("parses named lines", () => {
      expect(parseGridLine("header-start")).toEqual({
        start: "header-start",
        end: null,
      } satisfies GridLine);
    });

    test("parses single numbers", () => {
      expect(parseGridLine("3")).toEqual({
        start: 3,
        end: null,
      } satisfies GridLine);
    });
  });

  describe("parseTrackSizingFunction", () => {
    test("parses simple track sizes", () => {
      expect(parseTrackSizingFunction("1fr 2fr")).toEqual([
        { single: { fr: 1 } },
        { single: { fr: 2 } },
      ]);
    });

    test("parses repeat functions", () => {
      expect(parseTrackSizingFunction("repeat(3, 1fr)")).toEqual([
        { repeat: [3, [{ fr: 1 }]] },
      ]);
    });

    test("parses auto-fill repeat", () => {
      expect(parseTrackSizingFunction("repeat(auto-fill, 100px)")).toEqual([
        { repeat: ["auto-fill", [100]] },
      ]);
    });

    test("parses auto-fit repeat", () => {
      expect(parseTrackSizingFunction("repeat(auto-fit, 1fr)")).toEqual([
        { repeat: ["auto-fit", [{ fr: 1 }]] },
      ]);
    });
  });

  describe("parseTextOverflow", () => {
    test("parses valid values", () => {
      expect(parseTextOverflow("ellipsis")).toBe("ellipsis");
      expect(parseTextOverflow("clip")).toBe("clip");
    });

    test("falls back to clip for invalid values", () => {
      expect(parseTextOverflow("fade")).toBe("clip");
    });
  });

  describe("parseFontWeight", () => {
    test("parses numeric values", () => {
      expect(parseFontWeight(400)).toBe(400);
      expect(parseFontWeight(700)).toBe(700);
      expect(parseFontWeight(300)).toBe(300);
    });

    test("parses string keywords", () => {
      expect(parseFontWeight("normal")).toBe(400);
      expect(parseFontWeight("bold")).toBe(700);
      expect(parseFontWeight("lighter")).toBe(300);
      expect(parseFontWeight("bolder")).toBe(600);
    });

    test("parses string numbers", () => {
      expect(parseFontWeight("500")).toBe(500);
      expect(parseFontWeight("900")).toBe(900);
    });

    test("clamps values to valid range", () => {
      expect(parseFontWeight(0)).toBe(1);
      expect(parseFontWeight(1500)).toBe(1000);
    });

    test("falls back to 400 for invalid values", () => {
      expect(parseFontWeight("invalid")).toBe(400);
    });
  });
});
