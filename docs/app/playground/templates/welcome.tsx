export default function Welcome() {
  return (
    <div
      tw="flex w-full h-full flex-col justify-center bg-[#0a0a0a] items-center"
      style={{
        backgroundImage: "radial-gradient(circle at 50% 10%, #2a0a0a 0%, #0a0a0a 60%)",
      }}
    >
      <div tw="justify-center items-center flex flex-col text-white">
        <img src="https://takumi.kane.tw/logo.svg" tw="w-30 h-30 mb-8" />
        <h1 tw="font-extrabold text-8xl leading-none tracking-tighter mb-0 mt-0 flex items-center">
          Takumi <span tw="text-neutral-500 font-medium ml-6">Playground</span>
        </h1>
        <p tw="text-4xl text-white/75 font-semibold tracking-wide mt-10">
          Turn JSX into production-ready images fast. 🚀🗣️
        </p>
      </div>
    </div>
  );
}

const devicePixelRatio = 1.0;

export const options: PlaygroundOptions = {
  width: 1200 * devicePixelRatio,
  height: 630 * devicePixelRatio,
  format: "png",
  devicePixelRatio,
  emoji: "twemoji",
};
