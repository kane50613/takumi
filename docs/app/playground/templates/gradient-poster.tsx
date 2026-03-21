export default function Poster() {
  return (
    <div
      tw="flex h-full w-full items-center justify-center"
      style={{
        backgroundImage: "linear-gradient(135deg, #FF6B6B 0%, #556270 100%)",
      }}
    >
      <div tw="flex flex-col items-center shadow-2xl bg-white/10 rounded-[64px] border border-white/20 p-24 backdrop-blur-md">
        <div tw="text-[120px] mb-8 filter drop-shadow-lg leading-none">✨</div>
        <h1 tw="text-8xl font-black text-white text-center tracking-tighter filter drop-shadow-md">
          Create Magic
        </h1>
        <p tw="mt-6 text-4xl text-white/80 font-medium tracking-wide">Unleash your creativity</p>
      </div>
    </div>
  );
}

export const options = {
  width: 1080,
  height: 1080,
  format: "png",
};
