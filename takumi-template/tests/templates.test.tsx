import { test } from "bun:test";
import { join } from "node:path";
import { Renderer } from "@takumi-rs/core";
import { fromJsx } from "@takumi-rs/helpers/jsx";
import { write } from "bun";
import type { ReactNode } from "react";
import BlogPostTemplate from "../src/templates/blog-post-template";
import DocsTemplate from "../src/templates/docs-template";
import ProductCardTemplate from "../src/templates/product-card-template";

const renderer = new Renderer({
  persistentImages: [
    {
      src: "takumi.svg",
      data: await Bun.file(
        join(import.meta.dirname, "..", "..", "assets", "images", "takumi.svg"),
      ).arrayBuffer(),
    },
  ],
});

function testRender(name: string, template: ReactNode) {
  test(name, async () => {
    const node = await fromJsx(template);
    const start = performance.now();

    const buffer = await renderer.render(node, {
      width: 1200,
      height: 630,
      format: "webp",
    });

    const end = performance.now();

    console.log(`Rendered in ${Math.round(end - start)}ms`);

    await write(
      join(import.meta.dirname, "output", `${name}.webp`),
      buffer.buffer,
    );
  });
}

testRender(
  "docs-template",
  <DocsTemplate
    title="Fumadocs Integration"
    description="When will Fuma meet me in person? Hope we can meet in Japan! Culpa dolore eu ullamco aute exercitation sint aute nostrud qui tempor commodo ad culpa culpa. Laborum laboris eu laborum Lorem aliquip nulla nulla est proident eu. Officia deserunt aute ex quis exercitation ut. Irure cupidatat eu dolor Lorem eu aliquip mollit voluptate esse aute fugiat officia proident aliquip."
    icon={<img alt="Takumi" src="takumi.svg" tw="w-16 h-16" />}
    primaryColor="hsla(354, 90%, 54%, 0.3)"
    primaryTextColor="hsl(354, 90%, 60%)"
    site="Takumi"
  />,
);

testRender(
  "blog-post-template",
  <BlogPostTemplate
    title="The Future of Web Rendering with Rust and WebAssembly"
    author="Kane Wang"
    date="Nov 24, 2025"
    category="Engineering"
    avatar={
      <img
        alt="Avatar"
        src="https://avatars.githubusercontent.com/u/33802653?s=400&u=265f123fc40f34df69466e0a4368f64cc8837e2f&v=4"
        tw="w-full h-full object-cover rounded-full"
      />
    }
  />,
);

testRender(
  "product-card-template",
  <ProductCardTemplate
    productName="Takumi Pro"
    price="$299"
    description="The ultimate image generation engine for your next project. Blazing fast, type-safe, and built for scale."
    brand="Takumi"
    image={
      <img
        alt="Product"
        src="takumi.svg"
        style={{ width: "200px", height: "200px", objectFit: "contain" }}
      />
    }
  />,
);
