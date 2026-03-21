export default function Profile() {
  return (
    <div tw="flex h-full w-full bg-slate-900 border-t-8 border-t-blue-500 py-12 px-16 text-white text-sans">
      <div tw="flex flex-col h-full w-full">
        <div tw="flex w-full items-center mb-8">
          <img
            src="https://avatars.githubusercontent.com/u/1024025"
            alt="Linus Torvalds"
            tw="w-32 h-32 rounded-full border-4 border-slate-700 bg-slate-400"
          />
          <div tw="flex flex-col ml-8">
            <span tw="text-5xl font-bold text-white">Linus Torvalds</span>
            <span tw="text-3xl text-slate-400 mt-2">@torvalds</span>
          </div>
        </div>

        <div tw="flex max-w-[800px] text-4xl leading-tight text-slate-200 mt-4">
          Creator of Linux and Git. Passionate about operating systems, open source, and making
          computers do what they're told.
        </div>

        <div tw="flex mt-auto text-3xl font-medium w-full items-center">
          <div tw="flex items-center text-slate-400 mr-12">
            <strong tw="text-white font-bold mr-3">125K</strong> Following
          </div>
          <div tw="flex items-center text-slate-400">
            <strong tw="text-white font-bold mr-3">8.2M</strong> Followers
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
