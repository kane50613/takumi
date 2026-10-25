import { expect, spyOn, test } from "bun:test";
import { container, type Declarations } from "@takumi-rs/helpers";
import { Renderer } from "../src/export";

const renderer = new Renderer();

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

test("warn about color property with invalid type", async () => {
  await expectInvalidType(
    renderStyle({
      // @ts-expect-error: invalid type test
      color: 123,
    }),
    "color",
    "integer `123`",
    "a value of 'currentColor' or <color>; also accepts 'initial', 'unset' or 'inherit'.",
  );
});

test("warn about color property with invalid string value", async () => {
  await expectInvalidValue(
    renderStyle({
      color: "notacolor",
    }),
    "color",
    "notacolor",
    "notacolor",
    "a value of 'currentColor' or <color>; also accepts 'initial', 'unset' or 'inherit'.",
  );
});

test("warn about width property with invalid type", async () => {
  await expectInvalidType(
    renderStyle({
      // @ts-expect-error: invalid type test
      width: true,
    }),
    "width",
    "boolean `true`",
    "a value of <length>, 'min-content', 'max-content', 'fit-content' or 'stretch'; also accepts 'initial', 'unset' or 'inherit'.",
  );
});

test("warn about width property with invalid string value", async () => {
  await expectInvalidValue(
    renderStyle({
      width: "invalid",
    }),
    "width",
    "invalid",
    "invalid",
    "a value of <length>, 'min-content', 'max-content', 'fit-content' or 'stretch'; also accepts 'initial', 'unset' or 'inherit'.",
  );
});

test("warn about alignItems property with invalid type", async () => {
  await expectInvalidType(
    renderStyle({
      // @ts-expect-error: invalid type test
      alignItems: [],
    }),
    "alignItems",
    "sequence",
    "a value of 'normal', 'baseline', 'stretch', 'start', 'end', 'flex-start', 'flex-end', 'self-start', 'self-end', 'center', 'safe' or 'unsafe'; also accepts 'initial', 'unset' or 'inherit'.",
  );
});

test("warn about alignItems property with invalid string value", async () => {
  await expectInvalidValue(
    renderStyle({
      alignItems: "invalid",
    }),
    "alignItems",
    "invalid",
    "invalid",
    "a value of 'normal', 'baseline', 'stretch', 'start', 'end', 'flex-start', 'flex-end', 'self-start', 'self-end', 'center', 'safe' or 'unsafe'; also accepts 'initial', 'unset' or 'inherit'.",
  );
});

test("warn about borderRadius property with invalid type", async () => {
  await expectInvalidType(
    renderStyle({
      // @ts-expect-error: invalid type test
      borderRadius: true,
    }),
    "borderRadius",
    "boolean `true`",
    "1 to 4 length values for width, optionally followed by '/' and 1 to 4 length values for height; also accepts 'initial', 'unset' or 'inherit'.",
  );
});

test("warn about borderRadius property with invalid string value", async () => {
  await expectInvalidValue(
    renderStyle({
      borderRadius: "invalid",
    }),
    "borderRadius",
    "invalid",
    "invalid",
    "1 to 4 length values for width, optionally followed by '/' and 1 to 4 length values for height; also accepts 'initial', 'unset' or 'inherit'.",
  );
});

test("warn about borderRadius property with invalid slash syntax", async () => {
  await expectInvalidValue(
    renderStyle({
      borderRadius: "10px / invalid",
    }),
    "borderRadius",
    "10px / invalid",
    "invalid",
    "1 to 4 length values for width, optionally followed by '/' and 1 to 4 length values for height; also accepts 'initial', 'unset' or 'inherit'.",
  );
});

test("warn about padding (Sides) with invalid type", async () => {
  await expectInvalidType(
    renderStyle({
      // @ts-expect-error: invalid type test
      padding: { top: null },
    }),
    "padding",
    "map",
    "1 ~ 4 values of <length>; also accepts 'initial', 'unset' or 'inherit'.",
  );
});

test("warn about padding (Sides) with invalid string value", async () => {
  await expectInvalidValue(
    renderStyle({
      padding: "invalid",
    }),
    "padding",
    "invalid",
    "invalid",
    "1 ~ 4 values of <length>; also accepts 'initial', 'unset' or 'inherit'.",
  );
});

test("warn about gap (SpacePair) with invalid type", async () => {
  await expectInvalidType(
    renderStyle({
      // @ts-expect-error: invalid type test
      gap: true,
    }),
    "gap",
    "boolean `true`",
    "1 ~ 2 values of 'normal' or <length>; also accepts 'initial', 'unset' or 'inherit'.",
  );
});

test("warn about gap (SpacePair) with invalid string value", async () => {
  await expectInvalidValue(
    renderStyle({
      gap: "invalid",
    }),
    "gap",
    "invalid",
    "invalid",
    "1 ~ 2 values of 'normal' or <length>; also accepts 'initial', 'unset' or 'inherit'.",
  );
});

// Tests fallback error messages when neither value_description() nor enum_values() is implemented
test("warn about textDecorationLine with invalid type", async () => {
  await expectInvalidType(
    renderStyle({
      // @ts-expect-error: invalid type test
      textDecorationLine: 123,
    }),
    "textDecorationLine",
    "integer `123`",
    "a value of 'underline', 'line-through' or 'overline' or 'none'; also accepts 'initial', 'unset' or 'inherit'.",
  );
});

test("warn about textDecorationLine with invalid string value", async () => {
  await expectInvalidValue(
    renderStyle({
      textDecorationLine: "invalid",
    }),
    "textDecorationLine",
    "invalid",
    "invalid",
    "a value of 'underline', 'line-through' or 'overline' or 'none'; also accepts 'initial', 'unset' or 'inherit'.",
  );
});

test("an invalid value drops only its declaration", async () => {
  const warn = spyOn(console, "warn").mockImplementation(() => {});

  try {
    const node = container({
      children: [],
      style: { width: 4, height: "-1.5%", backgroundColor: "rgb(255, 0, 0)" },
    });
    const image = await renderer.render(node, { width: 4, height: 4, format: "raw" });

    expect(Array.from(image.subarray(0, 4))).toEqual([0, 0, 0, 0]);
    expect(warn.mock.calls.map(([message]) => String(message))).toEqual([
      expect.stringMatching(/^takumi: ignoring invalid value for height: string "-1\.5%"/),
    ]);
  } finally {
    warn.mockRestore();
  }
});
