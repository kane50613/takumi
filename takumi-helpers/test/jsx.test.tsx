import { describe, expect, test } from "bun:test";
import { em, percentage, rem, vh } from "../src/helpers";
import { fromJsx } from "../src/jsx";
import type { ContainerNode, ImageNode, TextNode } from "../src/types";

describe("fromJsx", () => {
  test("converts text to TextNode", async () => {
    const result = await fromJsx("Hello World");
    expect(result).toEqual({
      type: "text",
      text: "Hello World",
    } as TextNode);
  });

  test("converts number to TextNode", async () => {
    const result = await fromJsx(42);
    expect(result).toEqual({
      type: "text",
      text: "42",
    } as TextNode);
  });

  test("returns undefined for null/undefined", async () => {
    expect(await fromJsx(null)).toBeUndefined();
    expect(await fromJsx(undefined)).toBeUndefined();
  });

  test("converts simple div to ContainerNode", async () => {
    const result = await fromJsx(<div>Hello</div>);
    expect(result).toEqual({
      type: "container",
      children: [
        {
          type: "text",
          text: "Hello",
        },
      ],
    } as ContainerNode);
  });

  test("handles function components", async () => {
    const MyComponent = ({ name }: { name: string }) => <div>Hello {name}</div>;

    const result = await fromJsx(<MyComponent name="World" />);
    expect(result).toEqual({
      type: "container",
      children: [
        { type: "text", text: "Hello " },
        { type: "text", text: "World" },
      ],
    } as ContainerNode);
  });

  test("handles async function components", async () => {
    const AsyncComponent = async ({ name }: { name: string }) => (
      <div>Hello {name}</div>
    );

    const result = await fromJsx(<AsyncComponent name="Async" />);
    expect(result).toEqual({
      type: "container",
      children: [
        { type: "text", text: "Hello " },
        { type: "text", text: "Async" },
      ],
    } as ContainerNode);
  });

  test("handles fragments", async () => {
    const result = await fromJsx(
      <>
        <div>First</div>
        <div>Second</div>
      </>,
    );

    expect(result).toEqual([
      {
        type: "container",
        children: [{ type: "text", text: "First" }],
      },
      {
        type: "container",
        children: [{ type: "text", text: "Second" }],
      },
    ]);
  });

  test("handles arrays", async () => {
    const items = ["First", "Second", "Third"];
    const result = await fromJsx(
      <div>
        {items.map((item) => (
          <span key={item}>{item}</span>
        ))}
      </div>,
    );

    expect(result).toEqual({
      type: "container",
      children: [
        {
          type: "container",
          children: [{ type: "text", text: "First" }],
        },
        {
          type: "container",
          children: [{ type: "text", text: "Second" }],
        },
        {
          type: "container",
          children: [{ type: "text", text: "Third" }],
        },
      ],
    } as ContainerNode);
  });

  test("converts img elements to ImageNode", async () => {
    const result = await fromJsx(
      <img src="https://example.com/image.jpg" alt="Test" />,
    );
    expect(result).toEqual({
      type: "image",
      src: "https://example.com/image.jpg",
    } as ImageNode);
  });

  test("handles img without src as container", async () => {
    const result = await fromJsx(<img alt="No src" />);
    expect(result).toEqual({
      type: "container",
      children: undefined,
    } as ContainerNode);
  });

  test("handles deeply nested structures", async () => {
    const result = await fromJsx(
      <div>
        <h1>Title</h1>
        <div>
          <p>
            Paragraph with <strong>bold</strong> text
          </p>
          <ul>
            <li>Item 1</li>
            <li>Item 2</li>
          </ul>
        </div>
      </div>,
    );

    expect(result).toEqual({
      type: "container",
      children: [
        {
          type: "container",
          children: [{ type: "text", text: "Title" }],
        },
        {
          type: "container",
          children: [
            {
              type: "container",
              children: [
                { type: "text", text: "Paragraph with " },
                {
                  type: "container",
                  children: [{ type: "text", text: "bold" }],
                },
                { type: "text", text: " text" },
              ],
            },
            {
              type: "container",
              children: [
                {
                  type: "container",
                  children: [{ type: "text", text: "Item 1" }],
                },
                {
                  type: "container",
                  children: [{ type: "text", text: "Item 2" }],
                },
              ],
            },
          ],
        },
      ],
    } as ContainerNode);
  });

  test("handles promises", async () => {
    const promiseElement = Promise.resolve("Resolved text");
    const result = await fromJsx(promiseElement);
    expect(result).toEqual({
      type: "text",
      text: "Resolved text",
    } as TextNode);
  });

  test("handles style props with dimensions", async () => {
    const result = await fromJsx(
      <div style={{ width: 100, height: "50px", maxWidth: "100%" }}>Test</div>,
    );

    expect(result).toEqual({
      type: "container",
      width: 100,
      height: 50,
      max_width: percentage(100),
      children: [{ type: "text", text: "Test" }],
    });
  });

  test("handles style props with spacing", async () => {
    const result = await fromJsx(
      <div style={{ padding: "10px 20px", margin: 15 }}>Test</div>,
    );

    expect(result).toEqual({
      type: "container",
      padding: { top: 10, right: 20, bottom: 10, left: 20 },
      margin: { top: 15, right: 15, bottom: 15, left: 15 },
      children: [{ type: "text", text: "Test" }],
    });
  });

  test("handles style props with flex properties", async () => {
    const result = await fromJsx(
      <div
        style={{
          flexDirection: "column",
          justifyContent: "center",
          alignItems: "flex-start",
          flexGrow: 1,
          gap: "8px",
        }}
      >
        Test
      </div>,
    );

    expect(result).toEqual({
      type: "container",
      flex_direction: "column",
      justify_content: "center",
      align_items: "flex-start",
      flex_grow: 1,
      gap: 8,
      children: [{ type: "text", text: "Test" }],
    });
  });

  test("handles style props with text properties", async () => {
    const result = await fromJsx(
      <div
        style={{
          fontSize: "16px",
          fontFamily: "Arial",
          fontWeight: "bold",
          lineHeight: 1.5,
          textAlign: "center",
        }}
      >
        Test
      </div>,
    );

    expect(result).toEqual({
      type: "container",
      font_size: 16,
      font_family: "Arial",
      font_weight: "bold",
      line_height: 1.5,
      text_align: "center",
      children: [{ type: "text", text: "Test" }],
    });
  });

  test("handles different length units", async () => {
    const result = await fromJsx(
      <div
        style={{
          width: "50%",
          height: "2rem",
          padding: "1em",
          margin: "10vh",
        }}
      >
        Test
      </div>,
    );

    expect(result).toEqual({
      type: "container",
      width: percentage(50),
      height: rem(2),
      padding: { top: em(1), right: em(1), bottom: em(1), left: em(1) },
      margin: { top: vh(10), right: vh(10), bottom: vh(10), left: vh(10) },
      children: [{ type: "text", text: "Test" }],
    });
  });

  test("handles img with style props", async () => {
    const result = await fromJsx(
      <img
        src="https://example.com/image.jpg"
        alt="Test content"
        style={{ width: 200, height: 150 }}
      />,
    );

    expect(result).toEqual({
      type: "image",
      src: "https://example.com/image.jpg",
      width: 200,
      height: 150,
    });
  });
});
