type Status = "supported" | "partial" | "unsupported";

export interface StylePropertyProps {
  name: string;
  status: Status;
  example?: string;
  note?: string;
}

const statusColors: Record<Status, string> = {
  supported: "bg-emerald-500/10 text-emerald-600 dark:text-emerald-400",
  partial: "bg-amber-500/10 text-amber-600 dark:text-amber-400",
  unsupported: "bg-rose-500/10 text-rose-600 dark:text-rose-400",
};

const statusLabels: Record<Status, string> = {
  supported: "Supported",
  partial: "Partial",
  unsupported: "Unsupported",
};

export function StyleProperty({ name, status, example, note }: StylePropertyProps) {
  return (
    <div className="my-3 flex flex-col gap-2 rounded-lg border border-fd-border bg-fd-card p-3">
      <div className="flex flex-wrap items-baseline gap-3">
        <code className="font-mono text-sm font-medium">{name}</code>
        <span className={`rounded px-2 py-0.5 text-xs ${statusColors[status]}`}>
          {statusLabels[status]}
        </span>
      </div>
      {example ? <code className="text-xs text-fd-muted-foreground">{example}</code> : null}
      {note ? <p className="text-xs text-fd-muted-foreground">{note}</p> : null}
    </div>
  );
}
