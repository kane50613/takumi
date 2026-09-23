import { expect, test } from "bun:test";
import { container, type Declarations } from "@takumi-rs/helpers";
import { Renderer } from "../src/export";

const renderer = new Renderer();

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
      `^InvalidArg, invalid type for ${escapeRegex(property)}: ${escapeRegex(actual)}; (?:Unexpected token: .+?, expected )?${escapeRegex(expected)}$`,
    ),
  );
}

function expectInvalidValue(
  action: () => unknown,
  property: string,
  input: string,
  near: string,
  expected: string,
  column = 1,
) {
  expect(action).toThrowError(
    new RegExp(
      `^InvalidArg, invalid value for ${escapeRegex(property)}, line 1, column ${column} near "${escapeRegex(near)}": string ${escapeRegex(JSON.stringify(input))}; (?:Unexpected token: .+?, expected )?${escapeRegex(expected)}$`,
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
    "a value of 'normal', 'stretch', 'space-between', 'space-around', 'space-evenly', 'start', 'end', 'flex-start', 'flex-end', 'center', 'safe' or 'unsafe'; also accepts 'initial', 'unset' or 'inherit'.",
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
    "a value of 'normal', 'stretch', 'space-between', 'space-around', 'space-evenly', 'start', 'end', 'flex-start', 'flex-end', 'center', 'safe' or 'unsafe'; also accepts 'initial', 'unset' or 'inherit'.",
  );
});

test("report deserialize error for color property with invalid type", () => {
  expectInvalidType(
    renderStyle({
      // @ts-expect-error: invalid type test
      color: 123,
    }),
    "color",
    "integer `123`",
    "a value of 'currentColor' or <color>; also accepts 'initial', 'unset' or 'inherit'.",
  );
});

test("report deserialize error for color property with invalid string value", () => {
  expectInvalidValue(
    renderStyle({
      color: "notacolor",
    }),
    "color",
    "notacolor",
    "notacolor",
    "a value of 'currentColor' or <color>; also accepts 'initial', 'unset' or 'inherit'.",
  );
});

test("report deserialize error for width property with invalid type", () => {
  expectInvalidType(
    renderStyle({
      // @ts-expect-error: invalid type test
      width: true,
    }),
    "width",
    "boolean `true`",
    "a value of <length>, 'min-content', 'max-content', 'fit-content', 'stretch' or <fit-content()>; also accepts 'initial', 'unset' or 'inherit'.",
  );
});

test("report deserialize error for width property with invalid string value", () => {
  expectInvalidValue(
    renderStyle({
      width: "invalid",
    }),
    "width",
    "invalid",
    "invalid",
    "a value of <length>, 'min-content', 'max-content', 'fit-content', 'stretch' or <fit-content()>; also accepts 'initial', 'unset' or 'inherit'.",
  );
});

test("report deserialize error for alignItems property with invalid type", () => {
  expectInvalidType(
    renderStyle({
      // @ts-expect-error: invalid type test
      alignItems: [],
    }),
    "alignItems",
    "sequence",
    "a value of 'normal', 'baseline', 'stretch', 'start', 'end', 'flex-start', 'flex-end', 'self-start', 'self-end', 'center', 'safe' or 'unsafe'; also accepts 'initial', 'unset' or 'inherit'.",
  );
});

test("report deserialize error for alignItems property with invalid string value", () => {
  expectInvalidValue(
    renderStyle({
      alignItems: "invalid",
    }),
    "alignItems",
    "invalid",
    "invalid",
    "a value of 'normal', 'baseline', 'stretch', 'start', 'end', 'flex-start', 'flex-end', 'self-start', 'self-end', 'center', 'safe' or 'unsafe'; also accepts 'initial', 'unset' or 'inherit'.",
  );
});

test("report deserialize error for borderRadius property with invalid type", () => {
  expectInvalidType(
    renderStyle({
      // @ts-expect-error: invalid type test
      borderRadius: true,
    }),
    "borderRadius",
    "boolean `true`",
    "1 to 4 length values for width, optionally followed by '/' and 1 to 4 length values for height; also accepts 'initial', 'unset' or 'inherit'.",
  );
});

test("report deserialize error for borderRadius property with invalid string value", () => {
  expectInvalidValue(
    renderStyle({
      borderRadius: "invalid",
    }),
    "borderRadius",
    "invalid",
    "invalid",
    "1 to 4 length values for width, optionally followed by '/' and 1 to 4 length values for height; also accepts 'initial', 'unset' or 'inherit'.",
  );
});

test("report deserialize error for borderRadius property with invalid slash syntax", () => {
  expectInvalidValue(
    renderStyle({
      borderRadius: "10px / invalid",
    }),
    "borderRadius",
    "10px / invalid",
    "invalid",
    "1 to 4 length values for width, optionally followed by '/' and 1 to 4 length values for height; also accepts 'initial', 'unset' or 'inherit'.",
    7,
  );
});

test("report deserialize error for padding (Sides) with invalid type", () => {
  expectInvalidType(
    renderStyle({
      // @ts-expect-error: invalid type test
      padding: { top: null },
    }),
    "padding",
    "map",
    "1 ~ 4 values of <length>; also accepts 'initial', 'unset' or 'inherit'.",
  );
});

test("report deserialize error for padding (Sides) with invalid string value", () => {
  expectInvalidValue(
    renderStyle({
      padding: "invalid",
    }),
    "padding",
    "invalid",
    "invalid",
    "1 ~ 4 values of <length>; also accepts 'initial', 'unset' or 'inherit'.",
  );
});

test("report deserialize error for gap (SpacePair) with invalid type", () => {
  expectInvalidType(
    renderStyle({
      // @ts-expect-error: invalid type test
      gap: true,
    }),
    "gap",
    "boolean `true`",
    "1 ~ 2 values of 'normal' or <length>; also accepts 'initial', 'unset' or 'inherit'.",
  );
});

test("report deserialize error for gap (SpacePair) with invalid string value", () => {
  expectInvalidValue(
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
test("report deserialize error for textDecorationLine with invalid type", () => {
  expectInvalidType(
    renderStyle({
      // @ts-expect-error: invalid type test
      textDecorationLine: 123,
    }),
    "textDecorationLine",
    "integer `123`",
    "a value of 'underline', 'line-through' or 'overline' or 'none'; also accepts 'initial', 'unset' or 'inherit'.",
  );
});

test("report deserialize error for textDecorationLine with invalid string value", () => {
  expectInvalidValue(
    renderStyle({
      textDecorationLine: "invalid",
    }),
    "textDecorationLine",
    "invalid",
    "invalid",
    "a value of 'underline', 'line-through' or 'overline' or 'none'; also accepts 'initial', 'unset' or 'inherit'.",
  );
});
