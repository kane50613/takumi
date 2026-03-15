import { Box, Code, Layers, LayoutTemplate, Play, Zap } from "lucide-react";

export type CssLibraryCardProps = {
  libraryName: string;
  title: string;
  description: string;
};

export function TailwindCard({
  libraryName,
  title,
  description,
}: CssLibraryCardProps) {
  return (
    <div className="flex h-[630px] w-[1200px] bg-[#09090b] font-sans flex-row overflow-hidden border border-[#27272a]">
      {/* Left Pane - Content & Typo */}
      <div className="flex w-[640px] h-full flex-col p-20 justify-between bg-linear-to-b from-[#09090b] to-[#18181b] relative z-10">
        <div className="flex flex-col">
          <div className="flex items-center px-4 py-2 w-max shrink-0 rounded-full bg-blue-500/10 border border-blue-500/20">
            <Layers className="w-4 h-4 shrink-0 text-blue-400 mr-2" />
            <span className="text-sm font-bold text-blue-400 tracking-widest uppercase">
              {libraryName}
            </span>
          </div>

          <h1 className="m-0 mt-12 text-[64px] font-black text-white tracking-tighter leading-[1.05]">
            {title}
          </h1>
          <p className="m-0 mt-8 text-2xl text-[#a1a1aa] font-medium leading-relaxed">
            {description}
          </p>
        </div>

        <div className="flex items-center bg-blue-600 w-max shrink-0 px-8 py-5 rounded-2xl shadow-[0_0_40px_-10px_rgba(37,99,235,0.5)]">
          <span className="text-xl font-bold text-white tracking-wide">
            INITIALIZE PIPELINE
          </span>
          <Play className="w-5 h-5 shrink-0 text-white ml-3 fill-white" />
        </div>
      </div>

      {/* Right Pane - Feature Grid */}
      <div className="flex flex-1 h-full bg-[#18181b] flex-col p-16 justify-center border-l border-[#27272a] shadow-[-20px_0_60px_-15px_rgba(0,0,0,0.5)]">
        <div className="flex flex-col gap-8">
          <div className="flex bg-[#09090b] p-8 rounded-3xl border border-[#27272a] items-start hover:border-blue-500/50 transition-colors">
            <div className="flex h-16 w-16 shrink-0 items-center justify-center rounded-2xl bg-[#18181b] border border-[#27272a] text-blue-500">
              <Code className="w-8 h-8 shrink-0" />
            </div>
            <div className="flex flex-col ml-8 justify-center">
              <h3 className="m-0 text-2xl font-black text-[#f4f4f5]">
                Static Resolution
              </h3>
              <p className="m-0 mt-2 text-lg text-[#71717a] font-medium leading-normal">
                Optimized AST-based scanning of JSX source files for utility
                tokens.
              </p>
            </div>
          </div>

          <div className="flex bg-[#09090b] p-8 rounded-3xl border border-[#27272a] items-start hover:border-blue-500/50 transition-colors">
            <div className="flex h-16 w-16 shrink-0 items-center justify-center rounded-2xl bg-[#18181b] border border-[#27272a] text-blue-500">
              <Zap className="w-8 h-8 shrink-0" />
            </div>
            <div className="flex flex-col ml-8 justify-center">
              <h3 className="m-0 text-2xl font-black text-[#f4f4f5]">
                Engine Build
              </h3>
              <p className="m-0 mt-2 text-lg text-[#71717a] font-medium leading-normal">
                Tailwind v4 JIT compilation into highly optimized raw CSS
                stylesheets.
              </p>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}

export function UnoCard({
  libraryName,
  title,
  description,
}: CssLibraryCardProps) {
  return (
    <div className="flex h-[630px] w-[1200px] bg-white font-sans flex-col overflow-hidden relative border border-zinc-200">
      {/* Massive Background Gradients */}
      <div className="absolute top-0 right-0 w-[800px] h-[800px] bg-linear-to-bl from-rose-500/20 via-fuchsia-500/20 to-transparent blur-3xl rounded-full translate-x-1/3 -translate-y-1/3" />
      <div className="absolute bottom-0 left-0 w-[600px] h-[600px] bg-linear-to-tr from-cyan-400/20 to-transparent blur-3xl rounded-full -translate-x-1/4 translate-y-1/4" />

      {/* Top Section */}
      <div className="flex w-full flex-row px-20 py-16 justify-between items-start relative z-10 shrink-0">
        <div className="flex flex-col max-w-[640px]">
          <div className="flex items-center px-4 py-1.5 w-max shrink-0 rounded-full bg-zinc-900 shadow-xl border border-zinc-800">
            <LayoutTemplate className="w-4 h-4 shrink-0 text-fuchsia-400 mr-2" />
            <span className="text-sm font-black text-white tracking-widest uppercase">
              {libraryName}
            </span>
          </div>

          <h1 className="m-0 mt-10 text-[72px] font-black text-zinc-900 tracking-tighter leading-[0.9]">
            {title}
          </h1>
          <p className="m-0 mt-8 text-[28px] text-zinc-500 font-medium leading-[1.3] max-w-[600px]">
            {description}
          </p>
        </div>

        <div className="flex w-32 h-32 shrink-0 bg-linear-to-tr from-rose-500 via-fuchsia-500 to-indigo-500 rounded-[32px] shadow-2xl rotate-6 flex-col items-center justify-center p-1 border-4 border-white">
          <div className="flex w-full h-full shrink-0 bg-black/10 rounded-[24px] justify-center items-center">
            <Zap className="w-12 h-12 shrink-0 text-white fill-white" />
          </div>
        </div>
      </div>

      {/* Bottom Section - Metric Cards */}
      <div className="flex w-full flex-row gap-8 px-20 pb-16 mt-auto relative z-10 flex-1 items-end">
        <div className="flex flex-1 bg-white/80 p-8 rounded-[40px] border border-white shadow-[0_30px_60px_-15px_rgba(0,0,0,0.05)] flex-col justify-end h-[240px]">
          <div className="flex h-16 w-16 shrink-0 bg-zinc-100 rounded-[20px] items-center justify-center mb-auto border border-zinc-200">
            <Zap className="w-8 h-8 shrink-0 text-rose-500" />
          </div>
          <h3 className="m-0 text-3xl font-black text-zinc-900 tracking-tight">
            Zero Runtime
          </h3>
          <p className="m-0 mt-3 text-lg text-zinc-500 font-medium leading-snug">
            Generated entirely on-demand with zero client-side overhead.
          </p>
        </div>

        <div className="flex flex-1 bg-white/80 p-8 rounded-[40px] border border-white shadow-[0_30px_60px_-15px_rgba(0,0,0,0.05)] flex-col justify-end h-[240px]">
          <div className="flex h-16 w-16 shrink-0 bg-zinc-100 rounded-[20px] items-center justify-center mb-auto border border-zinc-200">
            <Box className="w-8 h-8 shrink-0 text-fuchsia-500" />
          </div>
          <h3 className="m-0 text-3xl font-black text-zinc-900 tracking-tight">
            Extensible
          </h3>
          <p className="m-0 mt-3 text-lg text-zinc-500 font-medium leading-snug">
            Built on top of a comprehensive native engine plugin system.
          </p>
        </div>
      </div>
    </div>
  );
}
