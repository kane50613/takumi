import { expect, test } from "bun:test";
import {
  isTakumiError,
  render,
  type RenderWarning,
  type TakumiError,
  type TakumiErrorCode,
} from "../src";

// Each `expectType` call fails to compile under `tsc` once a type stops narrowing.

type Equal<A, B> =
  (<T>() => T extends A ? 1 : 2) extends <T>() => T extends B ? 1 : 2 ? true : false;

const expectType = <T extends true>(): T | undefined => undefined;

function subjectOf(warning: RenderWarning): string {
  switch (warning.code) {
    case "missing-glyphs":
      expectType<Equal<typeof warning.characters, string[]>>();

      return warning.characters.join("");
    case "unresolved-family":
      expectType<Equal<typeof warning.family, string>>();

      return warning.family;
    case "font-load-failed":
      expectType<Equal<typeof warning.source, string>>();
      expectType<Equal<typeof warning.cause, unknown>>();

      return warning.source;
    case "image-load-failed":
      expectType<Equal<typeof warning.url, string>>();

      return warning.url;
    default: {
      const unhandled: never = warning;

      return unhandled;
    }
  }
}

function warningsOf(error: unknown): RenderWarning[] {
  if (isTakumiError(error, "ERR_TAKUMI_STRICT")) {
    expectType<Equal<typeof error.code, "ERR_TAKUMI_STRICT">>();
    expectType<Equal<typeof error.name, "TakumiError">>();
    expectType<Equal<typeof error.warnings, RenderWarning[]>>();

    return error.warnings;
  }

  if (isTakumiError(error)) {
    expectType<Equal<typeof error.code, TakumiErrorCode>>();
    expectType<Equal<typeof error, TakumiError>>();
  }

  return [];
}

test("a strict render's TakumiError narrows to its warnings, and each warning to its code", async () => {
  const error: unknown = await render(<div style={{ fontFamily: "Absent" }}>a</div>, {
    width: 16,
    height: 16,
    onWarning: () => {},
    strict: true,
  }).catch((caught: unknown) => caught);

  expect(warningsOf(error).map(subjectOf)).toEqual(["Absent"]);
  expect(warningsOf(new Error("plain"))).toEqual([]);
});
