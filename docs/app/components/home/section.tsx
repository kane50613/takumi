import type { ReactNode } from "react";
import { cn } from "~/lib/utils";
import { ShikiHtml, type ThemedHtml } from "./shiki-html";

/** Every homepage section shares this grid: text in the first five columns, the visual in the last six. */
export const GRID = "mx-auto w-full max-w-300 lg:grid lg:grid-cols-12 lg:gap-x-12";
export const TEXT_COLUMN = "lg:col-span-5";
export const VISUAL_COLUMN = "lg:col-span-6 lg:col-start-7";

export const H1 =
  "font-[540] text-[clamp(2.5rem,3.9vw,3.5rem)] leading-[1.06] tracking-tight text-balance";
export const H2 =
  "font-[540] text-[clamp(1.75rem,2.6vw,2.25rem)] leading-[1.15] tracking-tight text-balance";
export const LEAD = "text-[1.0625rem] leading-relaxed text-muted-foreground";
export const LINK = "underline underline-offset-4 decoration-border hover:decoration-foreground";

export function Section({ children, className }: { children: ReactNode; className?: string }) {
  return (
    <section className={cn("px-6 py-20 lg:py-28", className)}>
      <div className={cn(GRID, "grid gap-y-10 items-start")}>{children}</div>
    </section>
  );
}

/** Label and value rows; `mono` marks values that are code. */
export function Specs({ rows }: { rows: { label: string; values: string[]; mono?: boolean }[] }) {
  return (
    <dl className="grid grid-cols-[6rem_minmax(0,1fr)] gap-x-4 gap-y-3 text-sm">
      {rows.map(({ label, values, mono }) => (
        <div key={label} className="contents">
          <dt className="text-muted-foreground">{label}</dt>
          <dd className={cn("leading-relaxed", mono && "font-mono text-[0.8125rem]")}>
            {values.join(mono ? "  " : ", ")}
          </dd>
        </div>
      ))}
    </dl>
  );
}

export function Code({ html }: { html: ThemedHtml }) {
  return (
    <ShikiHtml
      html={html}
      className="rounded-xl border bg-muted/50 p-5 font-mono text-xs leading-relaxed overflow-x-auto"
    />
  );
}
