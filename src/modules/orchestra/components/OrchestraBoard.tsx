import { invoke } from "@tauri-apps/api/core";
import { useState } from "react";
import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";
import {
  type BoardTask,
  boardTasks,
  canStop,
  statusLabel,
  statusTone,
} from "../lib/board";

const TONE = {
  busy: "bg-sky-500",
  attention: "bg-amber-500",
  ok: "bg-emerald-500",
  bad: "bg-red-500",
  idle: "bg-muted-foreground/40",
} as const;

export default function OrchestraBoard({ tasks }: { tasks: BoardTask[] }) {
  const [pending, setPending] = useState<string | null>(null);
  const shown = boardTasks(tasks);

  const stop = (task?: string) => {
    setPending(task ?? "*");
    invoke("orchestra_stop", { task: task ?? null })
      .catch((error) => console.error("[terax] stop task:", error))
      .finally(() => setPending(null));
  };

  return (
    <div className="flex flex-col text-[12px]">
      <div className="flex items-center justify-between border-b px-3 py-2">
        <span className="font-medium">Orchestra</span>
        <Button
          size="xs"
          variant="ghost"
          disabled={pending !== null || !shown.some(canStop)}
          onClick={() => stop()}
        >
          Stop all
        </Button>
      </div>
      <ul className="max-h-80 overflow-y-auto py-1">
        {shown.map((task) => (
          <li
            key={task.id}
            className="flex items-center gap-2 px-3 py-1.5 hover:bg-accent/50"
          >
            <span
              className={cn(
                "size-1.5 shrink-0 rounded-full",
                TONE[statusTone(task)],
              )}
            />
            <div className="min-w-0 flex-1">
              <div className="truncate">{task.title ?? task.branch}</div>
              <div className="truncate text-[10.5px] text-muted-foreground">
                {task.id} · {task.agent} · {statusLabel(task)}
                {task.error ? ` · ${task.error}` : ""}
              </div>
            </div>
            {canStop(task) ? (
              <Button
                size="xs"
                variant="ghost"
                disabled={pending !== null}
                onClick={() => stop(task.id)}
              >
                {task.status === "done" ? "Close" : "Stop"}
              </Button>
            ) : null}
          </li>
        ))}
      </ul>
      <p className="border-t px-3 py-2 text-[10.5px] text-muted-foreground">
        Stopping keeps each worktree and branch for review.
      </p>
    </div>
  );
}
