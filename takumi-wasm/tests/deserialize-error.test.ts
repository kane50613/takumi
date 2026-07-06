import { afterAll, expect, test } from "bun:test";
import { container } from "@takumi-rs/helpers";
import { Renderer } from "../bundlers/node";

const renderer = new Renderer();

afterAll(() => renderer.free());

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
  column = 1,
) {
  expect(action).toThrowError(
    new RegExp(
      `^Error: invalid value for ${escapeRegex(property)}, line 1, column ${column} near "${escapeRegex(near)}": string ${escapeRegex(JSON.stringify(input))}; (?:Unexpected token: .+?, expected )?${escapeRegex(expected)}$`,
    ),
  );
}

test("report deserialize error for justifyContent with wrong type", () => {
  expectInvalidType(
    () =>
      renderer.render(
        container({
          children: [],
          style: {
            // @ts-expect-error: invalid type test
            justifyContent: 123,
          },
        }),
        {
          width: 100,
          height: 100,
        },
      ),
    "justifyContent",
    "integer `123`",
    "a value of 'normal', 'stretch', 'space-between', 'space-around', 'space-evenly', 'start', 'end', 'flex-start', 'flex-end', 'center', 'safe' or 'unsafe'; also accepts 'initial', 'unset' or 'inherit'.",
  );
});

test("report deserialize error for justifyContent with invalid string value", () => {
  expectInvalidValue(
    () =>
      renderer.render(
        container({
          children: [],
          style: {
            justifyContent: "star",
          },
        }),
        {
          width: 100,
          height: 100,
        },
      ),
    "justifyContent",
    "star",
    "star",
    "a value of 'normal', 'stretch', 'space-between', 'space-around', 'space-evenly', 'start', 'end', 'flex-start', 'flex-end', 'center', 'safe' or 'unsafe'; also accepts 'initial', 'unset' or 'inherit'.",
  );
});

test("report deserialize error for color property with invalid type", () => {
  expectInvalidType(
    () =>
      renderer.render(
        container({
          children: [],
          style: {
            // @ts-expect-error: invalid type test
            color: 123,
          },
        }),
        {
          width: 100,
          height: 100,
        },
      ),
    "color",
    "integer `123`",
    "a value of 'currentColor' or <color>; also accepts 'initial', 'unset' or 'inherit'.",
  );
});

test("report deserialize error for color property with invalid string value", () => {
  expectInvalidValue(
    () =>
      renderer.render(
        {
          type: "container",
          children: [],
          style: {
            color: "notacolor",
          },
        },
        {
          width: 100,
          height: 100,
        },
      ),
    "color",
    "notacolor",
    "notacolor",
    "a value of 'currentColor' or <color>; also accepts 'initial', 'unset' or 'inherit'.",
  );
});

test("report deserialize error for width property with invalid type", () => {
  expectInvalidType(
    () =>
      renderer.render(
        container({
          children: [],
          style: {
            // @ts-expect-error: invalid type test
            width: true,
          },
        }),
        {
          width: 100,
          height: 100,
        },
      ),
    "width",
    "boolean `true`",
    "a value of <length>; also accepts 'initial', 'unset' or 'inherit'.",
  );
});

test("report deserialize error for width property with invalid string value", () => {
  expectInvalidValue(
    () =>
      renderer.render(
        {
          type: "container",
          children: [],
          style: {
            width: "invalid",
          },
        },
        {
          width: 100,
          height: 100,
        },
      ),
    "width",
    "invalid",
    "invalid",
    "a value of <length>; also accepts 'initial', 'unset' or 'inherit'.",
  );
});

test("report deserialize error for alignItems property with invalid type", () => {
  expectInvalidType(
    () =>
      renderer.render(
        container({
          children: [],
          style: {
            // @ts-expect-error: invalid type test
            alignItems: [],
          },
        }),
        {
          width: 100,
          height: 100,
        },
      ),
    "alignItems",
    "sequence",
    "a value of 'normal', 'baseline', 'stretch', 'start', 'end', 'flex-start', 'flex-end', 'center', 'safe' or 'unsafe'; also accepts 'initial', 'unset' or 'inherit'.",
  );
});

test("report deserialize error for alignItems property with invalid string value", () => {
  expectInvalidValue(
    () =>
      renderer.render(
        container({
          children: [],
          style: {
            alignItems: "invalid",
          },
        }),
        {
          width: 100,
          height: 100,
        },
      ),
    "alignItems",
    "invalid",
    "invalid",
    "a value of 'normal', 'baseline', 'stretch', 'start', 'end', 'flex-start', 'flex-end', 'center', 'safe' or 'unsafe'; also accepts 'initial', 'unset' or 'inherit'.",
  );
});

