export default function ArticleCover() {
  return (
    <div tw="flex h-full w-full flex-col justify-between bg-linear-to-br from-night-900 to-night-950 p-14 text-night-100">
      <div tw="flex items-center">
        <span tw="rounded-full bg-brand-500/15 px-5 py-2 text-xl font-bold tracking-widest text-brand-300 uppercase">
          Engineering
        </span>
        <span tw="ml-6 text-2xl text-night-500">Oct 24, 2026 · 8 min read</span>
      </div>

      <h1 tw="m-0 text-7xl font-black leading-none tracking-tighter">
        Building a renderer <span tw="text-brand-400">without a browser</span>
      </h1>

      <div tw="flex items-center">
        <img
          src="https://avatars.githubusercontent.com/u/10137?v=4"
          alt=""
          tw="h-16 w-16 rounded-xl border-2 border-brand-500/40"
        />
        <div tw="ml-5 flex flex-col">
          <span tw="text-lg text-night-500">Published by</span>
          <span tw="text-2xl font-bold">The Engineering Team</span>
        </div>
      </div>
    </div>
  );
}

// Swap this block for a teal or amber scale and the whole cover re-skins.
export const options: PlaygroundOptions = {
  width: 1200,
  height: 630,
  format: "png",
  cssVariables: {
    "--color-brand-300": "#c4b5fd",
    "--color-brand-400": "#a78bfa",
    "--color-brand-500": "#7c3aed",
    "--color-night-100": "#f4f4f5",
    "--color-night-500": "#71717b",
    "--color-night-900": "#1c1825",
    "--color-night-950": "#0e0c14",
  },
};
