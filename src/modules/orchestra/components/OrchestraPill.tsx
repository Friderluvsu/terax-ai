import { lazy, Suspense } from "react";
import {
  Popover,
  PopoverContent,
  PopoverTrigger,
} from "@/components/ui/popover";
import { cn } from "@/lib/utils";
import { summarize } from "../lib/board";
import { useOrchestraTasks } from "../lib/useOrchestraTasks";

const OrchestraBoard = lazy(() => import("./OrchestraBoard"));

export function OrchestraPill() {
  const tasks = useOrchestraTasks();
  const summary = summarize(tasks);
  if (!summary.visible) return null;

  const dot =
    summary.needsInput > 0
      ? "bg-amber-500"
      : summary.failed > 0
        ? "bg-red-500"
        : summary.working > 0
          ? "bg-sky-500 animate-pulse"
          : "bg-emerald-500";
  const detail = [
    summary.working > 0 && `${summary.working} working`,
    summary.needsInput > 0 && `${summary.needsInput} need input`,
    summary.queued > 0 && `${summary.queued} queued`,
    summary.failed > 0 && `${summary.failed} failed`,
  ]
    .filter(Boolean)
    .join(", ");

  return (
    <Popover>
      <PopoverTrigger asChild>
        <button
          type="button"
          className="flex h-6 shrink-0 items-center gap-1.5 rounded-sm px-1.5 text-[11px] text-muted-foreground outline-none hover:bg-accent hover:text-foreground data-[state=open]:bg-accent data-[state=open]:text-foreground"
          title={detail || "Orchestra tasks"}
        >
          <span className={cn("size-1.5 rounded-full", dot)} />
          <span>Tasks {summary.total}</span>
          {summary.needsInput > 0 ? (
            <span className="text-amber-700 dark:text-amber-400">
              {summary.needsInput} waiting
            </span>
          ) : null}
        </button>
      </PopoverTrigger>
      <PopoverContent side="top" align="end" className="w-96 p-0">
        <Suspense fallback={null}>
          <OrchestraBoard tasks={tasks} />
        </Suspense>
      </PopoverContent>
    </Popover>
  );
}
