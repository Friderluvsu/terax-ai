import { FileViewIcon, StopIcon } from "@hugeicons/core-free-icons";
import { HugeiconsIcon } from "@hugeicons/react";
import { invoke } from "@tauri-apps/api/core";
import { useState } from "react";
import { toast } from "sonner";
import {
  Popover,
  PopoverContent,
  PopoverTrigger,
} from "@/components/ui/popover";
import { Spinner } from "@/components/ui/spinner";
import { cn } from "@/lib/utils";
import { AGENT_LAUNCHERS } from "@/modules/agents/lib/launcher";
import { type BoardTask, canStop, statusLabel, statusTone } from "../lib/board";

const TONE = {
  busy: "bg-sky-500 animate-pulse",
  attention: "bg-amber-500",
  ok: "bg-emerald-500",
  bad: "bg-red-500",
  idle: "bg-muted-foreground/40",
} as const;

const ICON_BUTTON =
  "rounded p-1 text-muted-foreground hover:bg-accent hover:text-foreground disabled:opacity-40";

function agentLabel(agent: string): string {
  return AGENT_LAUNCHERS.find((entry) => entry.id === agent)?.label ?? agent;
}

function basename(path: string): string {
  return path.split(/[\\/]/).filter(Boolean).pop() ?? path;
}

const stopPress = (event: React.PointerEvent) => event.stopPropagation();

function stopTask(task: BoardTask) {
  invoke("orchestra_stop", { task: task.id }).catch((error) =>
    toast.error(`Could not stop ${task.id}: ${String(error)}`),
  );
}

function hasResult(task: BoardTask): boolean {
  return (
    task.status === "done" ||
    task.status === "needs_input" ||
    task.status === "failed" ||
    task.checks !== undefined
  );
}

export function TileHeader({
  task,
  isConsole,
  cwd,
  onPointerDown,
}: {
  task: BoardTask | undefined;
  isConsole: boolean;
  cwd: string;
  onPointerDown: (event: React.PointerEvent) => void;
}) {
  return (
    <div
      onPointerDown={onPointerDown}
      className="flex h-8 shrink-0 cursor-grab items-center gap-2 border-b border-border/60 px-2 text-xs select-none active:cursor-grabbing"
    >
      {task ? (
        <>
          <span
            className={cn(
              "size-1.5 shrink-0 rounded-full",
              TONE[statusTone(task)],
            )}
          />
          <span className="min-w-0 truncate font-medium">
            {task.title ?? task.id}
          </span>
          <span className="shrink-0 text-muted-foreground">
            {agentLabel(task.agent)}
          </span>
          <span className="ml-auto shrink-0 text-muted-foreground">
            {statusLabel(task)}
          </span>
          {hasResult(task) ? <ResultButton task={task} /> : null}
          <button
            type="button"
            aria-label="Stop task"
            title="Stop and close this worker"
            onPointerDown={stopPress}
            onClick={() => stopTask(task)}
            disabled={!canStop(task)}
            className={ICON_BUTTON}
          >
            <HugeiconsIcon icon={StopIcon} size={13} strokeWidth={1.75} />
          </button>
        </>
      ) : (
        <>
          <span className="size-1.5 shrink-0 rounded-full bg-muted-foreground/40" />
          <span className="font-medium">
            {isConsole ? "Console" : "Worker"}
          </span>
          <span className="min-w-0 truncate text-muted-foreground">
            {basename(cwd)}
          </span>
        </>
      )}
    </div>
  );
}

type Result = { result: string | null; check_log: string | null };

function ResultButton({ task }: { task: BoardTask }) {
  const [result, setResult] = useState<Result | null>(null);
  const [error, setError] = useState<string | null>(null);

  const load = (open: boolean) => {
    if (!open) return;
    setResult(null);
    setError(null);
    invoke<Result>("orchestra_result", { task: task.id })
      .then(setResult)
      .catch((reason) => setError(String(reason)));
  };

  return (
    <Popover onOpenChange={load}>
      <PopoverTrigger asChild>
        <button
          type="button"
          aria-label="Show result"
          title="Result and checks"
          onPointerDown={stopPress}
          className={ICON_BUTTON}
        >
          <HugeiconsIcon icon={FileViewIcon} size={13} strokeWidth={1.75} />
        </button>
      </PopoverTrigger>
      <PopoverContent align="end" className="w-[28rem] p-0">
        <div className="flex items-center gap-2 border-b border-border/60 px-3 py-2 text-xs">
          <span className="font-medium">{task.title ?? task.id}</span>
          <span className="ml-auto font-mono text-muted-foreground">
            {task.branch}
          </span>
        </div>
        <div className="max-h-96 overflow-auto px-3 py-2 text-xs">
          {error ? (
            <p className="text-destructive">{error}</p>
          ) : !result ? (
            <Spinner className="size-4" />
          ) : (
            <>
              <pre className="font-sans whitespace-pre-wrap">
                {result.result ?? "The agent has not written result.md yet."}
              </pre>
              {result.check_log ? (
                <>
                  <p className="mt-3 mb-1 font-medium">Check log</p>
                  <pre className="rounded bg-muted/60 p-2 font-mono text-[11px] whitespace-pre-wrap">
                    {result.check_log}
                  </pre>
                </>
              ) : null}
            </>
          )}
        </div>
      </PopoverContent>
    </Popover>
  );
}

const MAX_OFF_CANVAS = 12;

/** Queued tasks and finished ones whose terminal already closed. */
export function OffCanvasTasks({ tasks }: { tasks: BoardTask[] }) {
  if (tasks.length === 0) return null;
  const shown = tasks.slice(-MAX_OFF_CANVAS).reverse();
  return (
    <div className="absolute bottom-3 left-3 flex max-h-[40%] w-64 flex-col overflow-auto rounded-md border border-border/60 bg-card/95 py-1 text-xs shadow-sm">
      {shown.map((task) => (
        <div key={task.id} className="flex items-center gap-2 px-2 py-1">
          <span
            className={cn(
              "size-1.5 shrink-0 rounded-full",
              TONE[statusTone(task)],
            )}
          />
          <span className="min-w-0 truncate">{task.title ?? task.id}</span>
          <span className="ml-auto shrink-0 text-muted-foreground">
            {statusLabel(task)}
          </span>
          {hasResult(task) ? <ResultButton task={task} /> : null}
          {task.status === "queued" ? (
            <button
              type="button"
              aria-label="Cancel task"
              title="Cancel"
              onClick={() => stopTask(task)}
              className={ICON_BUTTON}
            >
              <HugeiconsIcon icon={StopIcon} size={12} strokeWidth={1.75} />
            </button>
          ) : null}
        </div>
      ))}
    </div>
  );
}
