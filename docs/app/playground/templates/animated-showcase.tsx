const DURATION_MS = 3000;

const words = [
  { text: "動", delay: 0 },
  { text: "く", delay: 200 },
  { text: "文", delay: 400 },
  { text: "字", delay: 600 },
];

export default function AnimatedShowcase() {
  return (
    <div
      tw="flex h-full w-full items-center justify-center"
      style={{ backgroundImage: "linear-gradient(180deg, #fff7ed 0%, #fee2e2 100%)" }}
    >
      {words.map((word) => (
        <span
          key={word.text}
          lang="ja"
          tw="mx-2 text-7xl font-bold text-orange-950"
          style={{
            animationName: "bob",
            animationDuration: `${DURATION_MS}ms`,
            animationDelay: `${word.delay}ms`,
            animationIterationCount: "infinite",
            animationFillMode: "backwards",
            animationTimingFunction: "cubic-bezier(0.65, 0, 0.35, 1)",
          }}
        >
          {word.text}
        </span>
      ))}
    </div>
  );
}

export const options: PlaygroundOptions = {
  width: 640,
  height: 360,
  css: {
    keyframes: "bob",
    steps: [
      { offset: "0%", style: { transform: "translateY(0) scale(1)", color: "#7c2d12" } },
      { offset: "35%", style: { transform: "translateY(-28px) scale(1.15)", color: "#ea580c" } },
      { offset: "70%", style: { transform: "translateY(0) scale(1)", color: "#7c2d12" } },
      { offset: "100%", style: { transform: "translateY(0) scale(1)", color: "#7c2d12" } },
    ],
  },
  animation: {
    durationMs: DURATION_MS,
    fps: 30,
    format: "webp",
  },
};
