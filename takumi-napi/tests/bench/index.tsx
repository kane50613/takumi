import { writeFile } from "node:fs/promises";
import { fromJsx } from "@takumi-rs/helpers/jsx";
import { Globe2 } from "lucide-react";
import { bench, run, summary } from "mitata";
import DocsTemplate from "../../../docs/app/registry/image/docs";
import { Renderer, type RenderOptions } from "../../src/export";

function createNode(progress = 0) {
  const orbitOffsetX = Math.sin(progress * Math.PI * 2) * 18;
  const orbitOffsetY = Math.cos(progress * Math.PI * 2) * 14;
  const globeRotation = progress * 360;
  const globeScale = 1 + Math.sin(progress * Math.PI * 4) * 0.12;

  return fromJsx(
    <DocsTemplate
      title="Takumi Benchmark"
      description="See how Takumi performs in real world use cases!"
      site="takumi.kane.tw"
      icon={
        <div
          style={{
            width: 72,
            height: 72,
            position: "relative",
            transform: `translate(${orbitOffsetX}px, ${orbitOffsetY}px)`,
          }}
        >
          <div
            style={{
              position: "absolute",
              inset: 0,
              borderRadius: "50%",
              background:
                "radial-gradient(circle at 30% 30%, rgba(125, 211, 252, 0.65), rgba(59, 130, 246, 0.15) 70%, transparent 100%)",
              filter: "blur(1px)",
            }}
          />
          <div
            style={{
              position: "absolute",
              inset: 4,
              display: "grid",
              placeItems: "center",
              transform: `rotate(${globeRotation}deg) scale(${globeScale})`,
            }}
          >
            <Globe2 size={64} color="white" />
          </div>
        </div>
      }
      primaryColor="blue"
      primaryTextColor="white"
    />,
  );
}

function createScenes() {
  return Promise.all(
    [0, 0.33, 0.66, 1].map(async (progress) => {
      const { node } = await createNode(progress);

      return { node, durationMs: 250 };
    }),
  );
}

const renderer = new Renderer();

async function renderStill(options: RenderOptions) {
  const { node, css } = await createNode();

  return renderer.render(node, { ...options, css });
}

bench("createNode", createNode);

summary(() => {
  bench("createNode + render (raw)", () =>
    renderStill({
      width: 1200,
      height: 630,
      format: "raw",
    }));

  bench("createNode + render (png, fdeflate)", () =>
    renderStill({
      width: 1200,
      height: 630,
      quality: 75,
    }));

  bench("createNode + render (png, flate2)", () =>
    renderStill({
      width: 1200,
      height: 630,
      quality: 100,
    }));

  bench("createNode + render (webp 75%)", () =>
    renderStill({
      width: 1200,
      height: 630,
      format: "webp",
      quality: 75,
    }));

  bench("createNode + render (webp 100%)", () =>
    renderStill({
      width: 1200,
      height: 630,
      format: "webp",
      quality: 100,
    }));
});

summary(() => {
  bench("createNode + renderAnimation (webp, 30fps, 75%, 1000ms)", async () =>
    renderer.renderAnimation({
      scenes: await createScenes(),
      width: 1200,
      height: 630,
      fps: 30,
      format: "webp",
      quality: 75,
    }));

  bench("createNode + renderAnimation (webp, 30fps, 100%, 1000ms)", async () =>
    renderer.renderAnimation({
      scenes: await createScenes(),
      width: 1200,
      height: 630,
      fps: 30,
      format: "webp",
      quality: 100,
    }));

  bench("createNode + renderAnimation (apng, 30fps, 1000ms)", async () =>
    renderer.renderAnimation({
      scenes: await createScenes(),
      width: 1200,
      height: 630,
      fps: 30,
      format: "apng",
    }));

  bench("createNode + renderAnimation (gif, 30fps, 1000ms)", async () =>
    renderer.renderAnimation({
      scenes: await createScenes(),
      width: 1200,
      height: 630,
      fps: 30,
      format: "gif",
    }));
});

await writeFile("tests/bench/bench.png", await renderStill({ width: 1200, height: 630 }));

await run();
