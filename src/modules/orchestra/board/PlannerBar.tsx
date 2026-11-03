import {
  Add01Icon,
  Cancel01Icon,
  PlayIcon,
  SparklesIcon,
} from "@hugeicons/core-free-icons";
import { HugeiconsIcon } from "@hugeicons/react";
import { invoke } from "@tauri-apps/api/core";
import { useState } from "react";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Spinner } from "@/components/ui/spinner";
import { cn } from "@/lib/utils";
import {
  AGENT_LAUNCHERS,
  type AgentLauncherId,
} from "@/modules/agents/lib/launcher";
import {
  createParams,
  type Draft,
  draftProblem,
  newDraft,
  PLANNERS,
  type Planner,
  parsePlan,
} from "./lib/drafts";
import { useBoard, useBoardStore } from "./store";

const SELECT =
  "h-7 rounded-md border border-border/60 bg-input/40 px-1.5 text-xs text-foreground outline-none focus-visible:border-ring";
const FIELD =
  "w-full rounded-md border border-border/60 bg-input/40 px-2 py-1.5 text-xs text-foreground outline-none placeholder:text-muted-foreground focus-visible:border-ring";

function label(id: AgentLauncherId): string {
  return AGENT_LAUNCHERS.find((entry) => entry.id === id)?.label ?? id;
}

type Props = { run: string; cwd: string };

export function PlannerBar({ run, cwd }: Props) {
  const board = useBoard(run);
  const update = useBoardStore((state) => state.update);
  const [starting, setStarting] = useState(false);

  const setDraft = (id: number, patch: Partial<Draft>) =>
    update(run, (b) => ({
      drafts: b.drafts.map((d) => (d.id === id ? { ...d, ...patch } : d)),
    }));

  const plan = async () => {
    const job = board.job.trim();
    if (!job || board.planning) return;
    update(run, { planning: true });
    try {
      const planned = parsePlan(
        await invoke("orchestra_plan", { cwd, agent: board.planner, job }),
      );
      update(run, (b) => ({
        drafts: [
          ...b.drafts,
          ...planned.map((task) => newDraft(b.planner, task)),
        ],
      }));
    } catch (error) {
      toast.error(`${label(board.planner)} could not plan: ${String(error)}`);
    } finally {
      update(run, { planning: false });
    }
  };

  const problems = new Map(
    board.drafts.map((draft) => [draft.id, draftProblem(draft)]),
  );
  const ready =
    board.drafts.length > 0 && [...problems.values()].every((p) => !p);

  const start = async () => {
    if (!ready || starting) return;
    setStarting(true);
    try {
      await invoke("orchestra_board_open", {
        cwd,
        check: board.check.trim() || null,
      });
      for (const draft of board.drafts) {
        await invoke("orchestra_board_create", {
          run,
          params: createParams(draft),
        });
        update(run, (b) => ({
          drafts: b.drafts.filter((d) => d.id !== draft.id),
        }));
      }
      update(run, { job: "" });
    } catch (error) {
      toast.error(`Could not start the task: ${String(error)}`);
    } finally {
      setStarting(false);
    }
  };

  return (
    <div className="flex shrink-0 flex-col gap-2 border-b border-border/60 px-3 py-2">
      <div className="flex items-start gap-2">
        <textarea
          value={board.job}
          onChange={(event) => update(run, { job: event.target.value })}
          onKeyDown={(event) => {
            if (event.key === "Enter" && (event.metaKey || event.ctrlKey)) {
              event.preventDefault();
              void plan();
            }
          }}
          rows={1}
          maxLength={8000}
          placeholder="Describe the job. A planner agent splits it into tasks you can edit."
          className={cn(
            FIELD,
            "field-sizing-content max-h-32 min-h-8 flex-1 resize-none",
          )}
        />
        <select
          aria-label="Planner agent"
          value={board.planner}
          onChange={(event) =>
            update(run, { planner: event.target.value as Planner })
          }
          className={SELECT}
        >
          {PLANNERS.map((id) => (
            <option key={id} value={id}>
              Plan with {label(id)}
            </option>
          ))}
        </select>
        <Button
          size="sm"
          variant="secondary"
          className="h-7 gap-1.5 text-xs"
          disabled={!board.job.trim() || board.planning}
          onClick={() => void plan()}
        >
          {board.planning ? (
            <Spinner className="size-3.5" />
          ) : (
            <HugeiconsIcon icon={SparklesIcon} size={14} strokeWidth={1.75} />
          )}
          {board.planning ? "Planning" : "Plan"}
        </Button>
        <Button
          size="sm"
          variant="ghost"
          className="h-7 gap-1.5 text-xs"
          onClick={() =>
            update(run, (b) => ({ drafts: [...b.drafts, newDraft(b.planner)] }))
          }
        >
          <HugeiconsIcon icon={Add01Icon} size={14} strokeWidth={1.75} />
          Task
        </Button>
      </div>

      {board.planning ? (
        <p className="text-[11px] text-muted-foreground">
          {label(board.planner)} is reading the repository in read-only mode.
          This can take a minute.
        </p>
      ) : null}

      {board.drafts.length > 0 ? (
        <>
          <div className="flex gap-2 overflow-x-auto pb-1">
            {board.drafts.map((draft) => (
              <DraftCard
                key={draft.id}
                draft={draft}
                problem={problems.get(draft.id) ?? null}
                onChange={(patch) => setDraft(draft.id, patch)}
                onRemove={() =>
                  update(run, (b) => ({
                    drafts: b.drafts.filter((d) => d.id !== draft.id),
                  }))
                }
              />
            ))}
          </div>
          <div className="flex items-center gap-2">
            <Input
              value={board.check}
              onChange={(event) => update(run, { check: event.target.value })}
              maxLength={1024}
              placeholder="Check after each task, for example pnpm test (optional)"
              className="h-7 flex-1 rounded-md text-xs md:text-xs"
            />
            <Button
              size="sm"
              className="h-7 gap-1.5 text-xs"
              disabled={!ready || starting}
              title={ready ? undefined : "Every card needs a description"}
              onClick={() => void start()}
            >
              {starting ? (
                <Spinner className="size-3.5" />
              ) : (
                <HugeiconsIcon icon={PlayIcon} size={14} strokeWidth={1.75} />
              )}
              Start {board.drafts.length}
            </Button>
          </div>
        </>
      ) : null}
    </div>
  );
}

