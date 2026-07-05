const DURATION_MS = 3000;

export default function AnimatedShowcase() {
  return (
    <div
      tw="flex h-full w-full items-center justify-center overflow-hidden bg-[#fff7ed]"
      style={{
        backgroundImage: "linear-gradient(180deg, #fff7ed 0%, #fee2e2 100%)",
      }}
    >
      <div
        tw="flex h-32 w-32 items-center justify-center rounded-2xl bg-amber-300 font-semibold text-xl text-orange-950"
        style={{
          transformOrigin: "center",
          animationName: "stretch-cube",
          animationDuration: `${DURATION_MS}ms`,
          animationTimingFunction: "cubic-bezier(0.65, 0, 0.35, 1)",
          animationIterationCount: "infinite",
        }}
      >
        Animated!
      </div>
    </div>
  );
}

export const options: PlaygroundOptions = {
  width: 640,
  height: 360,
  keyframes: {
    "stretch-cube": {
      "0%": { transform: "rotate(0deg) scale(1, 1)", borderRadius: "16px" },
      "25%": { transform: "rotate(-3deg) scale(1.08, 0.92)", borderRadius: "28px 18px 24px 14px" },
      "50%": { transform: "rotate(0deg) scale(0.94, 1.06)", borderRadius: "50%" },
      "75%": { transform: "rotate(3deg) scale(1.04, 0.96)", borderRadius: "14px 26px 18px 30px" },
      "100%": { transform: "rotate(0deg) scale(1, 1)", borderRadius: "16px" },
    },
  },
  animation: {
    durationMs: DURATION_MS,
    fps: 24,
    format: "webp",
  },
};
