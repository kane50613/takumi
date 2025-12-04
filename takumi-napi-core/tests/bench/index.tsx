import { fromJsx } from "@takumi-rs/helpers/jsx";
import DocsTemplateV1 from "@takumi-rs/template/docs-template-v1";
import { write } from "bun";
import { Globe2 } from "lucide-react";
import { bench, run, summary } from "mitata";
import { Renderer } from "../../index.js";

function createNode() {
  return fromJsx(
    <DocsTemplateV1
      title="Takumi Benchmark"
      description="See how Takumi performs in real world use cases!"
      site="takumi.kane.tw"
      icon={<Globe2 size={64} color="white" />}
      primaryColor="blue"
      primaryTextColor="white"
    />,
  );
}

async function createAnimationNodes() {
  const fps = 30;
  const durationMs = 1000;
  const totalFrames = (durationMs * fps) / 1000;

  const frames = await Array.fromAsync({ length: totalFrames }, async () => {
    const node = await createNode();
    return {
      node,
      durationMs: durationMs / totalFrames,
    };
  });

  return {
    frames,
    fps,
    durationMs,
  };
}

const renderer = new Renderer();

bench("createNode", createNode);

summary(() => {
  bench("createNode + render (raw)", async () => {
    const node = await createNode();
    return renderer.render(node, {
      width: 1200,
      height: 630,
      format: "raw",
    });
  });

  bench("createNode + render (png)", async () => {
    const node = await createNode();
    return renderer.render(node, {
      width: 1200,
      height: 630,
    });
  });

  bench("createNode + render (webp)", async () => {
    const node = await createNode();
    return renderer.render(node, {
      width: 1200,
      height: 630,
      format: "webp",
    });
  });
});

summary(() => {
  bench("createNode + renderAnimation (webp, 30fps, 1000ms)", async () => {
    const { frames, fps, durationMs } = await createAnimationNodes();

    if (fps !== 30 || durationMs !== 1000) {
      throw new Error("Invalid fps or durationMs");
    }

    return renderer.renderAnimation(frames, {
      width: 1200,
      height: 630,
      format: "webp",
    });
  });

  bench("createNode + renderAnimation (apng, 30fps, 1000ms)", async () => {
    const { frames, fps, durationMs } = await createAnimationNodes();

    if (fps !== 30 || durationMs !== 1000) {
      throw new Error("Invalid fps or durationMs");
    }

    return renderer.renderAnimation(frames, {
      width: 1200,
      height: 630,
      format: "apng",
    });
  });
});

await write(
  "tests/bench/bench.png",
  await renderer
    .render(await createNode(), {
      width: 1200,
      height: 630,
    })
    .then((r) => r.buffer),
);

await run();
