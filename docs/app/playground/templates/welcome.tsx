export default function Welcome() {
  return (
    <div tw="flex h-full w-full flex-col justify-end bg-[#16130f] p-20">
      <h1 tw="m-0 text-8xl font-bold leading-none tracking-tighter text-white">Edit the code.</h1>
      <h1 tw="m-0 mt-2 text-8xl font-bold leading-none tracking-tighter text-[#ff4d4d]">
        Press Run.
      </h1>
      <p tw="mt-10 mb-0 text-3xl text-[#a8a29a]">
        The exported options decide what comes out: an image, an animation, or a PDF.
      </p>
    </div>
  );
}

export const options: PlaygroundOptions = {
  width: 1200,
  height: 630,
  format: "png",
};
