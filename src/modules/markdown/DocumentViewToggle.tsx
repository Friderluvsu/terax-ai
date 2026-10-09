import { cn } from "@/lib/utils";

export type DocumentViewOption<T extends string> = {
  value: T;
  label: string;
  disabled?: boolean;
  hint?: string;
};

type Props<T extends string> = {
  mode: T;
  options: DocumentViewOption<T>[];
  onChange: (mode: T) => void;
};

export function DocumentViewToggle<T extends string>({
  mode,
  options,
  onChange,
}: Props<T>) {
  return (
    <div className="absolute right-3 top-3 z-10 inline-flex items-center gap-0.5 rounded-md border border-border/60 bg-card/85 p-0.5 text-[11px] shadow-sm backdrop-blur">
      {options.map((o) => (
        <button
          key={o.value}
          type="button"
          onClick={() => onChange(o.value)}
          disabled={o.disabled}
          title={o.disabled ? o.hint : undefined}
          className={cn(
            "rounded px-2 py-0.5 transition-colors",
            mode === o.value
              ? "bg-accent text-foreground"
              : "text-muted-foreground hover:text-foreground",
            o.disabled &&
              "cursor-not-allowed opacity-40 hover:text-muted-foreground",
          )}
        >
          {o.label}
        </button>
      ))}
    </div>
  );
}
