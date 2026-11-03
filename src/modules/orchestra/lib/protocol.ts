import type { AgentLauncherId } from "@/modules/agents";
import { AGENT_LAUNCHERS } from "@/modules/agents";
import { quoteShellArg } from "@/lib/shellQuote";

export const ORCHESTRA_SPACE_ID = "orchestra";
export const ORCHESTRA_SPACE_NAME = "Orchestra";

export type TaskOpenRequest = {
  task: string;
  agent: AgentLauncherId;
  title?: string;
  cwd: string;
  briefPath: string;
};

export class TaskRequestError extends Error {
  constructor(
    readonly code: string,
    message: string,
  ) {
    super(message);
  }
}

const TASK_ID = /^[a-z0-9][a-z0-9-]{0,63}$/;
const CONTROL_CHARACTERS = /[\u0000-\u001f\u007f]/;

function invalid(message: string): never {
  throw new TaskRequestError("invalid_params", message);
}

function requireText(value: unknown, name: string, max: number): string {
  if (
    typeof value !== "string" ||
    value.length === 0 ||
    value.length > max ||
    CONTROL_CHARACTERS.test(value)
  ) {
    invalid(`${name} must be a single line of 1-${max} characters`);
  }
  return value;
}

export function parseTaskOpen(params: unknown): TaskOpenRequest {
  if (typeof params !== "object" || params === null) {
    invalid("task parameters are required");
  }
  const value = params as Record<string, unknown>;
  const task = requireText(value.task, "task", 64);
  if (!TASK_ID.test(task)) invalid("task id is malformed");
  const agent = AGENT_LAUNCHERS.find((entry) => entry.id === value.agent)?.id;
  if (!agent) invalid("agent is not a known launcher");
  return {
    task,
    agent,
    title:
      value.title === undefined || value.title === null
        ? undefined
        : requireText(value.title, "title", 120),
    cwd: requireText(value.cwd, "cwd", 4096),
    briefPath: requireText(value.brief_path, "brief_path", 4096),
  };
}

export function parseTaskClose(params: unknown): number {
  const pane =
    typeof params === "object" && params !== null
      ? (params as Record<string, unknown>).pane_id
      : undefined;
  if (!Number.isSafeInteger(pane) || (pane as number) < 0) {
    invalid("pane_id must be a non-negative integer");
  }
  return pane as number;
}

// Only the brief path is passed on the command line; the brief itself can be
// long and is never pasted into the shell.
export function workerPrompt(request: TaskOpenRequest): string {
  return [
    `You are a worker on task ${request.task} in a dedicated git worktree on branch task/${request.task}.`,
    `Read the task brief at ${request.briefPath} and complete it in this worktree.`,
    "Commit your work to this branch. Do not push, merge, or change other branches.",
  ].join(" ");
}

const PROMPT_FLAG: Partial<Record<AgentLauncherId, string>> = {
  gemini: "-i",
  opencode: "--prompt",
};

export function workerLaunchCommand(
  request: TaskOpenRequest,
  command: string,
  windows?: boolean,
): string {
  const flag = PROMPT_FLAG[request.agent];
  const prompt = quoteShellArg(workerPrompt(request), windows);
  return flag ? `${command} ${flag} ${prompt}` : `${command} ${prompt}`;
}
