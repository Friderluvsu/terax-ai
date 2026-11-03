import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { AgentLauncherId } from "@/modules/agents/lib/launcher";

export type ModelOption = { id: string; label: string; note: string };
export type AgentCatalog = {
  agent: AgentLauncherId;
  installed: boolean;
  models: ModelOption[];
};

export const CUSTOM_MODEL = "__custom__";

export function modelsFor(
  catalog: AgentCatalog[],
  agent: AgentLauncherId,
): ModelOption[] {
  return catalog.find((entry) => entry.agent === agent)?.models ?? [];
}

export function isInstalled(
  catalog: AgentCatalog[],
  agent: AgentLauncherId,
): boolean {
  // Unknown until the catalog loads, so nothing is flagged early.
  return (
    catalog.length === 0 ||
    (catalog.find((entry) => entry.agent === agent)?.installed ?? false)
  );
}

/** What the model select shows: a listed id, the default, or a custom one. */
export function modelChoice(models: ModelOption[], model: string): string {
  if (!model) return "";
  return models.some((entry) => entry.id === model) ? model : CUSTOM_MODEL;
}

let cached: Promise<AgentCatalog[]> | null = null;

export function useAgentCatalog(): AgentCatalog[] {
  const [catalog, setCatalog] = useState<AgentCatalog[]>([]);
  useEffect(() => {
    let disposed = false;
    cached ??= invoke<AgentCatalog[]>("orchestra_catalog").catch((error) => {
      cached = null;
      console.warn("[terax] agent catalog:", error);
      return [];
    });
    void cached.then((next) => {
      if (!disposed) setCatalog(next);
    });
    return () => {
      disposed = true;
    };
  }, []);
  return catalog;
}
