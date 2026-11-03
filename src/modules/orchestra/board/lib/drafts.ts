import {
  AGENT_LAUNCHERS,
  type AgentLauncherId,
} from "@/modules/agents/lib/launcher";

export const PLANNERS = ["claude", "codex", "gemini"] as const;
export type Planner = (typeof PLANNERS)[number];

export type Draft = {
  id: number;
  title: string;
  brief: string;
  agent: AgentLauncherId;
  model: string;
};

export type PlannedTask = { title: string; brief: string };

const MODEL = /^[A-Za-z0-9._:/[\]][A-Za-z0-9._:/[\]-]{0,63}$/;
const MAX_TITLE = 120;
const MAX_BRIEF_BYTES = 16 * 1024;

let nextDraftId = 1;

export function newDraft(
  agent: AgentLauncherId,
  task: PlannedTask = { title: "", brief: "" },
): Draft {
  return { id: nextDraftId++, ...task, agent, model: "" };
}

export function parsePlan(value: unknown): PlannedTask[] {
  if (!Array.isArray(value)) throw new Error("planner returned no task list");
  return value.flatMap((entry) => {
    if (typeof entry !== "object" || entry === null) return [];
    const { title, brief } = entry as Record<string, unknown>;
    if (typeof title !== "string" || typeof brief !== "string") return [];
    return [{ title, brief }];
  });
}

/** Mirrors the backend limits so a bad card is flagged before Start. */
export function draftProblem(draft: Draft): string | null {
  if (!draft.brief.trim()) return "Describe what this agent should do.";
  if (new TextEncoder().encode(draft.brief).length > MAX_BRIEF_BYTES) {
    return "The brief is longer than 16 KB.";
  }
  if (draft.title.length > MAX_TITLE || /[\u0000-\u001f]/.test(draft.title)) {
    return "Keep the title to one line of 120 characters.";
  }
  if (draft.model && !MODEL.test(draft.model))
    return "The model name is invalid.";
  if (!AGENT_LAUNCHERS.some((launcher) => launcher.id === draft.agent)) {
    return "Pick an agent.";
  }
  return null;
}

export function createParams(draft: Draft) {
  return {
    agent: draft.agent,
    brief: draft.brief.trim(),
    title: draft.title.trim() || null,
    model: draft.model.trim() || null,
  };
}
