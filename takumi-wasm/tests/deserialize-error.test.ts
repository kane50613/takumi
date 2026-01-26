import { expect, test } from "bun:test";
import { Renderer } from "../bundlers/node";

const renderer = new Renderer();

test("report deserialize error for justifyContent with wrong type", () => {
  expect(() =>
    renderer.render(
      {
        type: "container",
        children: [],
        style: {
          justifyContent: 123,
        },
      },
      {
        width: 100,
        height: 100,
      },
    ),
  ).toThrowError(
    "invalid type: integer `123`, expected a value of 'normal', 'start', 'end', 'flex-start', 'flex-end', 'center', 'stretch', 'space-between', 'space-around' or 'space-evenly'; also accepts 'initial' or 'inherit'.",
  );
});

test("report deserialize error for justifyContent with invalid string value", () => {
  expect(() =>
    renderer.render(
      {
        type: "container",
        children: [],
        style: {
          justifyContent: "star",
        },
      },
      {
        width: 100,
        height: 100,
      },
    ),
  ).toThrowError(
    "invalid value: string \"star\", expected a value of 'normal', 'start', 'end', 'flex-start', 'flex-end', 'center', 'stretch', 'space-between', 'space-around' or 'space-evenly'; also accepts 'initial' or 'inherit'.",
  );
});

test("report deserialize error for color property with invalid type", () => {
  expect(() =>
    renderer.render(
      {
        type: "container",
        children: [],
        style: {
          color: 123,
        },
      },
      {
        width: 100,
        height: 100,
      },
    ),
  ).toThrowError(
    "invalid type: integer `123`, expected a value of 'currentColor' or <color>; also accepts 'initial' or 'inherit'.",
  );
});

test("report deserialize error for color property with invalid string value", () => {
  expect(() =>
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
  ).toThrowError(
    "invalid value: string \"notacolor\", expected a value of 'currentColor' or <color>; also accepts 'initial' or 'inherit'.",
  );
});

test("report deserialize error for width property with invalid type", () => {
  expect(() =>
    renderer.render(
      {
        type: "container",
        children: [],
        style: {
          width: true,
        },
      },
      {
        width: 100,
        height: 100,
      },
    ),
  ).toThrowError(
    "invalid type: boolean `true`, expected a value of <length>; also accepts 'initial' or 'inherit'.",
  );
});

test("report deserialize error for width property with invalid string value", () => {
  expect(() =>
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
  ).toThrowError(
    "invalid value: string \"invalid\", expected a value of <length>; also accepts 'initial' or 'inherit'.",
  );
});

test("report deserialize error for alignItems property with invalid type", () => {
  expect(() =>
    renderer.render(
      {
        type: "container",
        children: [],
        style: {
          alignItems: [],
        },
      },
      {
        width: 100,
        height: 100,
      },
    ),
  ).toThrowError(
    "invalid type: sequence, expected a value of 'normal', 'start', 'end', 'flex-start', 'flex-end', 'center', 'baseline' or 'stretch'; also accepts 'initial' or 'inherit'.",
  );
});

test("report deserialize error for alignItems property with invalid string value", () => {
  expect(() =>
    renderer.render(
      {
        type: "container",
        children: [],
        style: {
          alignItems: "invalid",
        },
      },
      {
        width: 100,
        height: 100,
      },
    ),
  ).toThrowError(
    "invalid value: string \"invalid\", expected a value of 'normal', 'start', 'end', 'flex-start', 'flex-end', 'center', 'baseline' or 'stretch'; also accepts 'initial' or 'inherit'.",
  );
});

test("report deserialize error for borderRadius property with invalid type", () => {
  expect(() =>
    renderer.render(
      {
        type: "container",
        children: [],
        style: {
          borderRadius: true,
        },
      },
      {
        width: 100,
        height: 100,
      },
    ),
  ).toThrowError(
    "invalid type: boolean `true`, expected 1 to 4 length values for width, optionally followed by '/' and 1 to 4 length values for height; also accepts 'initial' or 'inherit'.",
  );
});

test("report deserialize error for borderRadius property with invalid string value", () => {
  expect(() =>
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
  ).toThrowError(
    "invalid value: string \"invalid\", expected 1 to 4 length values for width, optionally followed by '/' and 1 to 4 length values for height; also accepts 'initial' or 'inherit'.",
  );
});

test("report deserialize error for borderRadius property with invalid slash syntax", () => {
  expect(() =>
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
  ).toThrowError(
    "invalid value: string \"10px / invalid\", expected 1 to 4 length values for width, optionally followed by '/' and 1 to 4 length values for height; also accepts 'initial' or 'inherit'.",
  );
});

test("report deserialize error for padding (Sides) with invalid type", () => {
  expect(() =>
    renderer.render(
      {
        type: "container",
        children: [],
        style: {
          padding: { top: null },
        },
      },
      {
        width: 100,
        height: 100,
      },
    ),
  ).toThrowError(
    "invalid type: map, expected 1 ~ 4 values of <length>; also accepts 'initial' or 'inherit'.",
  );
});

test("report deserialize error for padding (Sides) with invalid string value", () => {
  expect(() =>
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
  ).toThrowError(
    "invalid value: string \"invalid\", expected 1 ~ 4 values of <length>; also accepts 'initial' or 'inherit'.",
  );
});

test("report deserialize error for gap (SpacePair) with invalid type", () => {
  expect(() =>
    renderer.render(
      {
        type: "container",
        children: [],
        style: {
          gap: true,
        },
      },
      {
        width: 100,
        height: 100,
      },
    ),
  ).toThrowError(
    "invalid type: boolean `true`, expected 1 ~ 2 values of <length>; also accepts 'initial' or 'inherit'.",
  );
});

test("report deserialize error for gap (SpacePair) with invalid string value", () => {
  expect(() =>
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
  ).toThrowError(
    "invalid value: string \"invalid\", expected 1 ~ 2 values of <length>; also accepts 'initial' or 'inherit'.",
  );
});

// Tests fallback error messages when neither value_description() nor enum_values() is implemented
test("report deserialize error for textDecorationLine with invalid type", () => {
  expect(() =>
    renderer.render(
      {
        type: "container",
        children: [],
        style: {
          textDecorationLine: 123,
        },
      },
      {
        width: 100,
        height: 100,
      },
    ),
  ).toThrowError(
    "invalid type: integer `123`, expected a value of 'underline', 'line-through' or 'overline'; also accepts 'none', 'initial' or 'inherit'.",
  );
});

test("report deserialize error for textDecorationLine with invalid string value", () => {
  expect(() =>
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
  ).toThrowError(
    "invalid value: string \"invalid\", expected a value of 'underline', 'line-through' or 'overline'; also accepts 'none', 'initial' or 'inherit'.",
  );
});
