type TaskStatus =
  | "queued"
  | "starting"
  | "running"
  | "needs_input"
  | "done"
  | "stopped"
  | "failed"
  | "interrupted";

export type BoardTask = {
  id: string;
  run: string;
  agent: string;
  title?: string;
  status: TaskStatus;
  branch: string;
  pane_id?: number;
  error?: string;
  checks?: "running" | "passed" | "failed";
};

export type BoardSummary = {
  visible: boolean;
  working: number;
  queued: number;
  needsInput: number;
  failed: number;
  total: number;
};

const WORKING = new Set<TaskStatus>(["starting", "running"]);

function failedTask(task: BoardTask): boolean {
  return task.status === "failed" || task.checks === "failed";
}

/** Tasks worth showing: anything still in flight or with its tab still open. */
export function boardTasks(tasks: BoardTask[]): BoardTask[] {
  return tasks.filter(
    (task) =>
      task.status === "queued" ||
      WORKING.has(task.status) ||
      task.status === "needs_input" ||
      task.pane_id !== undefined,
  );
}

export function summarize(tasks: BoardTask[]): BoardSummary {
  const shown = boardTasks(tasks);
  return {
    visible: shown.length > 0,
    working: shown.filter(
      (task) => WORKING.has(task.status) || task.checks === "running",
    ).length,
    queued: shown.filter((task) => task.status === "queued").length,
    needsInput: shown.filter((task) => task.status === "needs_input").length,
    failed: shown.filter(failedTask).length,
    total: shown.length,
  };
}

export function statusLabel(task: BoardTask): string {
  if (task.checks === "running") return "checking";
  if (task.checks === "failed") return "checks failed";
  if (task.status === "done" && task.checks === "passed")
    return "done, checks passed";
  return task.status.replace("_", " ");
}

export function statusTone(
  task: BoardTask,
): "busy" | "attention" | "ok" | "bad" | "idle" {
  if (task.status === "needs_input") return "attention";
  if (failedTask(task)) return "bad";
  if (WORKING.has(task.status) || task.checks === "running") return "busy";
  if (task.status === "done") return "ok";
  return "idle";
}

export function canStop(task: BoardTask): boolean {
  return task.status !== "stopped" && task.status !== "interrupted";
}
