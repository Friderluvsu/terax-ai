import type { AgentLauncherId } from "@/modules/agents";
import { AGENT_LAUNCHERS } from "@/modules/agents";
import { quoteShellArg } from "@/lib/shellQuote";

export { ORCHESTRA_SPACE_ID, ORCHESTRA_SPACE_NAME } from "./space";

export type TaskOpenRequest = {
  task: string;
  agent: AgentLauncherId;
  title?: string;
  cwd: string;
  briefPath: string;
  model?: string;
  settingsPath?: string;
  /** Board run that shows this worker as a tile. */
  board?: string;
};

export type TaskSendRequest = { paneId: number; text: string };

export type SupervisorFiles = {
  guidePath: string;
  settingsPath: string;
  jobPath: string;
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
const MODEL = /^[A-Za-z0-9._:/[\]][A-Za-z0-9._:/[\]-]{0,63}$/;
const MAX_SEND_BYTES = 4096;

function invalid(message: string): never {
  throw new TaskRequestError("invalid_params", message);
}

function optional<T>(
  value: unknown,
  parse: (value: unknown) => T,
): T | undefined {
  return value === undefined || value === null ? undefined : parse(value);
}

function requirePaneId(params: unknown): number {
  const pane =
    typeof params === "object" && params !== null
      ? (params as Record<string, unknown>).pane_id
      : undefined;
  if (!Number.isSafeInteger(pane) || (pane as number) < 0) {
    invalid("pane_id must be a non-negative integer");
  }
  return pane as number;
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
    title: optional(value.title, (title) => requireText(title, "title", 120)),
    cwd: requireText(value.cwd, "cwd", 4096),
    briefPath: requireText(value.brief_path, "brief_path", 4096),
    model: optional(value.model, (model) => {
      if (typeof model !== "string" || !MODEL.test(model)) {
        invalid("model is malformed");
      }
      return model;
    }),
    settingsPath: optional(value.settings_path, (path) =>
      requireText(path, "settings_path", 4096),
    ),
    board: optional(value.board, (run) => {
      if (typeof run !== "string" || !TASK_ID.test(run)) {
        invalid("board run is malformed");
      }
      return run;
    }),
  };
}

export function parseTaskScreen(params: unknown): {
  paneId: number;
  lines: number;
} {
  const paneId = requirePaneId(params);
  const lines = (params as Record<string, unknown>).lines;
  if (
    !Number.isSafeInteger(lines) ||
    (lines as number) < 1 ||
    (lines as number) > 200
  ) {
    invalid("lines must be an integer from 1 to 200");
  }
  return { paneId, lines: lines as number };
}

export function parseSupervisorFiles(value: unknown): SupervisorFiles {
  const record =
    typeof value === "object" && value !== null
      ? (value as Record<string, unknown>)
      : {};
  return {
    guidePath: requireText(record.guide_path, "guide_path", 4096),
    settingsPath: requireText(record.settings_path, "settings_path", 4096),
    jobPath: requireText(record.job_path, "job_path", 4096),
  };
}

// The guide is the system prompt and the settings pre-approve only the Terax
// CLI and edits inside the integration worktree, so nothing is installed into
// the user's Claude config.
export function supervisorLaunchCommand(
  command: string,
  files: SupervisorFiles,
  windows?: boolean,
): string {
  const prompt = `Read the job and run id in ${files.jobPath} and supervise that run now, following your instructions.`;
  return [
    command,
    "--settings",
    quoteShellArg(files.settingsPath, windows),
    "--append-system-prompt-file",
    quoteShellArg(files.guidePath, windows),
    "--",
    quoteShellArg(prompt, windows),
  ].join(" ");
}

export function parseTaskClose(params: unknown): number {
  return requirePaneId(params);
}

export function parseTaskSend(params: unknown): TaskSendRequest {
  const paneId = requirePaneId(params);
  const text = (params as Record<string, unknown>).text;
  if (
    typeof text !== "string" ||
    text.trim().length === 0 ||
    new TextEncoder().encode(text).length > MAX_SEND_BYTES ||
    CONTROL_CHARACTERS.test(text)
  ) {
    invalid(`text must be a single line of 1-${MAX_SEND_BYTES} bytes`);
  }
  return { paneId, text };
}

// Only the brief path is passed on the command line; the brief itself can be
// long and is never pasted into the shell.
function workerPrompt(request: TaskOpenRequest): string {
  const dir = parentDir(request.briefPath);
  const sep = request.briefPath.includes("\\") ? "\\" : "/";
  return [
    `You are a worker on task ${request.task} in a dedicated git worktree on branch task/${request.task}.`,
    `Read the task brief at ${request.briefPath} and complete it in this worktree.`,
    "Commit your work to this branch. Do not push, merge, or change other branches.",
    `When done, write a short summary (what changed, how you verified it, open questions) to ${dir}${sep}result.md.`,
  ].join(" ");
}

const PROMPT_FLAG: Partial<Record<AgentLauncherId, string>> = {
  antigravity: "-i",
  opencode: "--prompt",
};

const MODEL_FLAG: Partial<Record<AgentLauncherId, string>> = {
  claude: "--model",
  codex: "--model",
  antigravity: "--model",
};

function parentDir(path: string): string {
  const cut = Math.max(path.lastIndexOf("/"), path.lastIndexOf("\\"));
  return cut > 0 ? path.slice(0, cut) : path;
}

export function workerLaunchCommand(
  request: TaskOpenRequest,
  command: string,
  windows?: boolean,
): string {
  const parts = [command];
  // Workers run unattended: edits and sandboxed commands go through without
  // prompts, and the brief beside the worktree is readable.
  if (request.agent === "claude") {
    if (request.settingsPath) {
      parts.push("--settings", quoteShellArg(request.settingsPath, windows));
    }
    parts.push(
      "--permission-mode",
      "acceptEdits",
      "--add-dir",
      quoteShellArg(parentDir(request.briefPath), windows),
    );
  }
  if (request.agent === "antigravity") {
    parts.push(
      "--mode",
      "accept-edits",
      "--add-dir",
      quoteShellArg(parentDir(request.briefPath), windows),
    );
  }
  // Spelled out because Codex 0.163 removed the --full-auto shorthand.
  if (request.agent === "codex") {
    parts.push(
      "--sandbox",
      "workspace-write",
      "--ask-for-approval",
      "on-request",
    );
  }
  const modelFlag = MODEL_FLAG[request.agent];
  if (modelFlag && request.model) {
    parts.push(modelFlag, quoteShellArg(request.model, windows));
  }
  const flag = PROMPT_FLAG[request.agent];
  if (flag) parts.push(flag);
  // Claude's --add-dir is variadic and would swallow the prompt as a directory.
  if (request.agent === "claude") parts.push("--");
  parts.push(quoteShellArg(workerPrompt(request), windows));
  return parts.join(" ");
}
