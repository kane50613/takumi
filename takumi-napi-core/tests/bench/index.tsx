import { writeFile } from "node:fs/promises";
import { fromJsx } from "@takumi-rs/helpers/jsx";
import { Globe2 } from "lucide-react";
import { bench, run, summary } from "mitata";
import DocsTemplate from "../../../takumi-template/src/templates/docs-template";
import { Renderer } from "../../index.js";

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

async function createAnimationNodes() {
  const scenes = await Promise.all(
    [0, 0.33, 0.66, 1].map(async (progress) => {
      const { node } = await createNode(progress);
      return {
        node,
        durationMs: 250,
      };
    }),
  );

  return {
    scenes,
    fps: 30,
    durationMs: 1000,
  };
}

const renderer = new Renderer();

bench("createNode", createNode);

summary(() => {
  bench("createNode + render (raw)", async () => {
    const { node, stylesheets } = await createNode();
    return renderer.render(node, {
      width: 1200,
      height: 630,
      format: "raw",
      stylesheets,
    });
  });

  bench("createNode + render (png, fdeflate)", async () => {
    const { node, stylesheets } = await createNode();
    return renderer.render(node, {
      width: 1200,
      height: 630,
      quality: 75,
      stylesheets,
    });
  });

  bench("createNode + render (png, flate2)", async () => {
    const { node, stylesheets } = await createNode();
    return renderer.render(node, {
      width: 1200,
      height: 630,
      quality: 100,
      stylesheets,
    });
  });

  bench("createNode + render (webp 75%)", async () => {
    const { node, stylesheets } = await createNode();
    return renderer.render(node, {
      width: 1200,
      height: 630,
      format: "webp",
      quality: 75,
      stylesheets,
    });
  });

  bench("createNode + render (webp 100%)", async () => {
    const { node, stylesheets } = await createNode();
    return renderer.render(node, {
      width: 1200,
      height: 630,
      format: "webp",
      quality: 100,
      stylesheets,
    });
  });
});

summary(() => {
  bench("createNode + renderAnimation (webp, 30fps, 75%, 1000ms)", async () => {
    const { scenes, fps, durationMs } = await createAnimationNodes();

    if (fps !== 30 || durationMs !== 1000) {
      throw new Error("Invalid fps or durationMs");
    }

    return renderer.renderAnimation(scenes, {
      width: 1200,
      height: 630,
      fps,
      format: "webp",
      quality: 75,
    });
  });

  bench(
    "createNode + renderAnimation (webp, 30fps, 100%, 1000ms)",
    async () => {
      const { scenes, fps, durationMs } = await createAnimationNodes();

      if (fps !== 30 || durationMs !== 1000) {
        throw new Error("Invalid fps or durationMs");
      }

      return renderer.renderAnimation(scenes, {
        width: 1200,
        height: 630,
        fps,
        format: "webp",
        quality: 100,
      });
    },
  );

  bench("createNode + renderAnimation (apng, 30fps, 1000ms)", async () => {
    const { scenes, fps, durationMs } = await createAnimationNodes();

    if (fps !== 30 || durationMs !== 1000) {
      throw new Error("Invalid fps or durationMs");
    }

    return renderer.renderAnimation(scenes, {
      width: 1200,
      height: 630,
      fps,
      format: "apng",
    });
  });

  bench("createNode + renderAnimation (gif, 30fps, 1000ms)", async () => {
    const { scenes, fps, durationMs } = await createAnimationNodes();

    if (fps !== 30 || durationMs !== 1000) {
      throw new Error("Invalid fps or durationMs");
    }

    return renderer.renderAnimation(scenes, {
      width: 1200,
      height: 630,
      fps,
      format: "gif",
    });
  });
});

summary(() => {
  bench("createNode + encodeFrames (webp, 30fps, 75%, 1000ms)", async () => {
    const fps = 30;
    const durationMs = 1000;
    const totalFrames = (durationMs * fps) / 1000;
    const frames = await Promise.all(
      Array.from({ length: totalFrames }, async (_frame, frameIndex) => {
        const normalizedProgress =
          totalFrames > 1 ? frameIndex / (totalFrames - 1) : 0;
        const { node } = await createNode(normalizedProgress);
        return {
          node,
          durationMs: durationMs / totalFrames,
        };
      }),
    );

    return renderer.encodeFrames(frames, {
      width: 1200,
      height: 630,
      format: "webp",
      quality: 75,
    });
  });
});

const { node, stylesheets } = await createNode();

await writeFile(
  "tests/bench/bench.png",
  await renderer.render(node, {
    width: 1200,
    height: 630,
    stylesheets,
  }),
);

await run();
