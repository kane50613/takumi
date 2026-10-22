import { afterAll, expect, test } from "bun:test";
import { container, type Declarations } from "@takumi-rs/helpers";
import { Renderer } from "../bundlers/node";

const renderer = new Renderer();

afterAll(() => renderer.free());

function renderStyle(style: Declarations) {
  return () => renderer.render(container({ children: [], style }), { width: 100, height: 100 });
}

function escapeRegex(value: string): string {
  return value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

function expectInvalidType(
  action: () => unknown,
  property: string,
  actual: string,
  expected: string,
) {
  expect(action).toThrowError(
    new RegExp(
      `^Error: invalid type for ${escapeRegex(property)}: ${escapeRegex(actual)}; (?:Unexpected token: .+?, expected )?${escapeRegex(expected)}$`,
    ),
  );
}

function expectInvalidValue(
  action: () => unknown,
  property: string,
  input: string,
  near: string,
  expected: string,
) {
  expect(action).toThrowError(
    new RegExp(
      `^Error: invalid value for ${escapeRegex(property)} near "${escapeRegex(near)}": string ${escapeRegex(JSON.stringify(input))}; (?:Unexpected token: .+?, expected )?${escapeRegex(expected)}$`,
    ),
  );
}

test("report deserialize error for justifyContent with wrong type", () => {
  expectInvalidType(
    renderStyle({
      // @ts-expect-error: invalid type test
      justifyContent: 123,
    }),
    "justifyContent",
    "integer `123`",
    "a value of 'normal', 'stretch', 'space-between', 'space-around', 'space-evenly', 'start', 'end', 'flex-start', 'flex-end', 'flow-start', 'flow-end', 'center', 'safe' or 'unsafe'; also accepts 'initial', 'unset' or 'inherit'.",
  );
});

test("report deserialize error for justifyContent with invalid string value", () => {
  expectInvalidValue(
    renderStyle({
      justifyContent: "star",
    }),
    "justifyContent",
    "star",
    "star",
    "a value of 'normal', 'stretch', 'space-between', 'space-around', 'space-evenly', 'start', 'end', 'flex-start', 'flex-end', 'flow-start', 'flow-end', 'center', 'safe' or 'unsafe'; also accepts 'initial', 'unset' or 'inherit'.",
  );
});
