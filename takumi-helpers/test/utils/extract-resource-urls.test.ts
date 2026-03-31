import { describe, expect, test } from "bun:test";
import { container, image } from "../../src/helpers";
import { extractResourceUrls } from "../../src/utils";

describe("extractResourceUrls", () => {
  test("extracts remote image and style urls without duplicates", () => {
    const remoteImageUrl = "https://example.com/image.png";
    const backgroundUrl = "https://example.com/background.png";
    const maskUrl = "https://example.com/mask.png";
    const node = container({
      children: [
        image({
          src: remoteImageUrl,
          width: 100,
          height: 100,
        }),
        image({
          src: "data:image/svg+xml,<svg xmlns='http://www.w3.org/2000/svg' />",
          width: 100,
          height: 100,
        }),
        container({
          style: {
            backgroundImage: `linear-gradient(#fff, #000), url("${backgroundUrl}")`,
            maskImage: `url(${maskUrl}) url(${backgroundUrl})`,
          },
        }),
      ],
    });

    expect(extractResourceUrls(node)).toEqual([remoteImageUrl, backgroundUrl, maskUrl]);
  });

  test("ignores malformed css url values", () => {
    const node = container({
      style: {
        backgroundImage: "url(https://example.com/good.png) url(",
      },
    });

    expect(extractResourceUrls(node)).toEqual(["https://example.com/good.png"]);
  });
});
