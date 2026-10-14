/** The homepage's text styles. Reading text stays at 18px or larger, and values at 16px. */
export const DISPLAY =
  "font-display font-[860] [font-stretch:112%] text-[clamp(1.875rem,3vw,3rem)] leading-[1.05] tracking-[-0.015em] text-balance";
/** The strike's word set as a hollow outline behind the forge, as wide as the page allows. */
export const BACKDROP_WORD =
  "font-display font-[900] [font-stretch:125%] text-[min(16vw,34svh)] leading-[1.15] tracking-[-0.03em] whitespace-nowrap text-transparent [-webkit-text-stroke:2px_color-mix(in_oklab,var(--foreground)_28%,transparent)]";
export const H2 =
  "font-display font-[840] [font-stretch:118%] text-[clamp(2rem,4vw,3.5rem)] leading-[0.98] tracking-[-0.02em] text-balance";
export const H3 = "font-display text-[1.625rem] font-[780] [font-stretch:112%] tracking-tight";
export const LEAD =
  "font-sans text-[clamp(1.125rem,1.45vw,1.375rem)] leading-[1.45] text-foreground/80 text-pretty";
export const BODY = "font-sans text-[1.125rem] leading-[1.5] text-muted-foreground text-pretty";
export const VALUE =
  "font-sans text-base font-medium leading-snug text-muted-foreground tabular-nums";
export const LINK =
  "font-sans font-semibold underline underline-offset-4 decoration-foreground/30 transition-[text-decoration-color] duration-150 ease-out-strong hover:decoration-foreground";
export const PRIMARY_BUTTON =
  "raised h-12 rounded-xl bg-primary px-6 text-base font-semibold text-primary-foreground transition-[transform,box-shadow,background-color] duration-150 ease-out-strong hover:bg-[color-mix(in_oklab,var(--primary),white_8%)] active:scale-[0.97]";
/** The page's side margin, shared so every section lines up on one edge. */
export const GUTTER = "px-[max(5vw,1.25rem)]";
