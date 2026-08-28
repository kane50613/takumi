export default function Poster() {
  return (
    <div
      tw="flex h-full w-full items-center justify-center"
      style={{
        backgroundImage:
          "conic-gradient(from 210deg, var(--color-dawn), var(--color-dusk), var(--color-noon), var(--color-dawn))",
      }}
    >
      <div
        tw="flex h-[560px] w-[560px] flex-col items-center justify-center rounded-full bg-white/10 text-white"
        style={{ backdropFilter: "blur(12px)" }}
      >
        <span tw="text-[140px] leading-none">🌗</span>
        <h1 tw="m-0 mt-6 text-7xl font-black tracking-tighter" style={{ mixBlendMode: "overlay" }}>
          Half light
        </h1>
        <p tw="mt-4 mb-0 text-3xl text-white/80">Conic gradient · blur · blend mode</p>
      </div>
    </div>
  );
}

export const options: PlaygroundOptions = {
  width: 1080,
  height: 1080,
  format: "png",
  css: [
    {
      selector: ":root",
      style: {
        "--color-dawn": "#ff6b6b",
        "--color-dusk": "#556270",
        "--color-noon": "#ffd93d",
      },
    },
  ],
};
