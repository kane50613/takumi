export default function ArticleCover() {
  return (
    <div tw="flex h-full w-full flex-col justify-between bg-linear-to-br from-zinc-900 to-black p-14 text-zinc-100">
      <div tw="flex items-center">
        <span tw="rounded-full bg-indigo-500/15 px-5 py-2 text-xl font-bold tracking-widest text-indigo-300 uppercase">
          Engineering
        </span>
        <span tw="ml-6 text-2xl text-zinc-500">Oct 24, 2026 · 8 min read</span>
      </div>

      <h1 tw="m-0 text-7xl font-black leading-none tracking-tighter">
        Building a renderer <span tw="text-indigo-400">without a browser</span>
      </h1>

      <div tw="flex items-center">
        <img
          src="https://avatars.githubusercontent.com/u/10137?v=4"
          alt=""
          tw="h-16 w-16 rounded-xl"
        />
        <div tw="ml-5 flex flex-col">
          <span tw="text-lg text-zinc-400">Published by</span>
          <span tw="text-2xl font-bold">The Engineering Team</span>
        </div>
      </div>
    </div>
  );
}

export const options: PlaygroundOptions = {
  width: 1200,
  height: 630,
  format: "png",
};
