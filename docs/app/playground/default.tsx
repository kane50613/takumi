export default function Image() {
  return (
    <div
      tw="w-full h-full justify-center items-center"
      style={{
        backgroundImage: "url(https://picsum.photos/1200/630)",
      }}
    >
      <div tw="p-8 bg-white/50 justify-center items-center flex flex-col">
        <h1 tw="mt-0 font-medium text-7xl block whitespace-pre">
          Welcome to <span tw="text-[#ff3535]">Takumi </span>
          Playground!
        </h1>
        <span tw="text-black/60 text-4xl mb-0">
          You can try out and experiment with Takumi here.
        </span>
      </div>
    </div>
  );
}

export const options: PlaygroundOptions = {
  width: 1200,
  height: 630,
  format: "png",
};
