import { describe, expect, it } from "bun:test";
import { Renderer } from "../index.js";

describe("Renderer.measure", () => {
  const renderer = new Renderer();

  it("should measure a simple container", async () => {
    const node = {
      type: "container",
      style: {
        width: 100,
        height: 100,
        backgroundColor: "red",
      },
      children: [],
    };

    const result = await renderer.measure(node);

    expect(result).toEqual({
      width: 100,
      height: 100,
      transform: [1, 0, 0, 1, 0, 0],
      children: [],
      runs: [],
    });
  });

  it("should measure nested children with layout", async () => {
    const node = {
      type: "container",
      style: {
        display: "flex",
        width: 200,
        height: 200,
        padding: 10,
      },
      children: [
        {
          type: "text",
          text: "Hello",
          style: {
            width: 50,
            height: 50,
          },
        },
        {
          type: "container",
          style: {
            flex: 1,
            height: 50,
          },
        },
      ],
    };

    const result = await renderer.measure(node);

    expect(result).toEqual({
      width: 200,
      height: 200,
      transform: [1, 0, 0, 1, 0, 0],
      children: [
        {
          width: 50,
          height: 50,
          transform: [1, 0, 0, 1, 10, 10],
          children: [],
          runs: [],
        },
        {
          width: 130,
          height: 50,
          transform: [1, 0, 0, 1, 60, 10],
          children: [],
          runs: [],
        },
      ],
      runs: [],
    });
  });

  it("should include transforms", async () => {
    const node = {
      type: "container",
      style: {
        width: 100,
        height: 100,
        transform: "translateX(50px) scale(2)",
        transformOrigin: "0 0",
      },
    };

    const result = await renderer.measure(node);

    expect(result).toEqual({
      width: 100,
      height: 100,
      transform: [2, 0, 0, 2, 50, 0],
      children: [],
      runs: [],
    });
  });
});