function DraftCard({
  draft,
  problem,
  onChange,
  onRemove,
}: {
  draft: Draft;
  problem: string | null;
  onChange: (patch: Partial<Draft>) => void;
  onRemove: () => void;
}) {
  return (
    <div className="flex w-72 shrink-0 flex-col gap-1.5 rounded-lg border border-border/60 bg-card p-2">
      <div className="flex items-center gap-1">
        <input
          value={draft.title}
          onChange={(event) => onChange({ title: event.target.value })}
          maxLength={120}
          placeholder="Title"
          className={cn(
            FIELD,
            "border-transparent bg-transparent px-1 font-medium",
          )}
        />
        <button
          type="button"
          aria-label="Remove task"
          onClick={onRemove}
          className="rounded p-1 text-muted-foreground hover:bg-accent hover:text-foreground"
        >
          <HugeiconsIcon icon={Cancel01Icon} size={12} strokeWidth={2} />
        </button>
      </div>
      <textarea
        value={draft.brief}
        onChange={(event) => onChange({ brief: event.target.value })}
        rows={4}
        placeholder="What should this agent do, and how should it verify it?"
        className={cn(FIELD, "resize-none")}
      />
      <div className="flex items-center gap-1.5">
        <select
          aria-label="Agent"
          value={draft.agent}
          onChange={(event) =>
            onChange({ agent: event.target.value as AgentLauncherId })
          }
          className={SELECT}
        >
          {AGENT_LAUNCHERS.map((launcher) => (
            <option key={launcher.id} value={launcher.id}>
              {launcher.label}
            </option>
          ))}
        </select>
        <input
          value={draft.model}
          onChange={(event) => onChange({ model: event.target.value })}
          maxLength={64}
          placeholder="Model (default)"
          className={cn(FIELD, "h-7 py-0")}
        />
      </div>
      {problem && (draft.brief || draft.title) ? (
        <p className="text-[11px] text-destructive">{problem}</p>
      ) : null}
    </div>
  );
}
