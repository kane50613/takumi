export interface CodeDemoProps {
  highlightedHtml: string;
}

export function CodeDemo({ highlightedHtml }: CodeDemoProps) {
  return (
    <div className="grid grid-cols-1 lg:grid-cols-[1fr_auto_1fr] gap-6 items-center">
      <div className="border border-border rounded-2xl overflow-hidden bg-background backdrop-blur-sm">
        <div className="flex items-center gap-3 px-4 py-3 border-b border-border bg-muted/30">
          <div className="flex gap-1.5">
            <span className="w-2.5 h-2.5 rounded-full bg-[#ff5f57]" />
            <span className="w-2.5 h-2.5 rounded-full bg-[#febc2e]" />
            <span className="w-2.5 h-2.5 rounded-full bg-[#28c840]" />
          </div>
          <span className="text-xs text-muted-foreground font-mono">route.tsx</span>
        </div>
        <div
          suppressHydrationWarning
          className="py-4 text-[0.8rem] leading-relaxed overflow-x-auto sm:overflow-x-hidden [&_pre]:bg-transparent! [&_pre]:m-0! [&_pre]:p-0! [&_code]:bg-transparent!"
          dangerouslySetInnerHTML={{ __html: highlightedHtml }}
        />
      </div>
      <div className="text-muted-foreground/40 max-lg:rotate-90 max-lg:justify-self-center">
        <svg width="48" height="48" viewBox="0 0 24 24" fill="none" aria-hidden="true">
          <path
            d="M5 12h14m-4-4l4 4-4 4"
            stroke="currentColor"
            strokeWidth="1.5"
            strokeLinecap="round"
            strokeLinejoin="round"
          />
        </svg>
      </div>

      <div className="border border-border rounded-2xl overflow-hidden bg-background backdrop-blur-sm">
        <div className="flex items-center gap-3 px-4 py-3 border-b border-border bg-muted/30">
          <span className="text-xs text-muted-foreground font-mono">output.png</span>
          <span className="ml-auto text-[0.65rem] px-2 py-0.5 rounded-full bg-primary/20 text-primary font-semibold">
            1200×630
          </span>
        </div>
        <div className="p-3">
          <img
            src="https://raw.githubusercontent.com/kane50613/takumi/refs/heads/master/example/twitter-images/output/og-image.png"
            alt="Takumi rendered OG output"
            className="w-full h-auto rounded-lg block"
            width={1200}
            height={630}
          />
        </div>
      </div>
    </div>
  );
}
