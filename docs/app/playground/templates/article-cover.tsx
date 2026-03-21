export default function Article() {
  return (
    <div tw="flex h-full w-full  text-zinc-100 font-sans">
      <div tw="flex flex-col h-full w-full bg-linear-to-br from-zinc-900 to-black border border-zinc-800 p-12 shadow-2xl relative overflow-hidden">
        <div tw="absolute -top-24 -right-24 w-96 h-96 bg-indigo-500/10 rounded-full blur-3xl" />

        <div tw="flex items-center mb-12">
          <div tw="flex items-center justify-center bg-indigo-500/10 px-5 py-2 rounded-full border border-indigo-500/20">
            <span tw="text-indigo-400 text-xl font-bold uppercase tracking-widest">
              Engineering
            </span>
          </div>
          <div tw="ml-6 w-1.5 h-1.5 rounded-full bg-zinc-700" />
          <span tw="ml-6 text-zinc-500 text-2xl font-medium">Oct 24, 2024</span>
        </div>

        <div tw="flex flex-col flex-1">
          <h1 tw="text-6xl font-black leading-none tracking-tighter mb-0 text-white block whitespace-pre text-wrap">
            <span>Building High-Performance </span>
            <span tw="text-indigo-400">Image Generation </span>
            <span>with Rust</span>
          </h1>
          <p tw="text-3xl text-zinc-400 leading-relaxed text-pretty">
            A deep dive into the architecture and optimizations that make Takumi the fastest image
            generation engine.
          </p>
        </div>

        <div tw="flex items-center mt-auto">
          <div tw="flex items-center bg-white/5 border border-white/10 rounded-2xl p-4 pr-8 backdrop-blur-sm">
            <img
              src="https://avatars.githubusercontent.com/u/10137?v=4"
              alt="Engineering Team"
              tw="w-16 h-16 rounded-xl shadow-lg"
            />
            <div tw="flex flex-col ml-5">
              <span tw="text-zinc-400 text-lg font-medium">Published by</span>
              <span tw="text-zinc-100 text-2xl font-bold">The Engineering Team</span>
            </div>
          </div>
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