test("report deserialize error for borderRadius property with invalid type", () => {
  expectInvalidType(
    () =>
      renderer.render(
        container({
          children: [],
          style: {
            // @ts-expect-error: invalid type test
            borderRadius: true,
          },
        }),
        {
          width: 100,
          height: 100,
        },
      ),
    "borderRadius",
    "boolean `true`",
    "1 to 4 length values for width, optionally followed by '/' and 1 to 4 length values for height; also accepts 'initial', 'unset' or 'inherit'.",
  );
});

test("report deserialize error for borderRadius property with invalid string value", () => {
  expectInvalidValue(
    () =>
      renderer.render(
        {
          type: "container",
          children: [],
          style: {
            borderRadius: "invalid",
          },
        },
        {
          width: 100,
          height: 100,
        },
      ),
    "borderRadius",
    "invalid",
    "invalid",
    "1 to 4 length values for width, optionally followed by '/' and 1 to 4 length values for height; also accepts 'initial', 'unset' or 'inherit'.",
  );
});

test("report deserialize error for borderRadius property with invalid slash syntax", () => {
  expectInvalidValue(
    () =>
      renderer.render(
        {
          type: "container",
          children: [],
          style: {
            borderRadius: "10px / invalid",
          },
        },
        {
          width: 100,
          height: 100,
        },
      ),
    "borderRadius",
    "10px / invalid",
    "invalid",
    "1 to 4 length values for width, optionally followed by '/' and 1 to 4 length values for height; also accepts 'initial', 'unset' or 'inherit'.",
    7,
  );
});

test("report deserialize error for padding (Sides) with invalid type", () => {
  expectInvalidType(
    () =>
      renderer.render(
        container({
          children: [],
          style: {
            // @ts-expect-error: invalid type test
            padding: { top: null },
          },
        }),
        {
          width: 100,
          height: 100,
        },
      ),
    "padding",
    "map",
    "1 ~ 4 values of <length>; also accepts 'initial', 'unset' or 'inherit'.",
  );
});

test("report deserialize error for padding (Sides) with invalid string value", () => {
  expectInvalidValue(
    () =>
      renderer.render(
        {
          type: "container",
          children: [],
          style: {
            padding: "invalid",
          },
        },
        {
          width: 100,
          height: 100,
        },
      ),
    "padding",
    "invalid",
    "invalid",
    "1 ~ 4 values of <length>; also accepts 'initial', 'unset' or 'inherit'.",
  );
});

test("report deserialize error for gap (SpacePair) with invalid type", () => {
  expectInvalidType(
    () =>
      renderer.render(
        container({
          children: [],
          style: {
            // @ts-expect-error: invalid type test
            gap: true,
          },
        }),
        {
          width: 100,
          height: 100,
        },
      ),
    "gap",
    "boolean `true`",
    "1 ~ 2 values of 'normal' or <length>; also accepts 'initial', 'unset' or 'inherit'.",
  );
});

test("report deserialize error for gap (SpacePair) with invalid string value", () => {
  expectInvalidValue(
    () =>
      renderer.render(
        {
          type: "container",
          children: [],
          style: {
            gap: "invalid",
          },
        },
        {
          width: 100,
          height: 100,
        },
      ),
    "gap",
    "invalid",
    "invalid",
    "1 ~ 2 values of 'normal' or <length>; also accepts 'initial', 'unset' or 'inherit'.",
  );
});

// Tests fallback error messages when neither value_description() nor enum_values() is implemented
test("report deserialize error for textDecorationLine with invalid type", () => {
  expectInvalidType(
    () =>
      renderer.render(
        container({
          children: [],
          style: {
            // @ts-expect-error: invalid type test
            textDecorationLine: 123,
          },
        }),
        {
          width: 100,
          height: 100,
        },
      ),
    "textDecorationLine",
    "integer `123`",
    "a value of 'underline', 'line-through' or 'overline' or 'none'; also accepts 'initial', 'unset' or 'inherit'.",
  );
});

test("report deserialize error for textDecorationLine with invalid string value", () => {
  expectInvalidValue(
    () =>
      renderer.render(
        {
          type: "container",
          children: [],
          style: {
            textDecorationLine: "invalid",
          },
        },
        {
          width: 100,
          height: 100,
        },
      ),
    "textDecorationLine",
    "invalid",
    "invalid",
    "a value of 'underline', 'line-through' or 'overline' or 'none'; also accepts 'initial', 'unset' or 'inherit'.",
  );
});
