import { afterAll, expect, spyOn, test } from "bun:test";
import { container, type Declarations } from "@takumi-rs/helpers";
import { Renderer } from "../bundlers/node";

const renderer = new Renderer();

afterAll(() => renderer.free());

/** The warnings rendering a box with `style` logs. */
function renderStyle(style: Declarations) {
  return async () => {
    const warn = spyOn(console, "warn").mockImplementation(() => {});

    try {
      await renderer.render(container({ children: [], style }), { width: 100, height: 100 });

      return warn.mock.calls.map(([message]) => String(message));
    } finally {
      warn.mockRestore();
    }
  };
}

function escapeRegex(value: string): string {
  return value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

async function expectInvalidType(
  warnings: () => Promise<string[]>,
  property: string,
  actual: string,
  expected: string,
) {
  expect(await warnings()).toEqual([
    expect.stringMatching(
      new RegExp(
        `^takumi: ignoring invalid type for ${escapeRegex(property)}: ${escapeRegex(actual)}; (?:Unexpected token: .+?, expected )?${escapeRegex(expected)}$`,
      ),
    ),
  ]);
}

async function expectInvalidValue(
  warnings: () => Promise<string[]>,
  property: string,
  input: string,
  near: string,
  expected: string,
) {
  expect(await warnings()).toEqual([
    expect.stringMatching(
      new RegExp(
        `^takumi: ignoring invalid value for ${escapeRegex(property)} near "${escapeRegex(near)}": string ${escapeRegex(JSON.stringify(input))}; (?:Unexpected token: .+?, expected )?${escapeRegex(expected)}$`,
      ),
    ),
  ]);
}

test("warn about justifyContent with wrong type", async () => {
  await expectInvalidType(
    renderStyle({
      // @ts-expect-error: invalid type test
      justifyContent: 123,
    }),
    "justifyContent",
    "integer `123`",
    "a value of 'normal', 'stretch', 'space-between', 'space-around', 'space-evenly', 'start', 'end', 'flex-start', 'flex-end', 'flow-start', 'flow-end', 'center', 'safe' or 'unsafe'; also accepts 'initial', 'unset' or 'inherit'.",
  );
});

test("warn about justifyContent with invalid string value", async () => {
  await expectInvalidValue(
    renderStyle({
      justifyContent: "star",
    }),
    "justifyContent",
    "star",
    "star",
    "a value of 'normal', 'stretch', 'space-between', 'space-around', 'space-evenly', 'start', 'end', 'flex-start', 'flex-end', 'flow-start', 'flow-end', 'center', 'safe' or 'unsafe'; also accepts 'initial', 'unset' or 'inherit'.",
  );
});
